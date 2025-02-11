//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2025 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Scripting.
//!

use std::error::Error;

pub enum ScriptToMainThreadMsg {
    Finished(Result<(), String>)
}

pub enum MainToScriptThreadMsg {
}

pub fn create_lua_state(
    sender: glib::Sender<ScriptToMainThreadMsg>,
    receiver: crossbeam::channel::Receiver<MainToScriptThreadMsg>,
) -> Result<mlua::Lua, Box<dyn Error>> {
    let lua = mlua::Lua::new();

    create_pkg_vidoxide(&lua, sender, receiver)?;

    Ok(lua)
}

fn create_pkg_vidoxide(
    lua: &mlua::Lua,
    sender: glib::Sender<ScriptToMainThreadMsg>,
    receiver: crossbeam::channel::Receiver<MainToScriptThreadMsg>
) -> Result<(), Box<dyn Error>> {

    let pkg_vidoxide = lua.create_table()?;

    let f = lua.create_function(|_, _: ()| { println!("foo called"); Ok(()) })?;
    pkg_vidoxide.set("foo", f)?;

    lua.globals().set("vidoxide", pkg_vidoxide)?;

    Ok(())
}
