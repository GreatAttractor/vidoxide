//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini connection GUI.
//!

use crate::{
    devices::DeviceConnection,
    gui::ConnectionCreator
};
use gtk::prelude::*;
use std::error::Error;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct DreamFocuserMiniConnectionCreator {
    controls: gtk::Box,
    device: gtk::Entry
}

impl DreamFocuserMiniConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 0);

        controls.pack_start(
            &gtk::Label::new(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyACM0” on Linux):")),
            false,
            false,
            PADDING
        );

        let device = gtk::Entry::new();
        controls.pack_start(&device, false, true, PADDING);

        Box::new(DreamFocuserMiniConnectionCreator{ controls, device })
    }
}

impl ConnectionCreator for DreamFocuserMiniConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls }

    fn create(&self, configuration: &crate::config::Configuration) -> Result<DeviceConnection, Box<dyn Error>> {
        let device = self.device.text().as_str().to_string();

        // TODO: update configuration configuration.set_...(&device);
        Ok(DeviceConnection::DreamFocuserMini{ device })
    }

    fn label(&self) -> &'static str { "DreamFocuser mini" }
}
