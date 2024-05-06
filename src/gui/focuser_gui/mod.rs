//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope focuser GUI.
//!

mod connection_dialog;

use crate::{focuser, gui::show_message, ProgramData};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

pub fn init_focuser_menu(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let item_disconnect = gtk::MenuItem::with_label("Disconnect");
    item_disconnect.connect_activate(clone!(@weak program_data_rc => @default-panic, move |menu_item| {
        // program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.on_disconnect();
        {
            let mut pd = program_data_rc.borrow_mut();
            let focuser_info = pd.focuser_data.focuser.as_ref().unwrap().info();
            pd.focuser_data.focuser = None;
            //pd.gui.as_ref().unwrap().mount_widgets.on_disconnect();
            log::info!("disconnected from {}", focuser_info);
        }
        menu_item.set_sensitive(false);
    }));
    item_disconnect.set_sensitive(false);

    let item_connect = gtk::MenuItem::with_label("Connect...");
    item_connect.connect_activate(clone!(
        @weak program_data_rc,
        @weak item_disconnect
        => @default-panic, move |_| {
            match connection_dialog::show_focuser_connect_dialog(&program_data_rc) {
                Some(connection) => {
                    match focuser::connect_to_focuser(connection) {
                        Err(e) => show_message(
                            &format!("Failed to connect to focuser: {:?}.", e),
                            "Error",
                            gtk::MessageType::Error,
                            &program_data_rc
                        ),
                        Ok(focuser) => {
                            log::info!("connected to {}", focuser.info());
                            program_data_rc.borrow_mut().focuser_data.focuser = Some(focuser);
                            item_disconnect.set_sensitive(true);
                        }
                    }
                },
                _ => ()
            }
        }
    ));

    menu.append(&item_connect);
    menu.append(&item_disconnect);

    menu
}

pub fn focuser_move(speed: focuser::Speed, program_data_rc: &Rc<RefCell<ProgramData>>) -> Result<(), ()> {
    log::info!("attempted focuser move with speed {:.02}", speed.get());
    Ok(())
}
