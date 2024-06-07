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

mod connection_dialog;

use crate::{devices::focuser, gui::show_message, ProgramData};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct FocuserWidgets {
    wbox: gtk::Box,
    status: gtk::Label
}

impl FocuserWidgets {
    pub fn wbox(&self) -> &gtk::Box { &self.wbox }

    fn on_connect(&self, focuser: &focuser::FocuserWrapper)
    {
        self.wbox.set_sensitive(true);
        self.status.set_text(&format!("{}", focuser.get().info()));
        // let mut sss = self.slew_speed_supported.borrow_mut();
        // let mut info = "supported slewing speeds: ".to_string();
        // for (idx, speed) in SLEWING_SPEEDS.iter().enumerate() {
        //     if let SiderealMultiple::Multiple(m) = speed.sidereal_multiple {
        //         sss[idx] = mount.slewing_speed_supported(m * mount::SIDEREAL_RATE);
        //         info += &format!("{:.0}x, ", m);
        //     } else {
        //         sss[idx] = true;
        //     }
        // }
        // log::info!("{}", info);
    }

    fn on_disconnect(&self)
    {
        self.wbox.set_sensitive(false);
        self.status.set_text("disconnected");
        // self.disable_sky_tracking_btn();
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
            match connection_dialog::show_focuser_connect_dialog(&program_data_rc) {
                Some(connection) => {
                    match focuser::connect_to_focuser(connection) {
                        Err(e) => show_message(
                            &format!("Failed to connect to focuser: {:?}.", e),
                            "Error",
                            gtk::MessageType::Error,
                            &program_data_rc
                        ),
                        Ok(focuser) => {
                            log::info!("connected to {}", focuser.get().info());
                            program_data_rc.borrow().gui.as_ref().unwrap().focuser_widgets.on_connect(&focuser);
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
    log::info!("attempted focuser move with speed {:.02}", speed.get()); //TESTING #######
    let res = program_data_rc.borrow_mut().focuser_data.focuser.as_mut().unwrap().move_in_dir(speed, dir);
    if let Err(e) = &res { /*TODO on_mount_error(e, program_data_rc)*/ }
    res.map_err(|_| ())
}

pub fn create_focuser_box(program_data_rc: &Rc<RefCell<ProgramData>>) -> FocuserWidgets {
    let contents = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let upper_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    //upper_box.pack_start(&gtk::Label::new(Some("Slewing speed:")), false, false, PADDING);

    // let model = gtk::ListStore::new(&[gtk::glib::Type::STRING]);
    // for (idx, speed) in SLEWING_SPEEDS.iter().enumerate() {
    //     model.insert_with_values(Some(idx as u32), &[(0u32, &speed.label)]);
    // }
    // let slew_speed = gtk::ComboBox::with_model(&model);
    // let renderer = gtk::CellRendererText::new();
    // slew_speed.pack_start(&renderer, true);
    // slew_speed.add_attribute(&renderer, "text", 0);
    // let slew_speed_supported = Rc::new(RefCell::new([false; SLEWING_SPEEDS.len()]));
    // slew_speed.set_cell_data_func(&renderer, Some(Box::new(
    //     clone!(@weak slew_speed_supported => @default-panic, move |_, cell, model, iter| {
    //         let path = model.path(iter).unwrap();
    //         cell.set_sensitive(slew_speed_supported.borrow()[path.indices()[0] as usize]);
    //     })
    // )));
    // slew_speed.set_active(Some(0));

    // upper_box.pack_start(&slew_speed, false, false, PADDING);

    // let btn_calibrate = gtk::ButtonBuilder::new()
    //     .label("calibrate")
    //     .tooltip_text("Calibrate guiding by establishing mount-camera orientation (uses the selected slewing speed)")
    //     .build();
    // btn_calibrate.connect_clicked(clone!(@weak program_data_rc
    //     => @default-panic, move |btn| on_start_calibration(btn, &program_data_rc))
    // );
    // upper_box.pack_end(&btn_calibrate, false, false, PADDING);

    let btn_move_neg = gtk::Button::with_label("←");
    btn_move_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(1.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    btn_move_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(0.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    upper_box.pack_start(&btn_move_neg, false, false, PADDING);

    let btn_move_pos = gtk::Button::with_label("→");
    btn_move_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(1.0), focuser::FocuserDir::Positive, &program_data_rc).is_err())
    }));
    btn_move_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(focuser_move(focuser::Speed::new(0.0), focuser::FocuserDir::Negative, &program_data_rc).is_err())
    }));
    upper_box.pack_start(&btn_move_pos, false, false, PADDING);

    let btn_stop = gtk::Button::with_label("stop");
    // btn_stop.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| on_stop(&program_data_rc)));
    upper_box.pack_end(&btn_stop, false, false, PADDING);

    // let btn_sky_tracking = gtk::ToggleButtonBuilder::new()
    //     .label("🌐 ⟳")
    //     .tooltip_text("Enable sky tracking")
    //     .build();

    // let signal_sky_tracking = btn_sky_tracking.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
    //     on_toggle_sky_tracking(btn, &program_data_rc);
    // }));

    // upper_box.pack_end(&btn_sky_tracking, false, false, PADDING);

    contents.pack_start(&upper_box, false, false, PADDING);

    // let (primary_neg, secondary_pos, secondary_neg, primary_pos) = create_direction_buttons(program_data_rc);

    // let dir_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    // dir_box.pack_start(&primary_neg, true, true, 0);
    // dir_box.pack_start(&secondary_pos, true, true, 0);
    // dir_box.pack_start(&secondary_neg, true, true, 0);
    // dir_box.pack_start(&primary_pos, true, true, 0);
    // contents.pack_start(&dir_box, false, false, PADDING);

    // let lower_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    // lower_box.pack_start(&gtk::Label::new(Some("Guiding speed:")), false, false, PADDING);

    // let guide_speed = gtk::ComboBoxText::new();
    // for speed in GUIDING_SPEEDS {
    //     guide_speed.append_text(&speed.label);
    // }
    // guide_speed.set_active(Some(3));
    // lower_box.pack_start(&guide_speed, false, false, PADDING);

    // let btn_guide = gtk::ToggleButtonBuilder::new()
    //     .label("guide")
    //     .tooltip_text("Enable guiding")
    //     .build();
    // let signal_guide = btn_guide.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
    //     if btn.is_active() {
    //         guiding::start_guiding(&program_data_rc);
    //     } else {
    //         if let Err(e) = guiding::stop_guiding(&program_data_rc) {
    //             on_mount_error(&e, &program_data_rc);
    //         }
    //     }
    // }));
    // lower_box.pack_start(&btn_guide, false, false, PADDING);

    // contents.pack_start(&lower_box, false, false, PADDING);

    let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let status_label = gtk::LabelBuilder::new().justify(gtk::Justification::Left).label("disconnected").build();
    status_box.pack_start(&status_label, false, false, PADDING);
    contents.pack_end(&status_box, false, false, PADDING);

    contents.set_sensitive(false);

    FocuserWidgets{
        wbox: contents,
        status: status_label,
    }
}
