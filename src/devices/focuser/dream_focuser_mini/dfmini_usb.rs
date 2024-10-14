//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini USB executor.
//!

use crate::devices::{
    focuser::dream_focuser_mini::{CmdExecutor, Command, Position, Speed, State, to_raw_speed},
    utils
};
use std::{convert::TryInto, error::Error};

const PREAMBLE: u8 = 'M' as u8;

pub struct UsbExecutor {
    serial_port: Box<dyn serialport::SerialPort>,
}

impl UsbExecutor {
    pub fn new(device: &str) -> Result<Box<dyn CmdExecutor>, Box<dyn Error>> {
        let serial_port = serialport::new(device, 115200)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(100))
            .open()?;

            Ok(Box::new(UsbExecutor{ serial_port }))
    }
}

impl CmdExecutor for UsbExecutor {
    // TODO: use proper target position handling
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            let mut raw_speed = to_raw_speed(speed);
            if target.0 < 0 { raw_speed = -raw_speed; }
            send_cmd_and_get_reply(
                &mut self.serial_port,
                Command::Move,
                &[(raw_speed & 0xFF) as u8, (raw_speed >> 8) as u8, 0, 0],
                0
            ).map(|_| ())
        }
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        send_cmd_and_get_reply(&mut self.serial_port, Command::Stop,  &[0u8; 4], 0).map(|_| ())
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        Ok(State{ pos: Position(0), moving: Some(false), temperature: None })
    }
}

fn checksum(opcode: u8, payload: &[u8; 4], address: u8) -> u8 {
    PREAMBLE
        .wrapping_add(opcode)
        .wrapping_add(payload[0])
        .wrapping_add(payload[1])
        .wrapping_add(payload[2])
        .wrapping_add(payload[3])
        .wrapping_add(address)
}

fn send_cmd_and_get_reply<T: std::io::Read + std::io::Write>(
    device: &mut T,
    command: Command,
    payload: &[u8; 4],
    address: u8
) -> Result<[u8; 4], Box<dyn Error>> {
    let out_buf: [u8; 8] = [
        PREAMBLE,
        command.opcode(),
        payload[0], payload[1], payload[2], payload[3],
        address,
        checksum(command.opcode(), payload, address)
    ];

    let in_buf = utils::send_cmd_and_get_reply(
        device,
        &out_buf,
        utils::ResponseType::NumCharsReceived(8),
        utils::InvalidResponseTreatment::Fail
    )?;

    let expected_checksum = checksum(in_buf[1], &in_buf[2..6].try_into().unwrap(), in_buf[6]);
    if in_buf[7] != expected_checksum {
        return Err("invalid checksum".into());
    }

    let mut result = [0u8; 4];
    result.copy_from_slice(&in_buf[2..6]);
    Ok(result)
}
