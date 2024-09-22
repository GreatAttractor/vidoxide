//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! FocusCube3 connection GUI.
//!

use crate::{
    devices::{DeviceConnection, focuser::FC3Connection},
    gui::ConnectionCreator
};
use gtk::prelude::*;
use std::error::Error;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct FocusCube3ConnectionCreator {
    controls: gtk::Box,
    rb_serial: gtk::RadioButton,
    serial_port: gtk::Entry,
    rb_net: gtk::RadioButton,
    network_addr: gtk::Entry,
    network_pwd: gtk::Entry
}

impl FocusCube3ConnectionCreator {
    pub fn new(configuration: &crate::config::Configuration) -> Box<dyn ConnectionCreator> {
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let serial_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let rb_serial = gtk::RadioButton::with_label("Serial port:");
        serial_box.pack_start(&rb_serial, false, false, PADDING);
        let serial_port = gtk::Entry::new();
        serial_port.set_tooltip_text(Some("Device name (e.g., “COM5” on Windows or “/dev/ttyACM0” on Linux)"));
        if let Some(s) = configuration.focuscube3_last_serial_port() { serial_port.set_text(&s); }
        serial_box.pack_start(&serial_port, false, true, PADDING);
        controls.pack_start(&serial_box, false, false, PADDING);

        let net_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let rb_net = gtk::RadioButton::with_label_from_widget(&rb_serial, "Network address:");
        net_box.pack_start(&rb_net, false, false, PADDING);
        let network_addr = gtk::Entry::new();
        if let Some(s) = configuration.focuscube3_last_network_addr() {
            network_addr.set_text(&s);
        } else {
            network_addr.set_text("focuscube3.local:9999");
        }
        net_box.pack_start(&network_addr, false, false, PADDING);
        net_box.pack_start(&gtk::Label::new(Some("password:")), false, false, PADDING);
        let network_pwd = gtk::EntryBuilder::new().input_purpose(gtk::InputPurpose::Password).visibility(false).build();
        if let Some(s) = configuration.focuscube3_last_network_pwd() {
            network_pwd.set_text(&s);
        } else {
            network_pwd.set_text("12345678"); // default FC3 password
        }
        net_box.pack_start(&network_pwd, false, false, PADDING);
        controls.pack_start(&net_box, false, false, PADDING);
        controls.pack_start(
            &gtk::LabelBuilder::new()
                .label("<b>WARNING</b>: password is stored in plain text in the configuration file.")
                .use_markup(true)
                .halign(gtk::Align::End)
                .build(),
            false,
            true,
            PADDING
        );

        Box::new(FocusCube3ConnectionCreator { controls, rb_serial, serial_port, rb_net, network_addr, network_pwd })
    }
}

impl ConnectionCreator for FocusCube3ConnectionCreator {
    fn controls(&self) -> &gtk::Box { &self.controls }

    fn create(&self, configuration: &crate::config::Configuration) -> Result<DeviceConnection, Box<dyn Error>> {
        if self.rb_serial.is_active() {
            let device = self.serial_port.text().as_str().to_string();
            configuration.set_focuscube3_last_serial_port(&device);
            Ok(DeviceConnection::FocusCube3{ connection: FC3Connection::Serial{ device } })
        } else {
            let address = self.network_addr.text().as_str().to_string();
            let password = self.network_pwd.text().as_str().to_string();
            configuration.set_focuscube3_last_network_addr(&address);
            configuration.set_focuscube3_last_network_pwd(&password);

            Ok(DeviceConnection::FocusCube3{ connection: FC3Connection::TcpIp{ address, password } })
        }
    }

    fn label(&self) -> &'static str { "FocusCube3" }
}
