//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope mount GUI.
//!

#[cfg(feature = "mount_ascom")]
mod ascom;
mod ioptron;
mod simulator;
mod skywatcher;
pub mod connection_dialog;

use cgmath::{Point2, Vector2, InnerSpace};
use crate::{MountCalibration, ProgramData};
use crate::guiding;
use crate::mount;
use crate::mount::RadPerSec;
use glib::{clone};
use gtk::prelude::*;
use std::{cell::RefCell, error::Error, rc::Rc};
use super::show_message;

/// Control padding in pixels.
const PADDING: u32 = 10;

/// Time of slewing in each axis when calibrating for guiding.
const CALIBRATION_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

struct SlewingSpeed {
    sidereal_multiple: f64,
    label: &'static str
}

const SLEWING_SPEEDS: &'static [SlewingSpeed] = &[
    SlewingSpeed{ sidereal_multiple: 0.25, label: "0.25x" },
    SlewingSpeed{ sidereal_multiple: 0.5,  label: "0.5x" },
    SlewingSpeed{ sidereal_multiple: 1.0,  label: "1x" },
    SlewingSpeed{ sidereal_multiple: 2.0,  label: "2x" },
    SlewingSpeed{ sidereal_multiple: 4.0,  label: "4x" },
    SlewingSpeed{ sidereal_multiple: 8.0,  label: "8x" },
    SlewingSpeed{ sidereal_multiple: 16.0, label: "16x" },
    SlewingSpeed{ sidereal_multiple: 32.0, label: "32x" }
];

const GUIDING_SPEEDS: &'static [SlewingSpeed] = &[
    SlewingSpeed{ sidereal_multiple: 0.1,  label: "0.1x" },
    SlewingSpeed{ sidereal_multiple: 0.25, label: "0.25x" },
    SlewingSpeed{ sidereal_multiple: 0.4,  label: "0.4x" },
    SlewingSpeed{ sidereal_multiple: 0.5,  label: "0.5x" },
    SlewingSpeed{ sidereal_multiple: 0.75, label: "0.75x" },
    SlewingSpeed{ sidereal_multiple: 0.9,  label: "0.9x" }
];

pub struct MountWidgets {
    wbox: gtk::Box,
    status: gtk::Label,
    /// Button and its "activate" signal.
    sky_tracking: (gtk::ToggleButton, glib::SignalHandlerId),
    /// Button and its "activate" signal.
    guide: (gtk::ToggleButton, glib::SignalHandlerId),
    calibrate: gtk::Button,
    slew_speed: gtk::ComboBoxText,
    guide_speed: gtk::ComboBoxText
}

impl MountWidgets {
    pub fn wbox(&self) -> &gtk::Box { &self.wbox }

    pub fn on_target_tracking_ended(&self, reenable_calibration_button: bool) {
        self.disable_guide();
        if reenable_calibration_button {
            self.calibrate.set_sensitive(true);
        }
    }

    fn on_connect(&self, mount_info: &str, _tracking_enabled: bool)
    {
        self.wbox.set_sensitive(true);
        self.status.set_text(&format!("{}", mount_info));
    }

    fn on_disconnect(&self)
    {
        self.wbox.set_sensitive(false);
        self.status.set_text("disconnected");
        self.disable_sky_tracking_btn();
    }

    fn disable_sky_tracking_btn(&self) {
        let (btn_tracking, signal) = &self.sky_tracking;
        btn_tracking.block_signal(signal);
        btn_tracking.set_active(false);
        btn_tracking.unblock_signal(signal);
    }

    /// Returns slewing speed (multiple of sidereal rate) selected in combo box.
    fn slew_speed(&self) -> f64 {
        SLEWING_SPEEDS[self.slew_speed.active().unwrap() as usize].sidereal_multiple
    }

    /// Returns guiding speed (multiple of sidereal rate) selected in combo box.
    pub fn guide_speed(&self) -> f64 {
        GUIDING_SPEEDS[self.guide_speed.active().unwrap() as usize].sidereal_multiple
    }

    pub fn disable_guide(&self) {
        let (btn_guide, signal) = &self.guide;
        btn_guide.block_signal(signal);
        btn_guide.set_active(false);
        btn_guide.unblock_signal(signal);
    }
}

fn on_stop(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().stop();
    if let Err(e) = &res {
        on_mount_error(e);
        return;
    }

    let mut pd = program_data_rc.borrow_mut();

    pd.mount_data.guide_slewing = false;
    pd.mount_data.guiding_timer.stop();
    pd.mount_data.guiding_pos = None;
    pd.gui.as_ref().unwrap().mount_widgets.disable_guide();

    if pd.mount_data.calibration_in_progress() {
        pd.mount_data.calibration_timer.stop();
        pd.mount_data.calibration = None;
        pd.gui.as_ref().unwrap().mount_widgets.calibrate.set_sensitive(true);
    }

    if pd.mount_data.sky_tracking_on {
        pd.mount_data.sky_tracking_on = false;
        pd.gui.as_ref().unwrap().mount_widgets.disable_sky_tracking_btn();
        log::info!("sky tracking disabled");
    }
}

fn on_start_calibration(btn: &gtk::Button, program_data_rc: &Rc<RefCell<ProgramData>>) {
    if program_data_rc.borrow().tracking.is_none() {
        show_message("Target tracking is not enabled.", "Error", gtk::MessageType::Error);
        return;
    }

    {
        let mut pd = program_data_rc.borrow_mut();
        pd.mount_data.calibration = Some(MountCalibration{
            origin: pd.tracking.as_ref().unwrap().pos,
            primary_dir: None,
            secondary_dir: None,
            img_to_mount_axes: None
        });
    }

    let slew_speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed() * mount::SIDEREAL_RATE;
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(mount::Axis::Primary, slew_speed);
    if let Err(e) = &res {
        on_mount_error(e);
    } else {
        program_data_rc.borrow_mut().mount_data.calibration_timer.run_once(
            CALIBRATION_DURATION,
            clone!(@weak program_data_rc => @default-panic, move || { on_calibration_timer(&program_data_rc); }
        ));
        btn.set_sensitive(false);
    }
}

fn on_calibration_timer(program_data_rc: &Rc<RefCell<ProgramData>>) {
    if program_data_rc.borrow().mount_data.calibration.is_none() { return; }

    const MIN_VECTOR_LENGTH: i32 = 50;
    let must_show_error: RefCell<Option<String>> = RefCell::new(None);
    loop { // `program_data_rc` borrow starts
        let mut pd = program_data_rc.borrow_mut();
        let tracking_pos = pd.tracking.as_ref().unwrap().pos;

        let sd_on = pd.mount_data.sky_tracking_on;
        if pd.mount_data.calibration.as_ref().unwrap().primary_dir.is_none() {
            let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Primary, RadPerSec(0.0));
            if let Err(e) = &res { must_show_error.replace(Some(mount_error_msg(e))); break; }
        } else {
            let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, RadPerSec(0.0));
            if let Err(e) = &res { must_show_error.replace(Some(mount_error_msg(e))); break; }
        }

        let dir_getter = || -> Option<Vector2<f64>> {
            let delta = tracking_pos - pd.mount_data.calibration.as_ref().unwrap().origin;
            let len_sq = delta.magnitude2();
            if len_sq < MIN_VECTOR_LENGTH.pow(2) {
                must_show_error.replace(Some(
                    format!("Calibration failed: image moved by less than {} pixels.\n \
                        Try increasing the slewing speed.", MIN_VECTOR_LENGTH)
                ));
                None
            } else {
                let len = (len_sq as f64).sqrt();
                Some(Vector2{ x: delta.x as f64 / len, y: delta.y as f64 / len })
            }
        };

        if pd.mount_data.calibration.as_ref().unwrap().primary_dir.is_none() {
            if let Some(dir) = dir_getter() {
                pd.mount_data.calibration.as_mut().unwrap().primary_dir = Some(dir);

                let slew_speed = pd.gui.as_ref().unwrap().mount_widgets.slew_speed() * mount::SIDEREAL_RATE;
                let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, slew_speed);
                if let Err(e) = &res {
                    must_show_error.replace(Some(mount_error_msg(e)));
                    break;
                } else {
                    pd.mount_data.calibration.as_mut().unwrap().origin = pd.tracking.as_ref().unwrap().pos;
                    pd.mount_data.calibration_timer.run_once(
                        CALIBRATION_DURATION,
                        clone!(@weak program_data_rc => @default-panic, move || {
                            on_calibration_timer(&program_data_rc);
                        })
                    );
                }
            }
        } else {
            pd.mount_data.calibration.as_mut().unwrap().secondary_dir = dir_getter();

            let (primary_dir, secondary_dir) = (
                *pd.mount_data.calibration.as_mut().unwrap().primary_dir.as_ref().unwrap(),
                *pd.mount_data.calibration.as_mut().unwrap().secondary_dir.as_ref().unwrap()
            );

            match guiding::create_img_to_mount_axes_matrix(primary_dir, secondary_dir) {
                Ok(matrix) => { pd.mount_data.calibration.as_mut().unwrap().img_to_mount_axes = Some(matrix); },
                _ => {
                    must_show_error.replace(
                        Some("Mount-axes-to-image transformation matrix is non-invertible.".to_string())
                    );
                    break;
                }
            }
        }

        break;
    } // `program_data_rc` borrow ends

    if let Some(msg) = must_show_error.take() {
        program_data_rc.borrow_mut().mount_data.calibration = None;
        show_message(&msg, "Error", gtk::MessageType::Error);
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.calibrate.set_sensitive(true);
    } else {
        let calibration_finished = program_data_rc.borrow().mount_data.calibration.as_ref().unwrap().secondary_dir.is_some();
        if calibration_finished {
            show_message("Calibration completed.", "Information", gtk::MessageType::Info);
            program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.calibrate.set_sensitive(true);
        }
    }
}

pub fn create_mount_box(program_data_rc: &Rc<RefCell<ProgramData>>) -> MountWidgets {
    let contents = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let upper_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    upper_box.pack_start(&gtk::Label::new(Some("Slewing speed:")), false, false, PADDING);
    let slew_speed = gtk::ComboBoxText::new();
    for speed in SLEWING_SPEEDS {
        slew_speed.append_text(&speed.label); // TODO: disable rates unsupported by mount
    }
    slew_speed.set_active(Some(2));
    upper_box.pack_start(&slew_speed, false, false, PADDING);

    let btn_calibrate = gtk::ButtonBuilder::new()
        .label("calibrate")
        .tooltip_text("Calibrate guiding by establishing mount-camera orientation (uses the selected slewing speed)")
        .build();
    btn_calibrate.connect_clicked(clone!(@weak program_data_rc
        => @default-panic, move |btn| on_start_calibration(btn, &program_data_rc))
    );
    upper_box.pack_end(&btn_calibrate, false, false, PADDING);

    let btn_stop = gtk::Button::with_label("stop");
    btn_stop.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| on_stop(&program_data_rc)));
    upper_box.pack_end(&btn_stop, false, false, PADDING);

    let btn_sky_tracking = gtk::ToggleButtonBuilder::new()
        .label("🌐 ⟳")
        .tooltip_text("Enable sky tracking")
        .build();

    let signal_sky_tracking = btn_sky_tracking.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        on_toggle_sky_tracking(btn, &program_data_rc);
    }));

    upper_box.pack_end(&btn_sky_tracking, false, false, PADDING);

    contents.pack_start(&upper_box, false, false, PADDING);

    let (primary_neg, secondary_pos, secondary_neg, primary_pos) = create_direction_buttons(program_data_rc);

    let dir_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dir_box.pack_start(&primary_neg, true, true, 0);
    dir_box.pack_start(&secondary_pos, true, true, 0);
    dir_box.pack_start(&secondary_neg, true, true, 0);
    dir_box.pack_start(&primary_pos, true, true, 0);
    contents.pack_start(&dir_box, false, false, PADDING);

    let lower_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    lower_box.pack_start(&gtk::Label::new(Some("Guiding speed:")), false, false, PADDING);

    let guide_speed = gtk::ComboBoxText::new();
    for speed in GUIDING_SPEEDS {
        guide_speed.append_text(&speed.label);
    }
    guide_speed.set_active(Some(3));
    lower_box.pack_start(&guide_speed, false, false, PADDING);

    let btn_guide = gtk::ToggleButtonBuilder::new()
        .label("guide")
        .tooltip_text("Enable guiding")
        .build();
    let signal_guide = btn_guide.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() {
            guiding::start_guiding(&program_data_rc);
        } else {
            if let Err(e) = guiding::stop_guiding(&program_data_rc) {
                on_mount_error(&e);
            }
        }
    }));
    lower_box.pack_start(&btn_guide, false, false, PADDING);

    contents.pack_start(&lower_box, false, false, PADDING);

    let status_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let status_label = gtk::LabelBuilder::new().justify(gtk::Justification::Left).label("disconnected").build();
    status_box.pack_start(&status_label, false, false, PADDING);
    contents.pack_end(&status_box, false, false, PADDING);

    contents.set_sensitive(false);

    MountWidgets{
        wbox: contents,
        status: status_label,
        sky_tracking: (btn_sky_tracking, signal_sky_tracking),
        guide: (btn_guide, signal_guide),
        calibrate: btn_calibrate,
        slew_speed,
        guide_speed
    }
}

fn mount_error_msg(e: &Box<dyn Error>) -> String {
    format!("Error communicating with mount: {}.", e)
}

/// Shows mount error message.
///
/// Active borrows of `program_data` *must not be held* when calling this function.
///
pub fn on_mount_error(e: &Box<dyn Error>) {
    show_message(&mount_error_msg(e), "Error", gtk::MessageType::Error);
}

fn on_toggle_sky_tracking(btn: &gtk::ToggleButton, program_data_rc: &Rc<RefCell<ProgramData>>) {
    // TODO: if ST was enabled, abort calibration; or do not allow toggling ST during calibration

    let enable_tracking = btn.is_active();
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().set_tracking(enable_tracking);
    if let Err(e) = &res {
        btn.set_active(!enable_tracking);
        on_mount_error(e);
    }

    if !btn.is_active() {
        let _ = guiding::stop_guiding(program_data_rc);
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets().disable_guide();
    }

    program_data_rc.borrow_mut().mount_data.sky_tracking_on = btn.is_active();
    log::info!("sky tracking {}", if btn.is_active() { "enabled" } else { "disabled" });
}

/// Returns slewing buttons: (Primary-, Secondary+, Secondary-, Primary+).
fn create_direction_buttons(program_data_rc: &Rc<RefCell<ProgramData>>)
-> (gtk::Button, gtk::Button, gtk::Button, gtk::Button) {

    let zero = RadPerSec(0.0);

    let dir_primary_neg = gtk::Button::with_label("← Axis 1");
    dir_primary_neg.set_tooltip_text(Some("Primary axis negative slew"));
    dir_primary_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
            mount::Axis::Primary, -speed * mount::SIDEREAL_RATE
        );
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(res.is_err())
    }));
    dir_primary_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(mount::Axis::Primary, zero);
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(false)
    }));

    let dir_secondary_pos = gtk::Button::with_label("↑ Axis 2");
    dir_secondary_pos.set_tooltip_text(Some("Secondary axis positive slew"));
    dir_secondary_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
            mount::Axis::Secondary, speed * mount::SIDEREAL_RATE
        );
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(res.is_err())
    }));
    dir_secondary_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, zero);
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(false)
    }));

    let dir_secondary_neg = gtk::Button::with_label("↓ Axis 2");
    dir_secondary_neg.set_tooltip_text(Some("Secondary axis negative slew"));
    dir_secondary_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
            mount::Axis::Secondary, -speed * mount::SIDEREAL_RATE
        );
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(res.is_err())
    }));
    dir_secondary_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, zero);
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(false)
    }));

    let dir_primary_pos = gtk::Button::with_label("→ Axis 1");
    dir_primary_pos.set_tooltip_text(Some("Primary axis positive slew"));
    dir_primary_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
            mount::Axis::Primary, speed * mount::SIDEREAL_RATE
        );
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(res.is_err())
    }));
    dir_primary_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(mount::Axis::Primary, zero);
        if let Err(e) = &res { on_mount_error(e) }
        gtk::Inhibit(false)
    }));

    (dir_primary_neg, dir_secondary_pos, dir_secondary_neg, dir_primary_pos)
}

pub fn init_mount_menu(program_data_rc: &Rc<RefCell<ProgramData>>, app_window: &gtk::ApplicationWindow) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let item_disconnect = gtk::MenuItem::with_label("Disconnect");
    item_disconnect.connect_activate(clone!(@weak program_data_rc => @default-panic, move |menu_item| {
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.on_disconnect();
        {
            let mut pd = program_data_rc.borrow_mut();
            let mount_info = pd.mount_data.mount.as_ref().unwrap().get_info();
            pd.mount_data.mount = None;
            pd.mount_data.sky_tracking_on = false;
            pd.mount_data.calibration = None;
            pd.gui.as_ref().unwrap().mount_widgets.on_disconnect();
            log::info!("disconnected from {}", mount_info);
        }
        menu_item.set_sensitive(false);
    }));
    item_disconnect.set_sensitive(false);

    let item_connect = gtk::MenuItem::with_label("Connect...");
    item_connect.connect_activate(clone!(
        @weak program_data_rc,
        @weak app_window,
        @weak item_disconnect
        => @default-panic, move |_| {
            match connection_dialog::show_mount_connect_dialog(&app_window, &program_data_rc) {
                Some(connection) => {
                    match mount::connect_to_mount(connection) {
                        Err(e) => show_message(
                            &format!("Failed to connect to mount: {:?}.", e),
                            "Error",
                            gtk::MessageType::Error
                        ),
                        Ok(mut mount) => {
                            log::info!("connected to {}", mount.get_info());
                            let target_tracking_enabled = program_data_rc.borrow().tracking.is_some();
                            program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.on_connect(
                                &mount.get_info(),
                                target_tracking_enabled
                            );
                            mount.set_mount_simulator_data(program_data_rc.borrow().mount_simulator_data.clone());
                            program_data_rc.borrow_mut().mount_data.mount = Some(mount);
                            program_data_rc.borrow_mut().mount_data.calibration = None;
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
