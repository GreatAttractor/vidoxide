//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Device drivers module.
//!

mod utils;

pub mod focuser;

#[cfg(feature = "mount_ascom")]
use crate::gui::mount_gui::ascom;
use crate::gui::{
    ioptron,
    simulator,
    skywatcher
};
use gtk::prelude::*;
use strum_macros as sm;

#[derive(sm::EnumDiscriminants)]
pub enum DeviceConnection {
    SkyWatcherMountSerial{ device: String },
    IoptronMountSerial{ device: String },
    #[cfg(feature = "mount_ascom")]
    AscomMount{ prog_id: String },
    MountSimulator,
    DreamFocuserMini{ device: String },
    FocusCube3Serial{ device: String },
    FocusCube3TcpIp{ address: std::net::SocketAddr }
}

#[derive(Copy, Clone, PartialEq)]
pub enum DeviceType {
    Mount,
    Focuser
}

pub trait ConnectionCreator {
    fn controls(&self) -> &gtk::Box;
    fn create(&self, configuration: &crate::config::Configuration) -> DeviceConnection;
    fn label(&self) -> &'static str;
}

impl DeviceConnection {
    pub fn device_type(&self) -> DeviceType {
        match self {
            DeviceConnection::SkyWatcherMountSerial{..} => DeviceType::Mount,
            DeviceConnection::IoptronMountSerial{..} => DeviceType::Mount,
            #[cfg(feature = "mount_ascom")]
            DeviceConnection::AscomMount => DeviceType::Mount,
            DeviceConnection::MountSimulator => DeviceType::Mount,
            DeviceConnection::DreamFocuserMini{..} => DeviceType::Focuser,
            DeviceConnection::FocusCube3Serial{..} => DeviceType::Focuser,
            DeviceConnection::FocusCube3TcpIp{..} => DeviceType::Focuser,
        }
    }
}

impl DeviceConnectionDiscriminants {
    pub fn creator(&self, configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        match self {
            #[cfg(feature = "mount_ascom")]
            DeviceConnectionDiscriminants::AscomMount => creators.push(ascom::AscomConnectionCreator::new(configuration)),

            DeviceConnectionDiscriminants::MountSimulator => simulator::SimulatorConnectionCreator::new(configuration),

            DeviceConnectionDiscriminants::SkyWatcherMountSerial => skywatcher::SWConnectionCreator::new(configuration),

            DeviceConnectionDiscriminants::IoptronMountSerial => ioptron::IoptronConnectionCreator::new(configuration),

            _ => unimplemented!()
        }
    }
}
