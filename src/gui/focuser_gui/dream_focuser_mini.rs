//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! DreamFocuser mini connection GUI.
//!

use crate::{
    devices::{DeviceConnection, focuser::DFminiConnection},
    gui::ConnectionCreator
};
use gtk::prelude::*;
use std::error::Error;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct DreamFocuserMiniConnectionCreator {
    controls: gtk::Box,
    rb_usb: gtk::RadioButton,
    usb_serial_port: gtk::Entry,
    bluetooth_mac: gtk::Entry
}

impl DreamFocuserMiniConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let usb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let rb_usb = gtk::RadioButton::with_label("[USB] Serial port:");
        usb_box.pack_start(&rb_usb, false, false, PADDING);

        let usb_serial_port = gtk::Entry::new();
        usb_serial_port.set_tooltip_text(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyACM0” on Linux)"));
        if let Some(s) = configuration.dreamfocuser_mini_last_serial_port() { usb_serial_port.set_text(&s); }
        usb_box.pack_start(&usb_serial_port, false, true, PADDING);
        controls.pack_start(&usb_box, false, false, PADDING);

        let bt_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let rb_bt = gtk::RadioButton::with_label_from_widget(&rb_usb, "[Bluetooth] MAC address:");
        bt_box.pack_start(&rb_bt, false, false, PADDING);

        let bluetooth_mac = gtk::Entry::new();
        bluetooth_mac.set_tooltip_text(Some("Bluetooth MAC address of the focuser"));
        // TODO: read prev. MAC addr. from configuration
        bt_box.pack_start(&bluetooth_mac, false, true, PADDING);
        controls.pack_start(&bt_box, false, false, PADDING);

        Box::new(DreamFocuserMiniConnectionCreator{ controls, rb_usb, usb_serial_port, bluetooth_mac })
    }
}

impl ConnectionCreator for DreamFocuserMiniConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls }

    fn create(&self, configuration: &crate::config::Configuration) -> Result<DeviceConnection, Box<dyn Error>> {
        if self.rb_usb.is_active() {
            let device = self.usb_serial_port.text().as_str().to_string();
            configuration.set_dreamfocuser_mini_last_serial_port(&device);

            Ok(DeviceConnection::DreamFocuserMini{ connection: DFminiConnection::USB{ device } })
        } else {
            let mac_addr = self.bluetooth_mac.text().as_str().to_string();
            //TODO: store in configuration

            Ok(DeviceConnection::DreamFocuserMini{ connection: DFminiConnection::Bluetooth{ device: mac_addr } })
        }
    }

    fn label(&self) -> &'static str { "DreamFocuser mini" }
}
