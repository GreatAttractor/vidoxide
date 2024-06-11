//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope focuser GUI.
//!

mod connection_dialog; //TODO remove

pub mod focuscube3;

use crate::{
    devices::{DeviceConnectionDiscriminants, focuser},
    gui::{device_connection_dialog, show_message},
    ProgramData
};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

/// Control padding in pixels.
const PADDING: u32 = 10;

struct SpeedDescr {
    speed: focuser::Speed,
    label: String,
    supported: bool
}

pub struct FocuserWidgets {
    wbox: gtk::Box,
    status: gtk::Label,
    speeds: Rc<RefCell<Vec<SpeedDescr>>>,
    speed_combo: gtk::ComboBox,
}

impl FocuserWidgets {
    pub fn wbox(&self) -> &gtk::Box { &self.wbox }

    fn on_connect(&self, focuser: &mut focuser::FocuserWrapper)
    {
        self.wbox.set_sensitive(true);
        self.status.set_text(&format!("{}", focuser.get().info()));
        let focuser::SpeedRange{ min, max } = match focuser.get_mut().speed_range() {
            Ok(range) => range,
            Err(e) => {
                log::error!("Failed to obtain focuser speed range: {}; using 1x speed only.", e);
                focuser::SpeedRange{ min: focuser::Speed::one(), max: focuser::Speed::one() }
            }
        };
        let mut speeds = self.speeds.borrow_mut();
        for speed in speeds.iter_mut() {
            speed.supported = speed.speed >= min && speed.speed <= max;
            //speed[idx] = mount.slewing_speed_supported(m * mount::SIDEREAL_RATE);
        }
    }

    fn on_disconnect(&self)
    {
        self.wbox.set_sensitive(false);
        self.status.set_text("disconnected");
        // self.disable_sky_tracking_btn();
    }

    pub fn selected_speed(&self) -> focuser::Speed {
        self.speeds.borrow()[self.speed_combo.active().unwrap() as usize].speed
    }
}

pub fn init_focuser_menu(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let item_disconnect = gtk::MenuItem::with_label("Disconnect");
    item_disconnect.connect_activate(clone!(@weak program_data_rc => @default-panic, move |menu_item| {
        {
            let mut pd = program_data_rc.borrow_mut();
            let focuser_info = pd.focuser_data.focuser.as_ref().unwrap().get().info();
            pd.focuser_data.focuser = None;
            pd.gui.as_ref().unwrap().focuser_widgets.on_disconnect();
            log::info!("disconnected from {}", focuser_info);
        }
        menu_item.set_sensitive(false);
    }));
    item_disconnect.set_sensitive(false);

    let item_connect = gtk::MenuItem::with_label("Connect...");
    item_connect.connect_activate(clone!(
        @weak program_data_rc,
        @weak item_disconnect
        => @default-panic, move |_| {
            match device_connection_dialog::show_device_connection_dialog(
                "Connect to focuser",
                "Focuser type:",
                &program_data_rc,
                //TODO iterate over focuser-type items
                &[
                    DeviceConnectionDiscriminants::FocusCube3
                ]
            ) {
                Some(connection) => {
                    match focuser::connect_to_focuser(connection) {
                        Err(e) => show_message(
                            &format!("Failed to connect to focuser: {:?}.", e),
                            "Error",
                            gtk::MessageType::Error,
                            &program_data_rc
                        ),
                        Ok(mut focuser) => {
                            log::info!("connected to {}", focuser.get().info());
                            program_data_rc.borrow().gui.as_ref().unwrap().focuser_widgets.on_connect(&mut focuser);
                            program_data_rc.borrow_mut().focuser_data.focuser = Some(focuser);
                            item_disconnect.set_sensitive(true);
                        }
                    }
                },
                _ => ()
            }
        }
    ));

    menu.append(&item_connect);
    menu.append(&item_disconnect);

    menu
}

pub fn focuser_move(
    speed: focuser::Speed,
    dir: focuser::FocuserDir,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> Result<(), ()> {
    let res = program_data_rc.borrow_mut().focuser_data.focuser.as_mut().unwrap().begin_move_in_dir(speed, dir);
    if let Err(e) = &res { /*TODO on_error...*/ }
    res.map_err(|_| ())
}

pub fn create_focuser_box(program_data_rc: &Rc<RefCell<ProgramData>>) -> FocuserWidgets {
    let speeds = Rc::new(RefCell::new(vec![
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 64.0), label: "1/64x".into(), supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 32.0), label: "1/32x".into(), supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 16.0), label: "1/16x".into(), supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 8.0),  label: "1/8x".into() , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 4.0),  label: "1/4x".into() , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0 / 2.0),  label: "1/2x".into() , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(1.0),        label: "1x".into()   , supported: true  },
        SpeedDescr{ speed: focuser::Speed::new(2.0),        label: "2x".into()   , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(4.0),        label: "4x".into()   , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(8.0),        label: "8x".into()   , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(16.0),       label: "16x".into()  , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(32.0),       label: "32x".into()  , supported: false },
        SpeedDescr{ speed: focuser::Speed::new(64.0),       label: "64x".into()  , supported: false },
    ]));

    let normal_speed_idx = speeds.borrow().iter().enumerate().find(|(idx, s)| s.speed.get() == 1.0).unwrap().0;

    let contents = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let upper_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    upper_box.pack_start(&gtk::Label::new(Some("Speed:")), false, false, PADDING);

    let model = gtk::ListStore::new(&[gtk::glib::Type::STRING]);
    for (idx, speed) in speeds.borrow().iter().enumerate() {
        model.insert_with_values(Some(idx as u32), &[(0u32, &speed.label)]);
    }
    let speed_combo = gtk::ComboBox::with_model(&model);
    let renderer = gtk::CellRendererText::new();
    speed_combo.pack_start(&renderer, true);
    speed_combo.add_attribute(&renderer, "text", 0);
    speed_combo.set_cell_data_func(&renderer, Some(Box::new(
        clone!(@weak speeds => @default-panic, move |_, cell, model, iter| {
            let path = model.path(iter).unwrap();
            cell.set_sensitive(speeds.borrow()[path.indices()[0] as usize].supported);
        })
    )));
    speed_combo.set_active(Some(normal_speed_idx as u32));
    upper_box.pack_start(&speed_combo, false, false, PADDING);

    let btn_stop = gtk::Button::with_label("stop");
    btn_stop.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| on_stop(&program_data_rc)));
    upper_box.pack_end(&btn_stop, false, false, PADDING);

    contents.pack_start(&upper_box, false, false, PADDING);

    let move_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    let btn_move_neg = gtk::Button::with_label("←");
    btn_move_neg.set_tooltip_text(Some("Move focuser in negative direction"));
    btn_move_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(1.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    btn_move_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(0.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    move_box.pack_start(&btn_move_neg, true, true, 0);

    let btn_move_pos = gtk::Button::with_label("→");
    btn_move_pos.set_tooltip_text(Some("Move focuser in positive direction"));
    btn_move_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(1.0), focuser::FocuserDir::Positive, &program_data_rc).is_err())
    }));
    btn_move_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(0.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    move_box.pack_start(&btn_move_pos, true, true, 0);

    contents.pack_start(&move_box, false, false, PADDING);

    let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let status_label = gtk::LabelBuilder::new().justify(gtk::Justification::Left).label("disconnected").build();
    status_box.pack_start(&status_label, false, false, PADDING);
    contents.pack_end(&status_box, false, false, PADDING);

    contents.set_sensitive(false);

    FocuserWidgets{
        wbox: contents,
        status: status_label,
        speeds,
        speed_combo
    }
}

fn on_stop(program_data_rc: &Rc<RefCell<ProgramData>>) {
    if let Err(e) = program_data_rc.borrow_mut().focuser_data.focuser.as_mut().unwrap().get_mut().stop() {
        log::error!("Failed to stop the focuser: {}.", e);
    }
}
