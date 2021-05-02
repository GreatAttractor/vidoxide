//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount connection dialog.
//!

use crate::gui::DialogDestroyer;
use crate::mount::MountConnection;
use crate::ProgramData;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

/// Returns `None` if canceled.
pub fn show_mount_connect_dialog(parent: &gtk::ApplicationWindow, program_data_rc: &Rc<RefCell<ProgramData>>)
-> Option<MountConnection> {
    let dialog = gtk::Dialog::new_with_buttons(
        Some("Connect to mount"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    let _ddestr = DialogDestroyer::new(&dialog);

    let notebook = gtk::Notebook::new();

    let (sw_tab, sw_device) = create_skywatcher_tab();
    sw_device.set_text(&program_data_rc.borrow().config.skywatcher_last_device().unwrap_or("".to_string()));
    notebook.append_page(&sw_tab, Some(&gtk::Label::new(Some("Sky-Watcher serial connection"))));

    dialog.get_content_area().pack_start(&notebook, true, true, PADDING);

    dialog.show_all();
    let response = dialog.run();

    if response == gtk::ResponseType::Accept {
        match notebook.get_current_page() {
            Some(0) => {
                program_data_rc.borrow().config.set_skywatcher_last_device(&sw_device.get_text().unwrap());
                Some(MountConnection::SkyWatcherSerial(sw_device.get_text().unwrap().as_str().to_string()))
            },
            Some(_) => unreachable!(),
            None => unreachable!()
        }
    } else {
        None
    }
}

fn create_skywatcher_tab() -> (gtk::Box, gtk::Entry) {
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    vbox.pack_start(
        &gtk::Label::new(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyUSB0” on Linux):")),
        false,
        false,
        PADDING
    );

    let entry = gtk::Entry::new();
    vbox.pack_start(&entry, true, false, PADDING);

    (vbox, entry)
}