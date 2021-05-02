//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount module.
//!

pub mod skywatcher;

#[derive(Copy, Clone)]
pub enum Axis { RA, Dec }

#[derive(Debug)]
pub enum MountError {
    CannotConnect,
    SkyWatcherError(skywatcher::SWError)
}

pub enum MountConnection {
    SkyWatcherSerial(String)
}

pub const SECONDS_PER_DAY: f64 = 86164.09065;

pub const SIDEREAL_RATE: f64 = 2.0 * std::f64::consts::PI / SECONDS_PER_DAY;

pub trait Mount {
    #[must_use]
    fn get_info(&self) -> Result<String, MountError>;

    /// Sets axis motion speed.
    ///
    /// # Parameters
    ///
    /// * `axis` - Axis to set motion for.
    /// * `speed` - Signed speed in radians per second.
    ///
    #[must_use]
    fn set_motion(&mut self, axis: Axis, speed: f64) -> Result<(), MountError>;

    #[must_use]
    fn stop_motion(&mut self, axis: Axis) -> Result<(), MountError>;

    #[must_use]
    fn get_motion_speed(&self, axis: Axis) -> Result<f64, MountError>;
}

pub fn connect_to_mount(connection: MountConnection) -> Result<Box<dyn Mount>, MountError> {
    match connection {
        MountConnection::SkyWatcherSerial(device) => {
            Ok(Box::new(skywatcher::SkyWatcher::new(&device)?))
        }
    }
}