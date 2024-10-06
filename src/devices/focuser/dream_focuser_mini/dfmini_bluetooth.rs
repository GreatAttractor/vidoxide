//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini Bluetooth executor.
//!

use crate::devices::{
    focuser::dream_focuser_mini::{CmdExecutor, Command, Position, Speed, State},
    utils
};
use std::{convert::TryInto, error::Error};

pub struct BluetoothExecutor {
    serial_port: Box<dyn serialport::SerialPort>,
}

impl BluetoothExecutor {
    pub fn new(device: &str) -> Result<Box<dyn CmdExecutor>, Box<dyn Error>> {
        let serial_port = serialport::new(device, 115200)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(100))
            .open()?;

            Ok(Box::new(BluetoothExecutor{ serial_port }))
    }
}

impl CmdExecutor for BluetoothExecutor {
    // TODO: use proper target position handling
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            let mut speed_i16 = (speed.get() * 5.0) as i16;
            if target.0 < 0 { speed_i16 = -speed_i16; }
            send_cmd(
                &mut self.serial_port,
                &[Command::Move.opcode(), (speed_i16 & 0xFF) as u8, (speed_i16 >> 8) as u8, 0, 0]
            )
        }
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        send_cmd(&mut self.serial_port, &[Command::Stop.opcode()])
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        Ok(State{ pos: Position(0), moving: Some(false), temperature: None })
    }
}

fn send_cmd<T: std::io::Read + std::io::Write>(
    device: &mut T,
    payload: &[u8]
) -> Result<(), Box<dyn Error>> {
    utils::send_cmd_and_get_reply(
        device,
        &payload,
        utils::ResponseType::None,
        utils::InvalidResponseTreatment::Fail
    )?;

    Ok(())
}
