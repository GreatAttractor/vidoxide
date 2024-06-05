//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Sky-Watcher mount connection GUI.
//!

use crate::{
    devices::{ConnectionCreator, DeviceConnection},
    gui::BasicConnectionControls
};

pub struct SWConnectionCreator {
    controls: BasicConnectionControls
}

impl SWConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        Box::new(SWConnectionCreator{
            controls: BasicConnectionControls::new(
                None,
                Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):"),
                true,
                Some(configuration.skywatcher_last_device().unwrap_or("".to_string()))
            )
        })
    }
}

impl ConnectionCreator for SWConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls.controls() }

    fn create(&self, configuration: &crate::config::Configuration) -> DeviceConnection {
        let device = self.controls.connection_string();
        configuration.set_skywatcher_last_device(&device);
        DeviceConnection::SkyWatcherMountSerial{device}
    }

    fn label(&self) -> &'static str { "Sky-Watcher (direct serial connection)" }
}
