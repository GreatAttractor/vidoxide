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
use strum::EnumIter;

#[derive(sm::EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub enum DeviceConnection {
    SkyWatcherMountSerial{ device: String },
    IoptronMountSerial{ device: String },
    #[cfg(feature = "mount_ascom")]
    AscomMount{ prog_id: String },
    MountSimulator,
    FocuserSimulator,
    //DreamFocuserMini{ device: String },
    FocusCube3{ connection: focuser::FC3Connection },
}


#[derive(Copy, Clone, PartialEq)]
pub enum DeviceType {
    Mount,
    Focuser
}

impl DeviceConnectionDiscriminants {
    pub fn device_type(&self) -> DeviceType {
        match self {
            DeviceConnectionDiscriminants::SkyWatcherMountSerial{..} => DeviceType::Mount,
            DeviceConnectionDiscriminants::IoptronMountSerial{..} => DeviceType::Mount,
            #[cfg(feature = "mount_ascom")]
            DeviceConnectionDiscriminants::AscomMount => DeviceType::Mount,
            DeviceConnectionDiscriminants::MountSimulator => DeviceType::Mount,
            // DeviceConnectionDiscriminants::DreamFocuserMini{..} => DeviceType::Focuser,
            DeviceConnectionDiscriminants::FocusCube3{..} => DeviceType::Focuser,
            DeviceConnectionDiscriminants::FocuserSimulator => DeviceType::Focuser,
        }
    }
}
