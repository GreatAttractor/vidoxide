//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! iOptron mount direct serial connection driver.
//!
//! Based on "iOptron® Mount RS-232 Command Language" (v. 3.10 2021-01-04).
//!

use crate::mount::{Axis, Mount, RadPerSec, SIDEREAL_RATE};
use std::error::Error;

// TODO: if guiding is active, does stop tracking cancels guiding as well?

mod command
{
    pub const END_CHAR: u8 = '#' as u8;
}

#[derive(Debug)]
enum ResponseType {
    None,
    EndsWith(char),
    NumCharsReceived(usize),
    CharsReceived(String)
}

pub struct Ioptron {
    model: String,
    device: String,
    serial_port: Box<dyn serialport::SerialPort>,
    tracking: bool
}

struct SupportedSlewingSpeed {
    id: char,
    speed: RadPerSec
}

/// Multiplies of sidereal rate.
const SUPPORTED_SLEWING_SPEEDS: [SupportedSlewingSpeed; 5] = [
    SupportedSlewingSpeed{ id: '1', speed: RadPerSec( 1.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '2', speed: RadPerSec( 2.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '3', speed: RadPerSec( 8.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '4', speed: RadPerSec(16.0 * SIDEREAL_RATE.0)},
    SupportedSlewingSpeed{ id: '5', speed: RadPerSec(64.0 * SIDEREAL_RATE.0)},
];

fn choose_slewing_speed(requested: RadPerSec) -> Option<&'static SupportedSlewingSpeed> {
    let is_close = |req: f64, actual: f64| { let rel = req.abs()/actual; rel >= 0.99 && rel <= 1.01 };

    for sss in &SUPPORTED_SLEWING_SPEEDS {
        if is_close(requested.0, sss.speed.0) { return Some(sss); }
    }

    None
}

impl Ioptron {
    /// Creates an iOptron mount instance.
    ///
    /// # Parameters
    ///
    /// * `device` - System device name to use for connecting to the mount,
    ///     e.g., "COM3" on Windows or "/dev/ttyUSB0" on Linux.
    ///
    #[must_use]
    pub fn new(device: &str) -> Result<Ioptron, Box<dyn Error>> {
        let mut serial_port = serialport::new(device, 115200)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(50))
            .open()?;

        let model = if let Ok(chars) = send_cmd_and_get_reply(
            &mut serial_port,
            ":MountInfo#".into(),
            ResponseType::NumCharsReceived(4),
            false
        ) {
            if let Ok(s) = String::from_utf8(chars) {
                match s.as_str() {
                    "0026" => "CEM26".into(),
                    "0027" => "CEM26-EC".into(),
                    "0028" => "GEM28".into(),
                    "0029" => "GEM28-EC".into(),
                    "0040" => "CEM40(G)".into(),
                    "0041" => "CEM40(G)-EC".into(),
                    "0043" => "GEM45(G)".into(),
                    "0044" => "GEM45(G)-EC".into(),
                    "0070" => "CEM70(G)".into(),
                    "0071" => "CEM70(G)-EC".into(),
                    "0120" => "CEM120".into(),
                    "0121" => "CEM120-EC".into(),
                    "0122" => "CEM120-EC2".into(),
                    "0066" => "HAE69B".into(),
                    _ => format!("(unknown - {})", s)
                }
            } else {
                "(unknown)".into()
            }
        } else {
            "(unknown)".into()
        };

        Ok(Ioptron{
            model,
            device: device.to_string(),
            serial_port,
            tracking: false
        })
    }
}

impl Drop for Ioptron {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl Mount for Ioptron {
    #[must_use]
    fn get_info(&self) -> String {
        format!("iOptron {} on {}", self.model, self.device)
    }

    #[must_use]
    fn set_tracking(&mut self, enabled: bool) -> Result<(), Box<dyn Error>> {
        match send_cmd_and_get_reply(
            &mut self.serial_port,
            format!(":ST{}#",  if enabled { "1" } else { "0" }),
            ResponseType::CharsReceived("1".into()),
            true
        ) {
            Ok(_) => { self.tracking = enabled; Ok(()) },
            Err(e) => Err(e)
        }
    }

    #[must_use]
    fn guide(&mut self, axis1_speed: RadPerSec, axis2_speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if !(axis1_speed.is_zero() && axis2_speed.is_zero()) && !self.tracking {
            return Err("cannot guide when tracking is disabled".into());
        }

        const MIN_SIDEREAL_MULT: f64 = 0.1;

        let a1_s = (axis1_speed.0.abs() / SIDEREAL_RATE.0).max(MIN_SIDEREAL_MULT);
        let a2_s = (axis2_speed.0.abs() / SIDEREAL_RATE.0).max(MIN_SIDEREAL_MULT);

        println!("using a1_s = {}, a2_s = {}", a1_s, a2_s);

        if axis1_speed.is_zero() {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZS00000#".into(), ResponseType::None, true).map(|_| ())?;
            send_cmd_and_get_reply(&mut self.serial_port, ":ZQ00000#".into(), ResponseType::None, true).map(|_| ())?;
        } else {
            if a1_s > 0.9 {
                return Err("unsupported primary axis guiding speed".into());
            }
        }

        if axis2_speed.is_zero() {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZE00000#".into(), ResponseType::None, true).map(|_| ())?;
            send_cmd_and_get_reply(&mut self.serial_port, ":ZC00000#".into(), ResponseType::None, true).map(|_| ())?;
        } else {
            if a2_s > 0.99 {
                return Err("unsupported secondary axis guiding speed".into());
            }
        }

        send_cmd_and_get_reply(
            &mut self.serial_port,
            format!(":RG{:02}{:02}#", (a1_s * 100.0).max(1.0) as i32, (a2_s * 100.0).max(1.0) as i32),
            ResponseType::CharsReceived("1".into()),
            true
        ).map(|_| ())?;

        if axis1_speed.0 > 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZQ99999#".into(), ResponseType::None, true).map(|_| ())?;
        } else if axis1_speed.0 < 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZS99999#".into(), ResponseType::None, true).map(|_| ())?;
        }

        if axis2_speed.0 > 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZC99999#".into(), ResponseType::None, true).map(|_| ())?;
        } else if axis2_speed.0 < 0.0 {
            send_cmd_and_get_reply(&mut self.serial_port, ":ZE99999#".into(), ResponseType::None, true).map(|_| ())?;
        }

        Ok(())
    }

    #[must_use]
    /// Specify zero speed to stop slewing (in any case, tracking is not affected).
    fn slew(&mut self, axis: Axis, speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            return send_cmd_and_get_reply(
                &mut self.serial_port,
                format!(":q{}#", match axis { Axis::Primary => "R", Axis::Secondary => "D" }),
                ResponseType::CharsReceived("1".into()),
                true
            ).map(|_| ());
        }

        match choose_slewing_speed(speed) {
            Some(s) => {
                send_cmd_and_get_reply(
                    &mut self.serial_port,
                    format!(":SR{}#", s.id),
                    ResponseType::CharsReceived("1".into()),
                    true
                ).map(|_| ())?;

                send_cmd_and_get_reply(
                    &mut self.serial_port,
                    format!(
                        ":m{}#",
                        match axis {
                            Axis::Primary => if speed.0 >= 0.0 { "e" } else { "w" },
                            Axis::Secondary => if speed.0 >= 0.0 { "n" } else { "s" }
                        }
                    ),
                    ResponseType::None,
                    true
                ).map(|_| ())?;

                Ok(())
            },

            None => Err("unsupported slewing speed".into())
        }
    }

    #[must_use]
    fn slewing_rate_supported(&self, speed: RadPerSec) -> bool {
        choose_slewing_speed(speed).is_some()
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.set_tracking(false)?;
        // stop all slewing
        send_cmd_and_get_reply(
            &mut self.serial_port,
            ":Q#".into(),
            ResponseType::CharsReceived("1".into()), true
        ).map(|_| ())
    }

}

fn send_cmd_and_get_reply<T: std::io::Read + std::io::Write>(
    device: &mut T,
    cmd: String,
    response_type: ResponseType,
    // HAE69B often does not return command confirmations (e.g., "1"), so let us just ignore them and log a warning
    ignore_invalid_response: bool
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

    if let ResponseType::CharsReceived(chars) = &response_type {
        if &buf != chars.as_bytes() { reply_error = true; }
    }

    if reply_error {
        let message = format!("cmd \"{}\" failed to get expected response: {:?}", cmd, response_type);
        if ignore_invalid_response {
            log::warn!("{}", message);
        } else {
            return Err(message.into());
        }
    }

    Ok(buf)
}
