//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2021-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ASCOM mount connection GUI.
//!

use crate::{
    device_connection::{ConnectionCreator, DeviceConnection},
    gui::BasicConnectionControls
};

pub struct AscomConnectionCreator {
    dialog_tab: gtk::Box,
    entry: gtk::Entry
}

impl AscomConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        Box::new(AscomConnectionCreator{
            controls: BasicConnectionControls::new(
                None,
                Some("Driver name:"),
                true,
                Some(configuration.ascom_last_driver().unwrap_or("".to_string()))
            )
        })
    }
}

impl ConnectionCreator for AscomConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls.controls() }

    fn create(&self, configuration: &crate::config::Configuration) -> DeviceConnection {
        let s = self.controls.connection_string();
        configuration.set_ascom_last_driver(&s);
        DeviceConnection::AscomMount{device: s}
    }

    fn label(&self) -> &'static str { "ASCOM" }
}
