//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! PegasusAstro FocusCube3 driver.
//!

use crate::devices::{
    focuser::{DegC, Focuser, Position, PositionRange, Speed, SpeedRange, State},
    utils,
    utils::{InvalidResponseTreatment, ResponseType}
};
use std::error::Error;

pub enum Connection {
    Serial{ device: String },
    TcpIp{ address: String }
}

enum Device {
    Serial(Box<dyn serialport::SerialPort>),
    TcpIp(std::net::TcpStream)
}

pub struct FocusCube3 {
    connection_str: String,
    device: Device // TODO: use Box<dyn Read+Write> when trait upcasting is stabilized
}

impl FocusCube3 {
    pub fn new(connection: Connection) -> Result<FocusCube3, Box<dyn Error>> {
        let device = match connection {
            Connection::Serial { ref device } => {
                Device::Serial(serialport::new(device, 115200)
                    .data_bits(serialport::DataBits::Eight)
                    .flow_control(serialport::FlowControl::None)
                    .parity(serialport::Parity::None)
                    .stop_bits(serialport::StopBits::One)
                    .timeout(std::time::Duration::from_millis(100))
                    .open()?)
            },

            Connection::TcpIp { ref address } => {
                let mut stream = std::net::TcpStream::connect(address)?;
                // authenticate with default password
                utils::send_cmd_and_get_reply(
                    &mut stream,
                    "12345678\n".into(),
                    ResponseType::EndsWith('\n'),
                    InvalidResponseTreatment::Ignore { log_warning: true }
                )?;

                Device::TcpIp(stream)
            }
        };

        let mut fc3 = FocusCube3{
            connection_str: match connection {
                Connection::Serial{ device } => device,
                Connection::TcpIp{ address } => address
            },
            device
        };

        fc3.sync(Position(500_000))?;

        Ok(fc3)
    }
}

// TODO simplify this (via enum_dispatch?)
macro_rules! do_send {
    ($io:expr, $cmd:expr, $resp_type:expr, $inv_resp_tr:expr) => {
        utils::send_cmd_and_get_reply($io, $cmd, $resp_type, $inv_resp_tr) //.map(|_| ())
    };
}

macro_rules! send_cmd {
    ($focuser:expr, $cmd:expr, $resp_type:expr, $inv_resp_tr:expr) => {
        match &mut $focuser.device {
            Device::Serial(io) => do_send!(io, $cmd, $resp_type, $inv_resp_tr),
            Device::TcpIp(io) => do_send!(io, $cmd, $resp_type, $inv_resp_tr),
        }
    };
}

impl Focuser for FocusCube3 {
    #[must_use]
    fn info(&self) -> String {
        format!("FocusCube3 on {}", self.connection_str)
    }

    fn pos_range(&mut self) -> Result<PositionRange, Box<dyn Error>> {
        Ok(PositionRange{ min: Position(0), max: Position(1317500) })
    }

    fn speed_range(&mut self) -> Result<SpeedRange, Box<dyn Error>> {
        Ok(SpeedRange{ min: Speed(1.0), max: Speed(400.0) })
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        let reply = send_cmd!(
            self,
            "FA\n".into(),
            ResponseType::EndsWith('\n'),
            InvalidResponseTreatment::Ignore { log_warning: true }
        )?;
        let reply = std::str::from_utf8(&reply)?;

        let parts: Vec<&str> = reply.split(':').collect();
        if parts.len() < 6 || parts[0] != "FC3" { return Err(format!("invalid response: {}", reply).into()); }

        let pos = Position(parts[1].parse::<i32>()?);
        let moving = Some(if parts[2].chars().nth(0).unwrap() == '0' { false } else { true });
        // TODO if the sensor is not connected, returns 0.0 - add some logic to detect recent other values and decide
        // if we should return `Some` or `None`.
        let temperature = Some(DegC(parts[3].parse::<f64>()?));

        Ok(State{ pos, moving, temperature })
    }

    fn begin_move(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            send_cmd!(
                self,
                format!("FM:{}\n", target.0),
                ResponseType::None,
                InvalidResponseTreatment::Ignore { log_warning: true }
            ).map(|_| ())
        }
    }

    fn sync(&mut self, current_pos: Position) -> Result<(), Box<dyn Error>> {
        send_cmd!(
            self,
            format!("FN:{}\n", current_pos.0),
            ResponseType::None,
            InvalidResponseTreatment::Ignore { log_warning: true }
        ).map(|_| ())
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        send_cmd!(
            self,
            "FH\n".into(),
            ResponseType::None,
            InvalidResponseTreatment::Ignore { log_warning: true }
        ).map(|_| ())
    }
}
