//
// Copyright (c) 2024 Diego Dompe <ddompe@gmail.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ZWO mount direct serial connection driver.
//!
//! Based on ZWO communication protocol spec 1.8
//! https://github.com/indigo-astronomy/indigo/blob/master/indigo_drivers/mount_asi/docs/ZWO%20Mount%20Serial%20Communication%20Protocol_v1.8.pdf
//! Tested with AM3 and AM5
//!

use crate::mount::{Axis, Mount, SlewSpeed, RadPerSec, SIDEREAL_RATE};
use std::error::Error;
use std::sync::atomic::Ordering;

pub struct ZWO {
    model: String,
    device: String,
    tracking: bool,
    serial_port: Box<dyn serialport::SerialPort>
}

pub const END_CHAR: char = '#';

#[derive(Debug)]
enum ResponseType {
    None,
    EndsWith(char),
    NumCharsReceived(usize),
    CharsReceived(String)
}

struct SupportedSlewingSpeed {
    id: char,
    speed: RadPerSec
}

/// Multiplies of sidereal rate.
const SUPPORTED_SLEWING_SPEEDS: [SupportedSlewingSpeed; 10] = [
    SupportedSlewingSpeed{ id: '0', speed: RadPerSec(   0.25 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '1', speed: RadPerSec(   0.5 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '2', speed: RadPerSec(   1.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '3', speed: RadPerSec(   2.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '4', speed: RadPerSec(   4.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '5', speed: RadPerSec(   8.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '6', speed: RadPerSec(  20.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '7', speed: RadPerSec(  60.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '8', speed: RadPerSec( 720.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '9', speed: RadPerSec(1440.0 * SIDEREAL_RATE.0)}, // max possible speed
];

fn choose_slewing_speed(requested: &SlewSpeed) -> Option<&'static SupportedSlewingSpeed> {
    match requested {
        SlewSpeed::Max(_) => SUPPORTED_SLEWING_SPEEDS.last(),

        SlewSpeed::Specific(s) => {
            let is_close = |req: f64, actual: f64| { let rel = req.abs() / actual; rel >= 0.99 && rel <= 1.01 };

            for sss in SUPPORTED_SLEWING_SPEEDS.iter().take(SUPPORTED_SLEWING_SPEEDS.len() - 1) {
                if is_close(s.0, sss.speed.0) { return Some(sss); }
            }

            None
        }
    }
}

impl ZWO {
    #[must_use]
    pub fn new(device: &str) -> Result<ZWO, Box<dyn Error>> {
        let mut serial_port = serialport::new(device, 9600)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(50))
            .open()?;


        let model = if let Ok(chars) = send_cmd_and_get_reply(
            &mut serial_port,
            ":GVP#".into(),
            ResponseType::EndsWith(END_CHAR)
        ) {
            if let Ok(s) = String::from_utf8(chars) {
                match s.as_str() {
                    "AM3" => "AM3".into(),
                    "AM5" => "AM5".into(),
                    _ => format!("(unknown - {})", s)
                }
            } else {
                "(unknown) string".into()
            }
        } else {
            "(unknown) command error".into()
        };

        // Disable tracking
        send_cmd_and_get_reply(&mut serial_port, ":Td#".into(), ResponseType::None).map(|_| ())?;

        Ok(ZWO{
            model: model,
            device: device.to_string(),
            tracking: false,
            serial_port
        })
    }
}

impl Drop for ZWO {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl Mount for ZWO {
    fn get_info(&self) -> String {
        format!("ZWO {} on {}", self.model, self.device)
    }


    fn set_tracking(&mut self, enabled: bool) -> Result<(), Box<dyn Error>> {
        match send_cmd_and_get_reply(
            &mut self.serial_port,
            format!(":T{}#",  if enabled { "e" } else { "d" }),
            ResponseType::CharsReceived("1".into())
        ) {
            Ok(_) => { self.tracking = enabled; Ok(()) },
            Err(e) => Err(e)
        }
    }

    fn guide(&mut self, axis1_speed: RadPerSec, axis2_speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if !(axis1_speed.is_zero() && axis2_speed.is_zero()) && !self.tracking {
            return Err("cannot guide when tracking is disabled".into());
        }

        const MIN_SIDEREAL_MULT: f64 = 0.1;

        let a1_s = (axis1_speed.0.abs() / SIDEREAL_RATE.0).max(MIN_SIDEREAL_MULT);
        let a2_s = (axis2_speed.0.abs() / SIDEREAL_RATE.0).max(MIN_SIDEREAL_MULT);

        if axis1_speed.is_zero() {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mge0000#".into(), ResponseType::None).map(|_| ())?;
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgw0000#".into(), ResponseType::None).map(|_| ())?;
        } else {
            if a1_s > 0.9 {
                return Err("unsupported primary axis guiding speed".into());
            }
        }

        if axis2_speed.is_zero() {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgn0000#".into(), ResponseType::None).map(|_| ())?;
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgs0000#".into(), ResponseType::None).map(|_| ())?;
        } else {
            if a2_s > 0.9 {
                return Err("unsupported secondary axis guiding speed".into());
            }
        }

        send_cmd_and_get_reply(
            &mut self.serial_port,
            format!(":Rg{:.2}#", a1_s.max(0.9)),
            ResponseType::None
        ).map(|_| ())?;

        if axis1_speed.0 > 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mge0500#".into(), ResponseType::None).map(|_| ())?;
        } else if axis1_speed.0 < 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgw0500#".into(), ResponseType::None).map(|_| ())?;
        }

        send_cmd_and_get_reply(
            &mut self.serial_port,
            format!(":Rg{:.2}#", a2_s.max(0.9)),
            ResponseType::None
        ).map(|_| ())?;

        if axis2_speed.0 > 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgn0500#".into(), ResponseType::None).map(|_| ())?;
        } else if axis2_speed.0 < 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":Mgs0500#".into(), ResponseType::None).map(|_| ())?;
        }

        Ok(())
    }

    fn slew(&mut self, axis: Axis, speed: SlewSpeed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            return send_cmd_and_get_reply(
                &mut self.serial_port,
                ":Q#".to_string(),
                ResponseType::None
            ).map(|_| ());
        }

        match choose_slewing_speed(&speed) {
            Some(s) => {
                send_cmd_and_get_reply(
                    &mut self.serial_port,
                    format!(":R{}#", s.id),
                    ResponseType::None
                ).map(|_| ())?;

                send_cmd_and_get_reply(
                    &mut self.serial_port,
                    format!(
                        ":M{}#",
                        match axis {
                            Axis::Primary => if speed.positive() { "e" } else { "w" },
                            Axis::Secondary => if speed.positive() { "n" } else { "s" }
                        }
                    ),
                    ResponseType::None
                ).map(|_| ())?;

                Ok(())
            },

            None => Err("unsupported slewing speed".into())
        }
    }

    fn slewing_speed_supported(&self, speed: RadPerSec) -> bool {
        choose_slewing_speed(&SlewSpeed::Specific(speed)).is_some()
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.set_tracking(false)?;
        // stop all slewing
        send_cmd_and_get_reply(
            &mut self.serial_port,
            ":Q#".into(),
            ResponseType::None
        ).map(|_| ())
    }
}


fn send_cmd_and_get_reply<T: std::io::Read + std::io::Write>(
    device: &mut T,
    cmd: String,
    response_type: ResponseType,
) -> Result<Vec<u8>, Box<dyn Error>> {
    device.write_all(&cmd.clone().into_bytes())?;

    match &response_type {
        ResponseType::CharsReceived(chars) => { if chars.is_empty() { return Ok(vec![]); } },
        ResponseType::NumCharsReceived(0) | ResponseType::None => { return Ok(vec![]); }
        _ => ()
    }

    let mut reply_error = false;

    let mut buf = vec![];
    let mut reply_received = false;
    while !reply_received {
        buf.push(0);
        if buf.len() > 1024 { return Err("response has too many characters".into()); }
        let blen = buf.len();
        if device.read_exact(&mut buf[blen - 1..blen]).is_err() {
            reply_error = true;
            break;
        }
        reply_received = match response_type {
            ResponseType::EndsWith(ch) => buf[blen - 1] == ch as u8,
            ResponseType::NumCharsReceived(num) => buf.len() == num,
            ResponseType::CharsReceived(ref chars) => buf.len() == chars.len(),
            ResponseType::None => unreachable!()
        };
    }

    match &response_type {
        ResponseType::CharsReceived(chars) => if &buf != chars.as_bytes() { reply_error = true; },
        ResponseType::EndsWith(ch) => { buf.pop(); },
        _ => ()
    }

    if reply_error {
        let message = format!("cmd \"{}\" failed to get expected response: {:?}", cmd, response_type);
          return Err(message.into());
    }

    Ok(buf)
}
