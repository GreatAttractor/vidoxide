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
    focuser::{DegC, Focuser, Position, PositionRange, Speed, SpeedRange, State, TargetPosition},
    utils,
    utils::{InvalidResponseTreatment, ResponseType}
};
use std::error::Error;

pub enum Connection {
    Serial{ device: String },
    TcpIp{ address: std::net::SocketAddr }
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

            Connection::TcpIp { address } => Device::TcpIp(std::net::TcpStream::connect(address)?)
        };

        Ok(FocusCube3{
            connection_str: match connection {
                Connection::Serial{ device } => device,
                Connection::TcpIp{ address } => address.to_string()
            },
            device
        })
    }
}

// TODO simplify this (via enum_dispatch?)
macro_rules! do_send {
    ($io:expr, $cmd:expr, $resp_type:expr, $inv_resp_tr:expr) => {
        utils::send_cmd_and_get_reply($io, $cmd, $resp_type, $inv_resp_tr).map(|_| ())
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
        unimplemented!()
    }

    fn move_(&mut self, target: TargetPosition, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            send_cmd!(
                self,
                format!("FM:{}\n", if let TargetPosition::Absolute(pos) = target { pos.0 } else { unimplemented!() }),
                ResponseType::None,
                InvalidResponseTreatment::Ignore { log_warning: true }
            )
        }
    }

    fn sync(&mut self, current_pos: Position) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        send_cmd!(
            self,
            "FH\n".into(),
            ResponseType::None,
            InvalidResponseTreatment::Ignore { log_warning: true }
        )
    }
}
