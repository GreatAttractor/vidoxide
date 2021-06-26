//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ASCOM mount connection GUI.
//!

use crate::gui::mount_gui::connection_dialog::ConnectionCreator;
use crate::mount::MountConnection;
use gtk::prelude::*;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct AscomConnectionCreator {
    dialog_tab: gtk::Box,
    entry: gtk::Entry
}

impl AscomConnectionCreator {
    pub(in crate::gui::mount_gui) fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

        vbox.pack_start(
            &gtk::Label::new(Some("Driver name:")),
            false,
            false,
            PADDING
        );

        let entry = gtk::Entry::new();
        entry.set_text(&configuration.ascom_last_driver().unwrap_or("".to_string()));
        vbox.pack_start(&entry, true, false, PADDING);

        Box::new(AscomConnectionCreator{ dialog_tab: vbox, entry })
    }
}

impl ConnectionCreator for AscomConnectionCreator {
    fn dialog_tab(&self) -> &gtk::Box { &self.dialog_tab }

    fn create(&self, configuration: &crate::config::Configuration) -> MountConnection {
        configuration.set_ascom_last_driver(&self.entry.get_text().unwrap());
        MountConnection::Ascom(self.entry.get_text().unwrap().as_str().to_string())
    }

    fn label(&self) -> &'static str { "ASCOM" }
}
