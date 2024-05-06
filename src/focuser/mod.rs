//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope focuser module.
//!

mod dream_focuser_mini;

use std::error::Error;

#[derive(strum_macros::EnumIter)]
pub enum Connection {
    /// Contains device name of serial port.
    DreamFocuserMini(String),
}

#[derive(Copy, Clone)]
pub struct Position(pub i32);

#[derive(Copy, Clone)]
pub struct Speed(f64);

impl Speed {
    pub fn new(value: f64) -> Speed {
        if value >= -1.0 && value <= 1.0 {
            Speed(value)
        } else {
            panic!("invalid focuser speed: {} (expected value between -1.0 and 1.0)", value);
        }
    }

    pub fn get(&self) -> f64 { self.0 }

    pub fn zero() -> Speed { Speed(0.0) }

    pub fn is_zero(&self) -> bool { self.0 == 0.0 }
}

pub trait Focuser {
    #[must_use]
    fn info(&self) -> String;

    fn position(&mut self) -> Result<Position, Box<dyn Error>>;

    fn move_(&mut self, speed: Speed) -> Result<(), Box<dyn Error>>;
}

pub fn connect_to_focuser(connection: Connection) -> Result<Box<dyn Focuser>, Box<dyn Error>> {
    match connection {
        Connection::DreamFocuserMini(device) => {
            Ok(Box::new(dream_focuser_mini::DreamFocuserMini::new(&device)?))
        },
    }
}
