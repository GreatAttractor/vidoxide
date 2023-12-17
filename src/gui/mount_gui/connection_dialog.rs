//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount connection dialog.
//!

use crate::gui::DialogDestroyer;
#[cfg(feature = "mount_ascom")]
use crate::gui::mount_gui::ascom;
use crate::gui::mount_gui::{ioptron, simulator, skywatcher};
use crate::mount::MountConnection;
use crate::ProgramData;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use strum::IntoEnumIterator;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub(in crate::gui::mount_gui) trait ConnectionCreator {
    fn dialog_tab(&self) -> &gtk::Box;
    fn create(&self, configuration: &crate::config::Configuration) -> MountConnection;
    fn label(&self) -> &'static str;
}

/// Returns `None` if canceled.
pub fn show_mount_connect_dialog(parent: &gtk::ApplicationWindow, program_data_rc: &Rc<RefCell<ProgramData>>)
-> Option<MountConnection> {
    macro_rules! configuration { {} => { program_data_rc.borrow().config } }

    let dialog = gtk::Dialog::with_buttons(
        Some("Connect to mount"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    let _ddestr = DialogDestroyer::new(&dialog);

    let notebook = gtk::Notebook::new();

    let mut creators: Vec<Box<dyn ConnectionCreator>> = vec![];

    for mount_type in MountConnection::iter() {
        match mount_type {
            #[cfg(feature = "mount_ascom")]
            MountConnection::Ascom(_) => creators.push(ascom::AscomConnectionCreator::new(&configuration!())),

            MountConnection::Simulator =>
                creators.push(simulator::SimulatorConnectionCreator::new(&configuration!())),

            MountConnection::SkyWatcherSerial(_) =>
                creators.push(skywatcher::SWConnectionCreator::new(&configuration!())),

            MountConnection::IoptronSerial(_) =>
                creators.push(ioptron::IoptronConnectionCreator::new(&configuration!())),
        }
    }

    for creator in &creators {
        notebook.append_page(creator.dialog_tab(), Some(&gtk::Label::new(Some(creator.label()))));
    }

    dialog.content_area().pack_start(&notebook, true, true, PADDING);

    dialog.show_all();
    let response = dialog.run();

    if response == gtk::ResponseType::Accept {
        Some(creators[notebook.current_page().unwrap() as usize].create(&configuration!()))
    } else {
        None
    }
}
