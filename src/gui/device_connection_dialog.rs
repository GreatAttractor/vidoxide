//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Device connection dialog.
//!

use crate::{
    ProgramData,
    device_connection::{ConnectionCreator, DeviceConnection, DeviceConnectionDiscriminants},
    gui::DialogDestroyer
};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

/// Control padding in pixels.
const PADDING: u32 = 10;

pub fn show_device_connection_dialog(
    title: &str,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    connections: &[DeviceConnectionDiscriminants]
) -> Option<DeviceConnection> {
    macro_rules! configuration { {} => { program_data_rc.borrow().config } }

    let dialog = gtk::Dialog::with_buttons(
        Some(title),
        Some(&program_data_rc.borrow().gui.as_ref().unwrap().app_window),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    let _ddestr = DialogDestroyer::new(&dialog);

    let mut creators: Vec<Box<dyn ConnectionCreator>> = vec![];

    for connection in connections {
        creators.push(connection.creator(&program_data_rc.borrow().config));
    }

    let combo = gtk::ComboBoxText::new();
    let notebook = gtk::NotebookBuilder::new().show_tabs(false).build();

    for creator in &creators {
        combo.append_text(creator.label());
        notebook.append_page(creator.controls(), Some(&gtk::Label::new(Some(creator.label()))));
    }
    combo.set_active(Some(0));
    combo.connect_changed(clone!(@weak notebook => @default-panic, move |combo| {
        notebook.set_page(combo.active().unwrap() as i32);
    }));

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    hbox.pack_start(&gtk::Label::new(Some("Mount type:")), false, true, PADDING);
    hbox.pack_start(&combo, true, true, PADDING);
    dialog.content_area().pack_start(&hbox, true, true, PADDING);

    dialog.content_area().pack_start(&notebook, true, true, PADDING);

    dialog.show_all();
    let response = dialog.run();

    if response == gtk::ResponseType::Accept {
        Some(creators[notebook.current_page().unwrap() as usize].create(&configuration!()))
    } else {
        None
    }
}
