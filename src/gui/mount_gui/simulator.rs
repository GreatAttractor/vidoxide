//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
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

pub struct SimulatorConnectionCreator {
    dialog_tab: gtk::Box
}

impl SimulatorConnectionCreator {
    pub(in crate::gui::mount_gui) fn new(_configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let dialog_tab = gtk::Box::new(gtk::Orientation::Vertical, 0);

        dialog_tab.pack_start(
            &gtk::Label::new(Some("Mount simulator.")),
            false,
            false,
            PADDING
        );

        Box::new(SimulatorConnectionCreator{ dialog_tab })
    }
}

impl ConnectionCreator for SimulatorConnectionCreator {
    fn dialog_tab(&self) -> &gtk::Box { &self.dialog_tab }

    fn create(&self, _configuration: &crate::config::Configuration) -> MountConnection {
        MountConnection::Simulator
    }

    fn label(&self) -> &'static str { "Simulator" }
}
