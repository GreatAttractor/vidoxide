//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2025 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Script runner thread.
//!

use crossbeam;
//use mlua;
use crate::{scripting, scripting::{MainToScriptThreadMsg, ScriptToMainThreadMsg}};
use glib::clone;
use std::error::Error;

pub fn script_thread(
    script: String, // TODO Use `mlua::AsChunk` somehow? But it carries a lifetime.
    sender: glib::Sender<ScriptToMainThreadMsg>,
    receiver: crossbeam::channel::Receiver<MainToScriptThreadMsg>,
) {
    if let Err(e) = clone!(@strong sender, @strong receiver => @default-panic, move || {
        let lua = scripting::create_lua_state(sender, receiver)?;
        lua.load(&script).exec()?;
        Result::<(), Box<dyn Error>>::Ok(())
    })() {
        sender.send(ScriptToMainThreadMsg::Finished(Err(e.to_string()))).unwrap();
    } else {
        sender.send(ScriptToMainThreadMsg::Finished(Ok(()))).unwrap();
    }
    //

}
