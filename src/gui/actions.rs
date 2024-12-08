//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! GUI actions.
//!

// action group name
pub const PREFIX: &'static str = "vidoxide";

// action names to be used for constructing `gio::SimpleAction`
pub const DISCONNECT_CAMERA: &'static str = "disconnect camera";
pub const TAKE_SNAPSHOT:     &'static str = "take snapshot";
pub const SET_ROI:           &'static str = "set roi";
pub const UNDOCK_PREVIEW:    &'static str = "undock preview area";

/// Returns prefixed action name to be used with `ActionableExt::set_action_name`.
pub fn prefixed(s: &str) -> String {
    format!("{}.{}", PREFIX, s)
}
