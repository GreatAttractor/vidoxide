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

use crate::{ProgramData, gui::show_message, workers};
use glib::clone;
use std::{cell::RefCell, rc::Rc};


//TESTING #########
const SCRIPT: &str = r#"
    vdx = vidoxide

    vdx.foo()
"#;

pub fn show_script_dialog(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let (sender_main, receiver_worker) = crossbeam::channel::unbounded();
    let (sender_worker, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    let join_handle = std::thread::spawn(
        move || workers::script::script_thread(SCRIPT.into(), sender_worker, receiver_worker)
    );

    //TODO block on join_handle or sth. No, rather make the script dialog modal. Then the user cannot mess anything up via GUI.
}
