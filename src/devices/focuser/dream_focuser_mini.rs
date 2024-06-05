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

use crate::devices::focuser::Focuser;
use std::error::Error;

pub struct DreamFocuserMini {
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
        Ok(DreamFocuserMini{})
    }
}

// impl Focuser for DreamFocuserMini {
//     fn info(&self) -> String {
//         "DreamFocuser mini".into()
//     }

//     fn move_(&mut self, speed: super::Speed) -> Result<(), Box<dyn Error>> {
//         log::info!("moving with speed {:.2}", speed.get());
//         Err("not yet implemented".into())
//     }

//     fn position(&mut self) -> Result<super::Position, Box<dyn Error>> {
//         Err("not yet implemented".into())
//     }
// }
