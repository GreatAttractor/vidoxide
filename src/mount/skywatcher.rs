//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Sky-Watcher mount direct serial connection driver.
//!
//! Based on Sky-Watcher's official API and examples.
//! NOTE: this code has been only tested with a 2014 HEQ5 mount.
//!

use std::{error::Error, f64::consts::PI};
use crate::mount::{Axis, Mount, RadPerSec, SIDEREAL_RATE};

const AXIS_STOP_MOTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const MAX_SPEED: RadPerSec = RadPerSec(800.0 * SIDEREAL_RATE.0);

const LOW_SPEED_THRESHOLD: RadPerSec = RadPerSec(128.0 * SIDEREAL_RATE.0);

impl Axis {
    fn as_char(&self) -> char {
        match self {
            Axis::Primary => '1',
            Axis::Secondary => '2'
        }
    }

    fn as_index(&self) -> usize {
        match self {
            Axis::Primary => 0,
            Axis::Secondary => 1
        }
    }
}

enum AxisStatus {
    FullStopped      = 0x0001,
    Slewing          = 0x0002,
    SlewingTo        = 0x0004,
    SlewingForward   = 0x0008,
    SlewingHighspeed = 0x0010,
    NotInitialized   = 0x0020
}

mod command {
    pub const START_CHAR_OUT: u8 = ':' as u8;
    pub const END_CHAR: u8       = 0xD;
    pub const START_CHAR_IN: u8  = '=' as u8;
    pub const ERROR_CHAR: u8     = '!' as u8;
}

enum Opcode {
    InitMotorCtrl,
    SetMotionMode,
    SetStepPeriod,
    StartMotion,
    StopMotion,
    GetGearRatio,
    GetTimerIntFreq,
    GetAxisStatus,
    GetHiSpeedRatio,
    GetPecPeriod
}

impl Opcode {
    fn as_char(&self) -> char {
        match self {
            Opcode::InitMotorCtrl =>   'F',
            Opcode::SetMotionMode =>   'G',
            Opcode::SetStepPeriod =>   'I',
            Opcode::StartMotion =>     'J',
            Opcode::StopMotion =>      'K',
            Opcode::GetGearRatio =>    'a',
            Opcode::GetTimerIntFreq => 'b',
            Opcode::GetAxisStatus =>   'f',
            Opcode::GetHiSpeedRatio => 'g',
            Opcode::GetPecPeriod =>    's'
        }
    }
}

mod motion {
    pub mod speed {
        pub const LOW: char  = '1';
        pub const HIGH: char = '3';
    }

    pub mod direction {
        pub const POS: char = '0';
        pub const NEG: char = '1';
    }
}

pub struct SkyWatcher {
    device: String,
    serial_port: Box<dyn serialport::SerialPort>,
    rad_rate_to_int: [f64; 2],
    hi_speed_ratio: [u32; 2],
    current_slewing_speed: [RadPerSec; 2],
    tracking: bool
}

impl SkyWatcher {
    /// Creates a Sky-Watcher mount instance.
    ///
    /// # Parameters
    ///
    /// * `device` - System device name to use for connecting to the mount,
    ///     e.g., "COM3" on Windows or "/dev/ttyUSB0" on Linux.
    ///
    #[must_use]
    pub fn new(device: &str) -> Result<SkyWatcher, Box<dyn Error>> {
        let mut serial_port = serialport::new(device, 9600)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(50))
            .open()?;

        let mut rad_to_step = [0.0; 2];

        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Primary, Opcode::GetGearRatio, "")?;
        rad_to_step[Axis::Primary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))? as f64 / (2.0 * PI);

        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Secondary, Opcode::GetGearRatio, "")?;
        rad_to_step[Axis::Secondary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))? as f64 / (2.0 * PI);

        let mut timer_interrupt_freq = [0u32; 2];

        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Primary, Opcode::GetTimerIntFreq, "")?;
        timer_interrupt_freq[Axis::Primary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))?;

        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Secondary, Opcode::GetTimerIntFreq, "")?;
        timer_interrupt_freq[Axis::Secondary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))?;

        let mut rad_rate_to_int = [0.0; 2];
        for i in 0..=1 {
            rad_rate_to_int[i] = timer_interrupt_freq[i] as f64 / rad_to_step[i];
        }

        let mut hi_speed_ratio = [0u32; 2];
        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Primary, Opcode::GetHiSpeedRatio, "")?;
        hi_speed_ratio[Axis::Primary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))?;

        let response = send_cmd_and_get_reply(&mut serial_port, Axis::Secondary, Opcode::GetHiSpeedRatio, "")?;
        hi_speed_ratio[Axis::Secondary.as_index()] = skywatcher_hex_str_to_u32(&extract_hex_number(&response))?;

        send_cmd_and_get_reply(&mut serial_port, Axis::Primary, Opcode::InitMotorCtrl, "")?;
        send_cmd_and_get_reply(&mut serial_port, Axis::Secondary, Opcode::InitMotorCtrl, "")?;

        Ok(SkyWatcher{
            device: device.to_string(),
            tracking: false,
            serial_port,
            rad_rate_to_int,
            hi_speed_ratio,
            current_slewing_speed: [RadPerSec(0.0); 2]
        })
    }

    #[must_use]
    fn is_stopped(&mut self, axis: Axis) -> Result<bool, Box<dyn Error>> {
        let response = send_cmd_and_get_reply(&mut self.serial_port, axis, Opcode::GetAxisStatus, "")?;
        if response.len() < 3 {
            Err("invalid response".into())
        } else {
            Ok(response[2] & 0x01 == 0)
        }
    }

    #[must_use]
    fn update_step_period(&mut self, axis: Axis, mut speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if speed > MAX_SPEED {
            speed = MAX_SPEED
        } else if speed < -MAX_SPEED {
            speed = -MAX_SPEED;
        }

        if speed.abs() > LOW_SPEED_THRESHOLD {
            speed /= self.hi_speed_ratio[axis.as_index()] as f64;
        }

        let factor = self.rad_rate_to_int[axis.as_index()];
        let speed_int = std::cmp::max(6, (factor / speed.abs().0) as u32);

        send_cmd_and_get_reply(
            &mut self.serial_port, axis, Opcode::SetStepPeriod, &u32_to_skywatcher_hex_str(speed_int)
        )?;

        Ok(())
    }

    fn set_motion(&mut self, axis: Axis, speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if speed.abs() < 0.001 * SIDEREAL_RATE {
            return self.stop_motion(axis);
        }

        // Cannot update speed between low and high speed regime; need to stop first.
        if (self.current_slewing_speed[axis.as_index()].abs() <= LOW_SPEED_THRESHOLD) ^
            (speed.abs() <= LOW_SPEED_THRESHOLD) {

            self.stop_motion(axis)?;
        }

        if speed.0 * self.current_slewing_speed[axis.as_index()].0 > 0.0 {
            // already slewing in the same direction
            self.update_step_period(axis, speed)?;
            self.current_slewing_speed[axis.as_index()] = speed;
        } else {
            let dir_positive = speed >= RadPerSec(0.0);
            let hi_speed = speed.abs() > LOW_SPEED_THRESHOLD;

            self.stop_motion(axis)?;

            send_cmd_and_get_reply(
                &mut self.serial_port,
                axis,
                Opcode::SetMotionMode,
                &format!(
                    "{}{}",
                    if hi_speed { motion::speed::HIGH } else { motion::speed::LOW },
                    if dir_positive { motion::direction::POS } else { motion::direction::NEG }
                )
            )?;

            self.update_step_period(axis, speed)?;

            send_cmd_and_get_reply(&mut self.serial_port, axis, Opcode::StartMotion, "")?;

            self.current_slewing_speed[axis.as_index()] = speed;
        }

        Ok(())
    }

    fn stop_motion(&mut self, axis: Axis) -> Result<(), Box<dyn Error>> {
        send_cmd_and_get_reply(&mut self.serial_port, axis, Opcode::StopMotion, "")?;

        let tstart = std::time::Instant::now();
        while tstart.elapsed() < AXIS_STOP_MOTION_TIMEOUT && !self.is_stopped(axis)? {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        self.current_slewing_speed[axis.as_index()] = RadPerSec(0.0);

        Ok(())
    }
}

impl Mount for SkyWatcher {
    fn get_info(&self) -> String {
        format!("Sky-Watcher on {}", self.device)
    }

    fn set_tracking(&mut self, enabled: bool) -> Result<(), Box<dyn Error>> {
        self.tracking = enabled;
        self.set_motion(Axis::Primary, if enabled { SIDEREAL_RATE } else { RadPerSec(0.0) })
    }

    fn guide(&mut self, axis1_speed: RadPerSec, axis2_speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if !self.tracking { return Err("cannot guide when tracking disabled".into()); }

        self.set_motion(Axis::Primary, axis1_speed + if self.tracking { SIDEREAL_RATE } else { RadPerSec(0.0) })?;
        self.set_motion(Axis::Secondary, axis2_speed)?;

        Ok(())
    }

    fn slew(&mut self, axis: Axis, speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        match axis {
            Axis::Primary => self.set_motion(axis, speed + if self.tracking { SIDEREAL_RATE } else { RadPerSec(0.0) })?,
            Axis::Secondary => self.set_motion(axis, speed)?
        }

        Ok(())
    }

    fn slewing_rate_supported(&self, speed: RadPerSec) -> bool {
        speed <= MAX_SPEED
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.stop_motion(Axis::Primary)?;
        self.stop_motion(Axis::Secondary)
    }
}

impl Drop for SkyWatcher {
    fn drop(&mut self) {
        let _ = self.stop_motion(Axis::Primary);
        let _ = self.stop_motion(Axis::Secondary);
    }
}

fn skywatcher_hex_str_to_u32(s: &[u8]) -> Result<u32, Box<dyn Error>> {
    if s.len() == 0 || (s.len() & 1 == 1) {
        return Err("invalid response".into())
    }

    let mut result: u32 = 0;
    for i in (0..=s.len() - 2).step_by(2) {
        let two_hex_digits = std::str::from_utf8(&s[i..i + 2])?;
        result += u32::from_str_radix(&two_hex_digits, 16)? << (i / 2 * 8);
    }

    Ok (result)
}

fn u32_to_skywatcher_hex_str(i: u32) -> String {
    format!("{:02X}{:02X}{:02X}", i & 0xFF, (i >> 8) & 0xFF, (i >> 16) & 0xFF)
}

fn extract_hex_number(mount_response: &[u8]) -> Vec<u8> {
    mount_response[1..1 + mount_response.len() - 2].to_vec()
}

fn send_cmd_and_get_reply(serial_port: &mut Box<dyn serialport::SerialPort>, axis: Axis, opcode: Opcode, params: &str)
-> Result<Vec<u8>, Box<dyn Error>> {
    let command_str = format!(
        "{}{}{}{}{}",
        command::START_CHAR_OUT as char,
        opcode.as_char(),
        axis.as_char(),
        params,
        command::END_CHAR as char
    ).into_bytes();

    serial_port.write_all(&command_str)?;

    let mut buf = vec![];
    let mut reply_received = false;
    while !reply_received {
        buf.push(0);
        let blen = buf.len();
        serial_port.read_exact(&mut buf[blen - 1..blen])?;
        if buf[blen - 1] == command::END_CHAR as u8 {
            reply_received = true;
        }
    }

    if buf[0] != command::START_CHAR_IN as u8 {
        Err("invalid response".into())
    } else {
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_sw_hex_str_parse() {
        assert_eq!(0x12ABCD, skywatcher_hex_str_to_u32(b"CDAB12").unwrap());
        assert_eq!(0x1A, skywatcher_hex_str_to_u32(b"1A").unwrap());
        assert_eq!(0xCCDD, skywatcher_hex_str_to_u32(b"DDCC").unwrap());
    }

    #[test]
    fn given_empty_string_fail() {
        assert!(skywatcher_hex_str_to_u32(b"").is_err());
    }

    #[test]
    fn given_odd_length_string_fail() {
        assert!(skywatcher_hex_str_to_u32(b"123").is_err());
    }

    #[test]
    fn given_non_hex_string_fail() {
        assert!(skywatcher_hex_str_to_u32(b"12%6").is_err());
    }

    #[test]
    fn given_u32_format() {
        assert_eq!("CDAB12", u32_to_skywatcher_hex_str(0x12ABCD));
        assert_eq!("1A0000", u32_to_skywatcher_hex_str(0x1A));
        assert_eq!("DDCC00", u32_to_skywatcher_hex_str(0xCCDD));
        assert_eq!("230100", u32_to_skywatcher_hex_str(0x123));
    }
}
