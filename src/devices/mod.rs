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
    FocusCube3{ connection: focuser::FC3Connection },

}

#[derive(Copy, Clone, PartialEq)]
pub enum DeviceType {
    Mount,
    Focuser
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
            DeviceConnection::FocusCube3{..} => DeviceType::Focuser,
        }
    }
}
