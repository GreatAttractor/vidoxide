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
pub mod ascom;
pub mod ioptron;
pub mod simulator;
pub mod skywatcher;
pub mod zwo;

use cgmath::{Point2, Vector2, InnerSpace};
use crate::{devices::{DeviceConnectionDiscriminants, DeviceType}, MountCalibration, ProgramData};
use crate::{devices::focuser, gui::{device_connection_dialog, show_message}, guiding, mount, mount::RadPerSec};
use glib::{clone};
use gtk::prelude::*;
use std::{cell::RefCell, error::Error, rc::Rc};
use strum::IntoEnumIterator;

/// Control padding in pixels.
const PADDING: u32 = 10;

/// Time of slewing in each axis when calibrating for guiding.
const CALIBRATION_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone)]
pub enum SiderealMultiple {
    Multiple(f64),
    Max, // max slewing speed supported by mount
}

#[derive(Clone)]
struct SlewingSpeed {
    sidereal_multiple: SiderealMultiple,
    label: &'static str
}

const SLEWING_SPEEDS: &'static [SlewingSpeed] = &[
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(1.0),    label: "1x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(2.0),    label: "2x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(4.0),    label: "4x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(8.0),    label: "8x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(16.0),   label: "16x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(20.0),   label: "20x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(32.0),   label: "32x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(60.0),   label: "60x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(64.0),   label: "64x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(128.0),  label: "128x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(256.0),  label: "256x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(512.0),  label: "512x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(720.0),  label: "720x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Max,              label: "MAX" },
];

const GUIDING_SPEEDS: &'static [SlewingSpeed] = &[
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.1),  label: "0.1x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.25), label: "0.25x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.4),  label: "0.4x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.5),  label: "0.5x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.75), label: "0.75x" },
    SlewingSpeed{ sidereal_multiple: SiderealMultiple::Multiple(0.9),  label: "0.9x" }
];

pub struct MountWidgets {
    wbox: gtk::Box,
    status: gtk::Label,
    /// Button and its "activate" signal.
    sky_tracking: (gtk::ToggleButton, glib::SignalHandlerId),
    /// Button and its "activate" signal.
    guide: (gtk::ToggleButton, glib::SignalHandlerId),
    calibrate: gtk::Button,
    slew_speed: gtk::ComboBox,
    guide_speed: gtk::ComboBoxText,
    /// Elements correspond to `SLEWING_SPEEDS`.
    slew_speed_supported: Rc<RefCell<[bool; SLEWING_SPEEDS.len()]>>
}

impl MountWidgets {
    pub fn wbox(&self) -> &gtk::Box { &self.wbox }

    pub fn on_target_tracking_ended(&self, reenable_calibration_button: bool) {
        self.disable_guide();
        if reenable_calibration_button {
            self.calibrate.set_sensitive(true);
        }
    }

    fn on_connect(&self, mount: &Box<dyn mount::Mount>, _tracking_enabled: bool)
    {
        self.wbox.set_sensitive(true);
        self.status.set_text(&format!("{}", mount.get_info()));
        let mut sss = self.slew_speed_supported.borrow_mut();
        let mut info = "supported slewing speeds: ".to_string();
        for (idx, speed) in SLEWING_SPEEDS.iter().enumerate() {
            if let SiderealMultiple::Multiple(m) = speed.sidereal_multiple {
                sss[idx] = mount.slewing_speed_supported(m * mount::SIDEREAL_RATE);
                info += &format!("{:.0}x, ", m);
            } else {
                sss[idx] = true;
            }
        }
        log::info!("{}", info);
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
    fn slew_speed(&self) -> SiderealMultiple {
        SLEWING_SPEEDS[self.slew_speed.active().unwrap() as usize].sidereal_multiple.clone()
    }

    /// Returns guiding speed (multiple of sidereal rate) selected in combo box.
    pub fn guide_speed(&self) -> f64 {
        match GUIDING_SPEEDS[self.guide_speed.active().unwrap() as usize].sidereal_multiple {
            SiderealMultiple::Multiple(value) => value,
            _ => unreachable!()
        }
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
        on_mount_error(e, program_data_rc);
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
        show_message("Target tracking is not enabled.", "Error", gtk::MessageType::Error, program_data_rc);
        return;
    }

    let selected_speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
    if match selected_speed {
        SiderealMultiple::Multiple(s) => s > 16.0,
        SiderealMultiple::Max => true
    } {
        show_message(
            "Selected slewing speed is too high for calibration.", "Error", gtk::MessageType::Error, program_data_rc
        );
        return;
    }

    let selected_multiple = if let SiderealMultiple::Multiple(s) = selected_speed { s } else { unreachable!() };

    {
        let mut pd = program_data_rc.borrow_mut();
        pd.mount_data.calibration = Some(MountCalibration{
            origin: pd.tracking.as_ref().unwrap().pos,
            primary_dir: None,
            secondary_dir: None,
            img_to_mount_axes: None,
            calibration_slew_speed: selected_multiple * mount::SIDEREAL_RATE
        });
    }

    let slew_speed = selected_multiple * mount::SIDEREAL_RATE;
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
        mount::Axis::Primary,
        mount::SlewSpeed::Specific(slew_speed)
    );
    if let Err(e) = &res {
        program_data_rc.borrow_mut().mount_data.calibration = None;
        on_mount_error(e, program_data_rc);
    } else {
        program_data_rc.borrow_mut().mount_data.calibration_timer.run(
            CALIBRATION_DURATION,
            true,
            clone!(@weak program_data_rc => @default-panic, move || { on_calibration_timer(&program_data_rc); }
        ));
        btn.set_sensitive(false);
    }
}

fn on_calibration_timer(program_data_rc: &Rc<RefCell<ProgramData>>) {
    if program_data_rc.borrow().mount_data.calibration.is_none() { return; }

    const MIN_VECTOR_LENGTH: i32 = 50;
    let must_show_error: RefCell<Option<String>> = RefCell::new(None);
    'block: { // `program_data_rc` borrow starts
        let mut pd = program_data_rc.borrow_mut();
        let tracking_pos = pd.tracking.as_ref().unwrap().pos;

        if pd.mount_data.calibration.as_ref().unwrap().primary_dir.is_none() {
            let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Primary, mount::SlewSpeed::zero());
            if let Err(e) = &res { must_show_error.replace(Some(mount_error_msg(e))); break 'block; }
        } else {
            let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, mount::SlewSpeed::zero());
            if let Err(e) = &res { must_show_error.replace(Some(mount_error_msg(e))); break 'block; }
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

                let slew_speed =
                    mount::SlewSpeed::Specific(pd.mount_data.calibration.as_ref().unwrap().calibration_slew_speed);
                let res = pd.mount_data.mount.as_mut().unwrap().slew(mount::Axis::Secondary, slew_speed);
                if let Err(e) = &res {
                    must_show_error.replace(Some(mount_error_msg(e)));
                    break 'block;
                } else {
                    pd.mount_data.calibration.as_mut().unwrap().origin = pd.tracking.as_ref().unwrap().pos;
                    pd.mount_data.calibration_timer.run(
                        CALIBRATION_DURATION,
                        true,
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
                    break 'block;
                }
            }
        }

    } // `program_data_rc` borrow ends

    if let Some(msg) = must_show_error.take() {
        program_data_rc.borrow_mut().mount_data.calibration = None;
        show_message(&msg, "Error", gtk::MessageType::Error, program_data_rc);
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.calibrate.set_sensitive(true);
    } else {
        let calibration_finished = program_data_rc.borrow().mount_data.calibration.as_ref().unwrap().secondary_dir.is_some();
        if calibration_finished {
            show_message("Calibration completed.", "Information", gtk::MessageType::Info, program_data_rc);
            program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.calibrate.set_sensitive(true);
        }
    }
}

pub fn create_mount_box(program_data_rc: &Rc<RefCell<ProgramData>>) -> MountWidgets {
    let contents = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let upper_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    upper_box.pack_start(&gtk::Label::new(Some("Slewing speed:")), false, false, PADDING);

    let model = gtk::ListStore::new(&[gtk::glib::Type::STRING]);
    for (idx, speed) in SLEWING_SPEEDS.iter().enumerate() {
        model.insert_with_values(Some(idx as u32), &[(0u32, &speed.label)]);
    }
    let slew_speed = gtk::ComboBox::with_model(&model);
    let renderer = gtk::CellRendererText::new();
    slew_speed.pack_start(&renderer, true);
    slew_speed.add_attribute(&renderer, "text", 0);
    let slew_speed_supported = Rc::new(RefCell::new([false; SLEWING_SPEEDS.len()]));
    slew_speed.set_cell_data_func(&renderer, Some(Box::new(
        clone!(@weak slew_speed_supported => @default-panic, move |_, cell, model, iter| {
            let path = model.path(iter).unwrap();
            cell.set_sensitive(slew_speed_supported.borrow()[path.indices()[0] as usize]);
        })
    )));
    slew_speed.set_active(Some(0));

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
                on_mount_error(&e, &program_data_rc);
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
        guide_speed,
        slew_speed_supported
    }
}

fn mount_error_msg(e: &Box<dyn Error>) -> String {
    format!("Error communicating with mount: {}.", e)
}

/// Shows mount error message.
///
/// Active borrows of `program_data` *must not be held* when calling this function.
///
pub fn on_mount_error(e: &Box<dyn Error>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    show_message(&mount_error_msg(e), "Error", gtk::MessageType::Error, program_data_rc);
}

fn on_toggle_sky_tracking(btn: &gtk::ToggleButton, program_data_rc: &Rc<RefCell<ProgramData>>) {
    // TODO: if ST was enabled, abort calibration; or do not allow toggling ST during calibration

    let enable_tracking = btn.is_active();
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().set_tracking(enable_tracking);
    if let Err(e) = &res {
        btn.set_active(!enable_tracking);
        on_mount_error(e, program_data_rc);
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
    let dir_primary_neg = gtk::Button::with_label("← Axis 1");
    dir_primary_neg.set_tooltip_text(Some("Primary axis negative slew"));
    dir_primary_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Primary, false, true, &program_data_rc).is_err())
    }));
    dir_primary_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Primary, false, false, &program_data_rc).is_err())
    }));

    let dir_secondary_pos = gtk::Button::with_label("↑ Axis 2");
    dir_secondary_pos.set_tooltip_text(Some("Secondary axis positive slew"));
    dir_secondary_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Secondary, true, true, &program_data_rc).is_err())
    }));
    dir_secondary_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Secondary, true, false, &program_data_rc).is_err())
    }));

    let dir_secondary_neg = gtk::Button::with_label("↓ Axis 2");
    dir_secondary_neg.set_tooltip_text(Some("Secondary axis negative slew"));
    dir_secondary_neg.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Secondary, false, true, &program_data_rc).is_err())
    }));
    dir_secondary_neg.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Secondary, false, false, &program_data_rc).is_err())
    }));

    let dir_primary_pos = gtk::Button::with_label("→ Axis 1");
    dir_primary_pos.set_tooltip_text(Some("Primary axis positive slew"));
    dir_primary_pos.connect_button_press_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Primary, true, true, &program_data_rc).is_err())
    }));
    dir_primary_pos.connect_button_release_event(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        gtk::Inhibit(axis_slew(mount::Axis::Primary, true, false, &program_data_rc).is_err())
    }));

    (dir_primary_neg, dir_secondary_pos, dir_secondary_neg, dir_primary_pos)
}



pub fn init_mount_menu(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Menu {
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

    let mount_connections: Vec<DeviceConnectionDiscriminants> =
        DeviceConnectionDiscriminants::iter().filter(|d| d.device_type() == DeviceType::Mount).collect();

    item_connect.connect_activate(clone!(
        @weak program_data_rc,
        @weak item_disconnect
        => @default-panic, move |_| {
            match device_connection_dialog::show_device_connection_dialog(
                "Connect to mount",
                "Mount type:",
                &program_data_rc,
                &mount_connections
            ) {
                Some(connection) => {
                    match mount::connect_to_mount(connection) {
                        Err(e) => show_message(
                            &format!("Failed to connect to mount: {:?}.", e),
                            "Error",
                            gtk::MessageType::Error,
                            &program_data_rc
                        ),
                        Ok(mut mount) => {
                            log::info!("connected to {}", mount.get_info());
                            let target_tracking_enabled = program_data_rc.borrow().tracking.is_some();
                            program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.on_connect(
                                &mount,
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

pub fn axis_slew(axis: mount::Axis, positive: bool, enable: bool, program_data_rc: &Rc<RefCell<ProgramData>>) -> Result<(), ()> {
    let speed = program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets.slew_speed();
    let res = program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().slew(
        axis,
        if enable {
            match speed {
                SiderealMultiple::Multiple(s) => mount::SlewSpeed::Specific(if positive { 1.0 } else { -1.0 } * s * mount::SIDEREAL_RATE),
                SiderealMultiple::Max => mount::SlewSpeed::Max(positive)
            }
        } else {
            mount::SlewSpeed::zero()
        }
    );
    if let Err(e) = &res { on_mount_error(e, program_data_rc) }

    res.map_err(|_| ())
}
