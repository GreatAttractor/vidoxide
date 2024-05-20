//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount module.
//!

#[cfg(feature = "mount_ascom")]
mod ascom;
mod ioptron;
mod simulator;
mod skywatcher;
mod zwo;

use std::error::Error;

#[derive(Copy, Clone)]
pub enum Axis { Primary, Secondary }

#[derive(strum_macros::EnumIter)]
pub enum MountConnection {
    /// Contains device name of serial port.
    SkyWatcherSerial(String),

    // Contains device name of serial port.
    IoptronSerial(String),

    // Contains device name of serial port.
    ZWOSerial(String),

    /// Contains ProgID of telescope (e.g., "EQMOD.Telescope").
    #[cfg(feature = "mount_ascom")]
    Ascom(String),

    Simulator
}

pub const SECONDS_PER_DAY: f64 = 86164.09065;

#[derive(Copy, Clone, PartialEq)]
pub struct RadPerSec(pub f64);

impl RadPerSec {
    pub fn is_zero(&self) -> bool { self.0 == 0.0 }

    pub fn abs(&self) -> RadPerSec { RadPerSec(self.0.abs()) }
}

impl std::ops::Mul<f64> for RadPerSec {
    type Output = Self;

    fn mul(self, x: f64) -> RadPerSec {
        RadPerSec(self.0 * x)
    }
}

impl std::ops::Mul<RadPerSec> for f64 {
    type Output = RadPerSec;

    fn mul(self, x: RadPerSec) -> RadPerSec {
        RadPerSec(self * x.0)
    }
}

impl std::ops::Add for RadPerSec {
    type Output = Self;

    fn add(self, x: RadPerSec) -> RadPerSec {
        RadPerSec(self.0 + x.0)
    }
}

impl std::cmp::PartialOrd<RadPerSec> for RadPerSec {
    fn partial_cmp(&self, other: &RadPerSec) -> Option<std::cmp::Ordering> {
        if self.0 < other.0 { Some(std::cmp::Ordering::Less) }
        else if self.0 > other.0 { Some(std::cmp::Ordering::Greater) }
        else { Some(std::cmp::Ordering::Equal) }
    }
}

impl std::ops::Neg for RadPerSec {
    type Output = RadPerSec;

    fn neg(self) -> Self::Output { RadPerSec(-self.0) }
}

impl std::ops::DivAssign<f64> for RadPerSec {
    fn div_assign(&mut self, rhs: f64) {
        self.0 /= rhs;
    }
}

pub const SIDEREAL_RATE: RadPerSec = RadPerSec(2.0 * std::f64::consts::PI / SECONDS_PER_DAY);

pub enum SlewSpeed {
    Specific(RadPerSec),
    Max(bool) // `true` means positive direction, `false` - negative
}

impl SlewSpeed {
    pub fn zero() -> SlewSpeed { SlewSpeed::Specific(RadPerSec(0.0)) }
    pub fn is_zero(&self) -> bool {
        match self { SlewSpeed::Specific(s) => s.0 == 0.0, _ => false }
    }
    pub fn positive(&self) -> bool {
        match self {
            SlewSpeed::Specific(s) => s.0 > 0.0,
            SlewSpeed::Max(positive) => *positive
        }
    }
}

pub trait Mount {
    #[must_use]
    fn get_info(&self) -> String;

    #[must_use]
    fn set_tracking(&mut self, enabled: bool) -> Result<(), Box<dyn Error>>;

    #[must_use]
    fn guide(&mut self, axis1_speed: RadPerSec, axis2_speed: RadPerSec) -> Result<(), Box<dyn Error>>;

    #[must_use]
    /// Specify zero speed to stop slewing (in any case, tracking is not affected).
    fn slew(&mut self, axis: Axis, speed: SlewSpeed) -> Result<(), Box<dyn Error>>;

    #[must_use]
    fn slewing_speed_supported(&self, speed: RadPerSec) -> bool;

    fn stop(&mut self) -> Result<(), Box<dyn Error>>;

    /// Only implemented by mount simulator.
    fn set_mount_simulator_data(&mut self, _mount_simulator_data: crate::MountSimulatorData) {}
}

pub fn connect_to_mount(connection: MountConnection) -> Result<Box<dyn Mount>, Box<dyn Error>> {
    match connection {
        MountConnection::SkyWatcherSerial(device) => {
            Ok(Box::new(skywatcher::SkyWatcher::new(&device)?))
        },

        MountConnection::IoptronSerial(device) => {
            Ok(Box::new(ioptron::Ioptron::new(&device)?))
        },

        MountConnection::ZWOSerial(device) => {
            Ok(Box::new(zwo::ZWO::new(&device)?))
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
