//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ZWO mount connection GUI.
//!

use crate::{
    devices::DeviceConnection,
    gui::{BasicConnectionControls, ConnectionCreator}
};
use std::error::Error;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct ZWOConnectionCreator {
    controls: BasicConnectionControls
}

impl ZWOConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        Box::new(ZWOConnectionCreator{
            controls: BasicConnectionControls::new(
                None,
                Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):"),
                true,
                Some(configuration.zwo_last_device().unwrap_or("".to_string()))
            )
        })
    }
}

impl ConnectionCreator for ZWOConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls.controls() }

    fn create(&self, configuration: &crate::config::Configuration) -> Result<DeviceConnection, Box<dyn Error>> {
        let device = self.controls.connection_string();
        configuration.set_zwo_last_device(&device);
        Ok(DeviceConnection::ZWOMountSerial{device})
    }

    fn label(&self) -> &'static str { "ZWO (direct serial connection)" }
}
