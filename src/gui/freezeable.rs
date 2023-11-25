//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Freezeable widget wrapper.
//!

use gtk::prelude::*;

/// Automates calling a widget's method with blocking the corresponding signal handler.
///
/// This struct purposefully does not support cloning (as it contains a signal handler ID).
///
pub struct Freezeable<T: glib::ObjectExt> {
    widget: T,
    signal: Option<glib::SignalHandlerId>
}

impl<T: glib::ObjectExt> Freezeable<T> {
    pub fn new(widget: T, signal: Option<glib::SignalHandlerId>) -> Freezeable<T> {
        Freezeable{ widget, signal }
    }

    pub fn freeze(&self) {
        if let Some(signal) = &self.signal { self.widget.block_signal(signal); }
    }

    pub fn thaw(&self) {
        if let Some(signal) = &self.signal { self.widget.unblock_signal(signal); }
    }

    pub fn set_signal(&mut self, signal: glib::SignalHandlerId) {
        self.signal = Some(signal);
    }
}

impl<T: glib::ObjectExt> Drop for Freezeable<T> {
    fn drop(&mut self) {
        // disable the signal to avoid unwanted side effects during widget removal (e.g., if a spin button's text box
        // has focus and the widget is removed, the changed signal handler gets called)
        if let Some(signal) = &self.signal { self.widget.block_signal(signal) };
    }
}

impl<T: glib::ObjectExt> std::ops::Deref for Freezeable<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}
