//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount module.
//!

#[cfg(feature = "mount_ascom")]
mod ascom;
mod simulator;
mod skywatcher;

#[derive(Copy, Clone)]
pub enum Axis { Primary, Secondary }

#[derive(Debug)]
pub enum MountError {
    CannotConnect,

    SkyWatcherError(skywatcher::SWError),

    #[cfg(feature = "mount_ascom")]
    AscomError(ascom::AscomError),

    SimulatorError(simulator::SimulatorError)
}

#[derive(strum_macros::EnumIter)]
pub enum MountConnection {
    /// Contains device name of serial port.
    SkyWatcherSerial(String),

    /// Contains ProgID of telescope (e.g., "EQMOD.Telescope").
    #[cfg(feature = "mount_ascom")]
    Ascom(String),

    Simulator
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

    /// Only implemented by mount simulator.
    fn set_mount_simulator_data(&mut self, _mount_simulator_data: crate::MountSimulatorData) {}
}

pub fn connect_to_mount(connection: MountConnection) -> Result<Box<dyn Mount>, MountError> {
    match connection {
        MountConnection::SkyWatcherSerial(device) => {
            Ok(Box::new(skywatcher::SkyWatcher::new(&device)?))
        },

        #[cfg(feature = "mount_ascom")]
        MountConnection::Ascom(progid) => {
            Ok(Box::new(ascom::Ascom::new(&progid)?))
        },

        MountConnection::Simulator => {
            Ok(Box::new(simulator::Simulator::new()))
        }
    }
}
