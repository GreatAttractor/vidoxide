//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Time widget.
//!

use gtk::prelude::*;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct TimeWidget {
    gtkbox: gtk::Box,
    btn_hours: gtk::SpinButton,
    btn_minutes: gtk::SpinButton,
    btn_seconds: gtk::SpinButton,
}

impl TimeWidget {
    pub fn new_with_value(hours: u32, minutes: u32, seconds: u32) -> TimeWidget {
        let gtkbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        const CLIMB_RATE: f64 = 1.1;
        const DIGITS: u32 = 0;

        let btn_hours = gtk::SpinButton::new(
            Some(&gtk::Adjustment::new(hours as f64, 0.0, 59.0, 1.0, 10.0, 0.0)), CLIMB_RATE, DIGITS
        );
        btn_hours.set_orientation(gtk::Orientation::Vertical);
        gtkbox.pack_start(&btn_hours, false, false, PADDING);
        gtkbox.pack_start(&gtk::Label::new(Some("h")), false, false, PADDING);

        let btn_minutes = gtk::SpinButton::new(
            Some(&gtk::Adjustment::new(minutes as f64, 0.0, 59.0, 1.0, 10.0, 0.0)), CLIMB_RATE, DIGITS
        );
        btn_minutes.set_orientation(gtk::Orientation::Vertical);
        gtkbox.pack_start(&btn_minutes, false, false, PADDING);
        gtkbox.pack_start(&gtk::Label::new(Some("m")), false, false, PADDING);

        let btn_seconds = gtk::SpinButton::new(
            Some(&gtk::Adjustment::new(seconds as f64, 0.0, 59.0, 1.0, 10.0, 0.0)), CLIMB_RATE, DIGITS
        );
        btn_seconds.set_orientation(gtk::Orientation::Vertical);
        gtkbox.pack_start(&btn_seconds, false, false, PADDING);
        gtkbox.pack_start(&gtk::Label::new(Some("s")), false, false, PADDING);

        TimeWidget { gtkbox, btn_hours, btn_minutes, btn_seconds }
    }

    pub fn get(&self) -> &gtk::Box { &self.gtkbox }

    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.btn_hours.value() as u64 * 3600 +
            self.btn_minutes.value() as u64 * 60 +
            self.btn_seconds.value() as u64
        )
    }
}
