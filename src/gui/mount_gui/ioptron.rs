//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! iOptron mount connection GUI.
//!

use crate::gui::mount_gui::connection_dialog::ConnectionCreator;
use crate::mount::MountConnection;
use gtk::prelude::*;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct IoptronConnectionCreator {
    dialog_tab: gtk::Box,
    entry: gtk::Entry
}

impl IoptronConnectionCreator {
    pub(in crate::gui::mount_gui) fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

        vbox.pack_start(
            &gtk::Label::new(Some("iOptron direct serial connection")),
            false,
            false,
            PADDING
        );

        vbox.pack_start(
            &gtk::Label::new(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):")),
            false,
            false,
            PADDING
        );

        let entry = gtk::Entry::new();
        entry.set_text(&configuration.ioptron_last_device().unwrap_or("".to_string()));
        vbox.pack_start(&entry, true, false, PADDING);

        Box::new(IoptronConnectionCreator{ dialog_tab: vbox, entry })
    }
}

impl ConnectionCreator for IoptronConnectionCreator {
    fn dialog_tab(&self) -> &gtk::Box { &self.dialog_tab }

    fn create(&self, configuration: &crate::config::Configuration) -> MountConnection {
        configuration.set_ioptron_last_device(&self.entry.text());
        MountConnection::IoptronSerial(self.entry.text().as_str().to_string())
    }

    fn label(&self) -> &'static str { "iOptron" }
}
