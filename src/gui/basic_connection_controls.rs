//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Basic controls for setting up a device connection.
//!

use gtk::prelude::*;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct BasicConnectionControls {
    box_: gtk::Box,
    entry: Option<gtk::Entry>
}

impl BasicConnectionControls {
    pub fn new(
        title: Option<&str>,
        info: Option<&str>,
        has_entry: bool,
        last_entry: Option<String>
    ) -> BasicConnectionControls {
        assert!(!(!has_entry && last_entry.is_some()), "cannot provide last entry value if there is no entry");

        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);

        if let Some(title) = title {
            box_.pack_start(
                &gtk::Label::new(Some(title)),
                false,
                false,
                PADDING
            );
        }

        if let Some(info) = info {
            box_.pack_start(
                &gtk::Label::new(Some(info)),
                false,
                false,
                PADDING
            );
        }


        let entry = if has_entry {
            let entry = gtk::Entry::new();
            entry.set_text(&last_entry.unwrap_or("".to_string()));
            box_.pack_start(&entry, true, false, PADDING);
            Some(entry)
        } else {
            None
        };

        BasicConnectionControls{ box_, entry }
    }

    pub fn controls(&self) -> &gtk::Box { &self.box_ }

    pub fn connection_string(&self) -> String {
        self.entry.as_ref().unwrap().text().to_string()
    }
}
