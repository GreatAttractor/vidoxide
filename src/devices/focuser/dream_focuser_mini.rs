//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini driver.
//!

use crate::devices::{
    focuser::{Focuser, Position, PositionRange, Speed, SpeedRange, State},
    utils
};
use std::{convert::TryInto, error::Error};

#[derive(Copy, Clone)]
enum Command {
    Stop,
    ReadPosition,
    SetPosition,
    CalibrateToPosition,
    Move,
}

impl Command {
    fn opcode(&self) -> u8 {
        (match self {
            Command::Stop =>                'H',
            Command::ReadPosition =>        'P',
            Command::SetPosition =>         'M',
            Command::CalibrateToPosition => 'Z',
            Command::Move =>                'R'
        }) as u8
    }
}

const PREAMBLE: u8 = 'M' as u8;

pub struct DreamFocuserMini {
    connection_str: String,
    serial_port: Box<dyn serialport::SerialPort>
}

impl DreamFocuserMini {
    /// Creates a DreamFocuser mini instance.
    ///
    /// # Parameters
    ///
    /// * `device` - System device name to use for connecting to the focuser,
    ///     e.g., "COM3" on Windows or "/dev/ttyUSB0" on Linux.
    ///
    #[must_use]
    pub fn new(device: &str) -> Result<DreamFocuserMini, Box<dyn Error>> {
        let serial_port = serialport::new(device, 115200)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(std::time::Duration::from_millis(100))
            .open()?;

        Ok(DreamFocuserMini{ connection_str: device.into(), serial_port })
    }
}

impl Focuser for DreamFocuserMini {
    #[must_use]
    fn info(&self) -> String {
        format!("DreamFocuser mini on {}", self.connection_str)
    }

    fn pos_range(&mut self) -> Result<PositionRange, Box<dyn Error>> {
        Ok(PositionRange{ min: Position(i32::MIN), max: Position(i32::MAX) })
    }

    fn speed_range(&mut self) -> Result<SpeedRange, Box<dyn Error>> {
        Ok(SpeedRange{ min: Speed(0.2), max: Speed(10.0) })
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        Ok(State{ pos: Position(0), moving: Some(false), temperature: None })
    }

    // TODO: use proper target position handling
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop()
        } else {
            let mut speed_i16 = (speed.get() * 5.0) as i16;
            if target.0 < 0 { speed_i16 = -speed_i16; }
            send_cmd_and_get_reply(
                &mut self.serial_port,
                Command::Move,
                &[(speed_i16 & 0xFF) as u8, (speed_i16 >> 8) as u8, 0, 0],
                0
            ).map(|_| ())
        }
    }

    fn sync(&mut self, current_pos: Position) -> Result<(), Box<dyn Error>> {
        // send_cmd!(
        //     self,
        //     format!("FN:{}\n", current_pos.0),
        //     ResponseType::None,
        //     InvalidResponseTreatment::Fail
        // ).map(|_| ())

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        send_cmd_and_get_reply(&mut self.serial_port, Command::Stop,  &[0u8; 4], 0).map(|_| ())
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
