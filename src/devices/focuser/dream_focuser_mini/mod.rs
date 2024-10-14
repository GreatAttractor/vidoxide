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

#[cfg(feature = "bluetooth")]
mod dfmini_bluetooth;
mod dfmini_usb;

#[cfg(feature = "bluetooth")]
use dfmini_bluetooth::BluetoothExecutor;
use dfmini_usb::UsbExecutor;

use crate::devices::focuser::{Focuser, Position, PositionRange, Speed, SpeedRange, State};
use std::error::Error;
#[cfg(feature = "bluetooth")]
use std::rc::Rc;

pub enum Connection {
    USB { device: String },
    #[cfg(feature = "bluetooth")]
    Bluetooth { mac_addr: String}
}

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

trait CmdExecutor {
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>>;

    fn stop(&mut self) -> Result<(), Box<dyn Error>>;

    fn state(&mut self) -> Result<State, Box<dyn Error>>;
}

pub struct DreamFocuserMini {
    connection_str: String,
    executor: Box<dyn CmdExecutor>
}

fn to_raw_speed(speed: Speed) -> i16 {
    (speed.get() * 5.0) as i16
}

impl DreamFocuserMini {
    #[must_use]
    pub fn new(
        connection: Connection,
        #[cfg(feature = "bluetooth")]
        tokio_rt: Rc<tokio::runtime::Runtime>
    ) -> Result<DreamFocuserMini, Box<dyn Error>> {
        match connection {
                Connection::USB{ ref device } => {
                    Ok(DreamFocuserMini{ connection_str: device.into(), executor: UsbExecutor::new(device)? })
                },

                #[cfg(feature = "bluetooth")]
                Connection::Bluetooth{ ref mac_addr } => {
                    Ok(DreamFocuserMini{
                        connection_str: mac_addr.into(),
                        executor: BluetoothExecutor::new(mac_addr, tokio_rt)?
                    })
                }
        }
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
        self.executor.state()
    }

    // TODO: use proper target position handling
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        self.executor.move_(target, speed)
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
        self.executor.stop()
    }
}
