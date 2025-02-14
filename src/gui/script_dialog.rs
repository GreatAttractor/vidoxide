//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2025 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Script dialog.
//!

use crate::{ProgramData, gui::{show_message}, workers};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};


//TESTING #########
const SCRIPT: &str = r#"
    vdx = vidoxide

    vdx.foo()
"#;

pub fn create_script_dialog(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> gtk::Dialog {
    let dialog = gtk::Dialog::with_buttons(
        Some("Run script"),
        Some(parent),
        gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("Close", gtk::ResponseType::Close)]
    );

    dialog.set_default_response(gtk::ResponseType::Close);

    dialog.connect_response(|dialog, response| {
        if response == gtk::ResponseType::Close { dialog.hide(); }
    });

    dialog.connect_delete_event(|dialog, _| {
        dialog.hide();
        gtk::Inhibit(true)
    });

    dialog.show_all();
    dialog.hide();

    dialog
}

// pub fn show_script_dialog(program_data_rc: &Rc<RefCell<ProgramData>>) {

//     let dialog = create_dialog(program_data_rc);
//     let _ddestr = DialogDestroyer::new(&dialog);

//     dialog.show_all(); //TODO do in initialization
//     dialog.run();


    // let (sender_main, receiver_worker) = crossbeam::channel::unbounded();
    // let (sender_worker, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    // let join_handle = std::thread::spawn(
    //     move || workers::script::script_thread(SCRIPT.into(), sender_worker, receiver_worker)
    // );



    //TODO block on join_handle or sth. No, rather make the script dialog modal. Then the user cannot mess anything up via GUI.
// }
