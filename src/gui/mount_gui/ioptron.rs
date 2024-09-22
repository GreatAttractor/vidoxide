//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! iOptron mount connection GUI.
//!

use crate::{
    devices::DeviceConnection,
    gui::{BasicConnectionControls, ConnectionCreator}
};
use std::error::Error;

pub struct IoptronConnectionCreator {
    controls: BasicConnectionControls
}

impl IoptronConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        Box::new(IoptronConnectionCreator{
            controls: BasicConnectionControls::new(
                None,
                Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):"),
                true,
                Some(configuration.ioptron_last_device().unwrap_or("".to_string()))
            )
        })
    }
}

impl ConnectionCreator for IoptronConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls.controls() }

    fn create(&self, configuration: &crate::config::Configuration) -> Result<DeviceConnection, Box<dyn Error>> {
        let device = self.controls.connection_string();
        configuration.set_ioptron_last_device(&device);
        Ok(DeviceConnection::IoptronMountSerial{device})
    }

    fn label(&self) -> &'static str { "iOptron (direct serial connection)" }
}
