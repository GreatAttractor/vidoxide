//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope focuser connection dialog.
//!

use crate::{devices::DeviceConnection, gui::DialogDestroyer, ProgramData};
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

/// Returns `None` if canceled.
pub fn show_focuser_connect_dialog(program_data_rc: &Rc<RefCell<ProgramData>>)
-> Option<DeviceConnection> {
    let dialog = gtk::Dialog::with_buttons(
        Some("Connect to focuser"),
        Some(&program_data_rc.borrow().gui.as_ref().unwrap().app_window),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    let _ddestr = DialogDestroyer::new(&dialog);

    dialog.show_all();
    let response = dialog.run();

    // if response == gtk::ResponseType::Accept {
    //     Some(DeviceConnection::FocusCube3{ connection: FC3Connection::Serial{ device: "/dev/ttyACM0".into() }})
    // } else {
        None
    // }
}
