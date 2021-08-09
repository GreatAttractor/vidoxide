//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Sky-Watcher mount connection GUI.
//!

use crate::gui::mount_gui::connection_dialog::ConnectionCreator;
use crate::mount::MountConnection;
use gtk::prelude::*;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct SWConnectionCreator {
    dialog_tab: gtk::Box,
    entry: gtk::Entry
}

impl SWConnectionCreator {
    pub(in crate::gui::mount_gui) fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

        vbox.pack_start(
            &gtk::Label::new(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):")),
            false,
            false,
            PADDING
        );

        let entry = gtk::Entry::new();
        entry.set_text(&configuration.skywatcher_last_device().unwrap_or("".to_string()));
        vbox.pack_start(&entry, true, false, PADDING);

        Box::new(SWConnectionCreator{ dialog_tab: vbox, entry })
    }
}

impl ConnectionCreator for SWConnectionCreator {
    fn dialog_tab(&self) -> &gtk::Box { &self.dialog_tab }

    fn create(&self, configuration: &crate::config::Configuration) -> MountConnection {
        configuration.set_skywatcher_last_device(&self.entry.text());
        MountConnection::SkyWatcherSerial(self.entry.text().as_str().to_string())
    }

    fn label(&self) -> &'static str { "Sky-Watcher serial connection" }
}
