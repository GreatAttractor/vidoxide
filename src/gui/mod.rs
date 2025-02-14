//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2025 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! GUI module.
//!

mod actions;
mod basic_connection_controls;
mod camera_gui;
mod checked_listbox;
#[cfg(feature = "controller")]
mod controller;
mod dec_intervals;
mod device_connection_dialog;
mod dispersion_dialog;
mod event_handlers;
mod focuser_gui;
mod freezeable;
mod histogram_utils;
mod histogram_view;
mod img_view;
mod initialization;
mod info_overlay;
mod mount_gui;
mod preview_processing;
mod psf_dialog;
mod rec_gui;
mod reticle_dialog;
mod roi_dialog;

use camera_gui::{
    CommonControlWidgets,
    ControlWidgetBundle,
    ListControlWidgets,
    NumberControlWidgets,
    BooleanControlWidgets
};
use cgmath::Point2;
#[cfg(feature = "controller")]
use controller::ControllerDialog;
use crate::ProgramData;
use crate::camera;
use crate::camera::CameraError;
use crate::devices::DeviceConnectionDiscriminants;
use dispersion_dialog::DispersionDialog;
use ga_image;
use ga_image::Rect;
use preview_processing::create_preview_processing_dialog;
use gtk::cairo;
use gtk::prelude::*;
use histogram_view::HistogramView;
use img_view::ImgView;
use info_overlay::{InfoOverlay, ScreenSelection, draw_info_overlay};
use focuser_gui::FocuserWidgets;
use mount_gui::MountWidgets;
use num_traits::cast::{FromPrimitive, AsPrimitive};
use psf_dialog::PsfDialog;
use rec_gui::RecWidgets;
use reticle_dialog::create_reticle_dialog;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

pub use basic_connection_controls::BasicConnectionControls;
#[cfg(feature = "controller")]
pub use crate::controller::on_controller_event;
pub use event_handlers::{
    on_capture_thread_message,
    on_histogram_thread_message,
    on_recording_thread_message,
    on_timer
};
pub use focuser_gui::{focuser_move, set_up_focuser_move_action};
pub use initialization::init_main_window;
pub use mount_gui::{axis_slew, on_mount_error};

/// Control padding in pixels.
const PADDING: u32 = 10;

const MOUSE_BUTTON_LEFT: u32 = 1;
const MOUSE_BUTTON_RIGHT: u32 = 3;

const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 20.0;

const ZOOM_CHANGE_FACTOR: f64 = 1.10;



struct StatusBarFields {
    preview_fps: gtk::Label,
    capture_fps: gtk::Label,
    temperature: gtk::Label,
    current_recording_info: gtk::Label,
    recording_overview: gtk::Label
}

pub struct Reticle {
    enabled: bool,
    dialog: gtk::Dialog,
    diameter: f64,
    opacity: f64,
    step: f64,
    line_width: f64
}

#[derive(Copy, Clone)]
pub struct Decibel(f32);

impl Decibel {
    fn get_gain_factor(&self) -> f32 { 10.0f32.powf(self.0 / 10.0) }
}

/// Processing is applied to the whole image or only to histogram area, if set.
#[derive(Clone)]
pub struct PreviewProcessing {
    dialog: gtk::Dialog,
    gamma: f32,
    gain: Decibel,
    stretch_histogram: bool
}

impl PreviewProcessing {
    pub fn is_effective(&self) -> bool {
        self.gamma != 1.0 || self.gain.0 != 0.0 || self.stretch_histogram
    }
}

/// Current mode of behavior of the left mouse button for the preview area.
#[derive(Copy, Clone)]
enum MouseMode {
    None,
    SelectROI,
    SelectCentroidArea,
    PlaceTrackingAnchor,
    SelectCropArea,
    SelectHistogramArea,
    MeasureDistance
}

impl MouseMode {
    pub fn is_selection(&self) -> bool {
        match self {
            MouseMode::SelectROI
            | MouseMode::SelectCentroidArea
            | MouseMode::SelectCropArea
            | MouseMode::SelectHistogramArea
            | MouseMode::MeasureDistance => true,

            MouseMode::None
            | MouseMode::PlaceTrackingAnchor => false
        }
    }
}

pub struct Stabilization {
    toggle_button: gtk::ToggleToolButton,
    position: Point2<i32>
}

pub struct GuiData {
    app_window: gtk::ApplicationWindow,
    controls_box: gtk::Box,
    control_widgets: std::collections::HashMap<camera::CameraControlId, (CommonControlWidgets, ControlWidgetBundle)>,
    status_bar: StatusBarFields,
    /// Menu items and their "activate" signals.
    camera_menu_items: Vec<(gtk::CheckMenuItem, glib::SignalHandlerId)>,
    camera_menu: gtk::Menu,
    preview_area: ImgView,
    rec_widgets: RecWidgets,
    reticle: Reticle,
    stabilization: Stabilization,
    preview_processing: PreviewProcessing,
    #[cfg(feature = "controller")]
    controller_dialog: ControllerDialog,
    dispersion_dialog: DispersionDialog,
    psf_dialog: PsfDialog,
    focuser_widgets: FocuserWidgets,
    mount_widgets: MountWidgets,
    mouse_mode: MouseMode,
    info_overlay: InfoOverlay,
    default_mouse_mode_button: gtk::RadioToolButton,
    histogram_view: HistogramView,
    // We must store an action map ourselves (and not e.g. reuse `SimpleActionGroup`), because currently (0.14.0) with
    // `gio` one cannot access a group's action in a way allowing to change its enabled state.
    action_map: HashMap<&'static str, gtk::gio::SimpleAction>,
    window_contents: gtk::Paned
}

impl GuiData {
    pub fn focuser_widgets(&self) -> &FocuserWidgets { &self.focuser_widgets }

    pub fn mount_widgets(&self) -> &MountWidgets { &self.mount_widgets }

    #[cfg(feature = "controller")]
    pub fn controller_dialog(&self) -> &ControllerDialog { &self.controller_dialog }

    #[cfg(feature = "controller")]
    pub fn controller_dialog_mut(&mut self) -> &mut ControllerDialog { &mut self.controller_dialog }
}

struct DialogDestroyer {
    dialog: gtk::Dialog
}

impl DialogDestroyer {
    fn new(dialog: &gtk::Dialog) -> DialogDestroyer { DialogDestroyer{ dialog: dialog.clone() } }
}

impl Drop for DialogDestroyer {
    fn drop(&mut self) {
        // `close` is not sufficient if another modal dialog is shown subsequently (`self.dialog` would remain visible,
        // blocking the event loop and requiring clicking the titlebar close icon); `hide` works, but does not delete
        // the dialog
        unsafe { self.dialog.destroy(); }
    }
}

pub trait ConnectionCreator {
    fn controls(&self) -> &gtk::Box;

    fn create(
        &self,
        configuration: &crate::config::Configuration
    ) -> Result<crate::devices::DeviceConnection, Box<dyn Error>>;

    fn label(&self) -> &'static str;
}

pub fn make_creator(
    connection: crate::devices::DeviceConnectionDiscriminants,
    config: &crate::config::Configuration
) -> Box<dyn ConnectionCreator> {
    type DCD = DeviceConnectionDiscriminants;
    match connection {
        #[cfg(feature = "mount_ascom")]
        DCD::AscomMount => creators.push(ascom::AscomConnectionCreator::new(config)),

        DCD::MountSimulator => mount_gui::simulator::SimulatorConnectionCreator::new(config),

        DCD::SkyWatcherMountSerial => mount_gui::skywatcher::SWConnectionCreator::new(config),

        DCD::IoptronMountSerial => mount_gui::ioptron::IoptronConnectionCreator::new(config),

        DCD::ZWOMountSerial => mount_gui::zwo::ZWOConnectionCreator::new(config),

        DCD::DreamFocuserMini => focuser_gui::dream_focuser_mini::DreamFocuserMiniConnectionCreator::new(config),

        DCD::FocusCube3 => focuser_gui::focuscube3::FocusCube3ConnectionCreator::new(config),

        DCD::FocuserSimulator => focuser_gui::simulator::SimulatorConnectionCreator::new(config),

        _ => unimplemented!()

    }
}

/// WARNING: this recursively enters the main event loop until the message dialog closes; therefore active borrows
/// of `program_data_rc` MUST NOT be held when calling this function.
pub fn show_message(msg: &str, title: &str, msg_type: gtk::MessageType, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let dialog = gtk::MessageDialog::new(
        Some(&program_data_rc.borrow().gui.as_ref().unwrap().app_window),
        gtk::DialogFlags::MODAL,
        msg_type,
        gtk::ButtonsType::Close,
        msg
    );
    let _ddestr = DialogDestroyer::new(&dialog.clone().upcast());

    dialog.set_title(title);
    dialog.set_use_markup(true);
    dialog.run();
    dialog.close();
}

fn show_about_dialog(program_data_rc: &Rc<RefCell<ProgramData>>) {
    show_message(
        &format!(
            "<big><big><b>Vidoxide</b></big></big>\n\n\
            Copyright © 2020-2025 Filip Szczerek (ga.software@yahoo.com)\n\n\
            This project is licensed under the terms of the MIT license (see the LICENSE file for details).\n\n\
            version: {}\n\
            OS: {}",
            crate::VERSION_STRING,
            os_info::get()
        ),
        "About Vidoxide",
        gtk::MessageType::Info,
        program_data_rc
    );
}

fn update_preview_info(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();

    match program_data.preview_fps_last_timestamp {
        None => {
            program_data.preview_fps_last_timestamp = Some(std::time::Instant::now());
            program_data.preview_fps_counter = 0;
        },
        Some(timestamp) => {
            let fps = program_data.preview_fps_counter as f64 / timestamp.elapsed().as_secs_f64();

            let img_size_str = match program_data.gui.as_ref().unwrap().preview_area.image_size() {
                Some((width, height)) => format!("{}x{}", width, height),
                None => "".to_string()
            };

            let zoom = program_data.gui.as_ref().unwrap().preview_area.get_zoom();
            program_data.gui.as_ref().unwrap().status_bar.preview_fps.set_label(&format!(
                "Preview: {} ({:.1}%)   {:.1} fps",
                img_size_str,
                zoom * 100.0,
                fps
            ));

            program_data.preview_fps_counter = 0;
            program_data.preview_fps_last_timestamp = Some(std::time::Instant::now());
        }
    }
}

fn update_refreshable_camera_controls(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let program_data = program_data_rc.borrow();

    for c_widget in &program_data.gui.as_ref().unwrap().control_widgets {
        if c_widget.1.0.refreshable {
            match &(c_widget.1).1 {
                ControlWidgetBundle::ListControl(ListControlWidgets{ combo, combo_changed_signal }) => {
                    let new_value = match program_data.camera.as_ref().unwrap().get_list_control(*c_widget.0) {
                        Ok(value) => value as u32,
                        Err(e) => {
                            println!("WARNING: Failed to read value of {} (error: {:?}).", &c_widget.1.0.name, e);
                            continue;
                        }
                    };

                    combo.block_signal(&combo_changed_signal);
                    combo.set_active(Some(new_value));
                    combo.unblock_signal(&combo_changed_signal);
                },

                ControlWidgetBundle::NumberControl(
                    NumberControlWidgets{ slider, spin_btn, intervals }
                ) => {
                    let new_value = match program_data.camera.as_ref().unwrap().get_number_control(*c_widget.0) {
                        Ok(value) => value,
                        Err(e) => {
                            println!("WARNING: Failed to read value of {} (error: {:?}).", &c_widget.1.0.name, e);
                            continue;
                        }
                    };

                    if let Some(intervals) = &intervals {
                        intervals.borrow().set_value(new_value);
                        let (new_interval_min, new_interval_max) = intervals.borrow().interval();
                        let slider = slider.borrow();
                        slider.freeze();
                        let adj = slider.adjustment();
                        adj.set_lower(new_interval_min);
                        adj.set_upper(new_interval_max);
                        adj.set_value(new_value);
                        slider.thaw();
                    }

                    if !spin_btn.borrow().has_focus() {
                        spin_btn.borrow().freeze();
                        spin_btn.borrow().set_value(new_value);
                        spin_btn.borrow().thaw();
                    }
                },

                ControlWidgetBundle::BooleanControl(BooleanControlWidgets{ state_checkbox, checkbox_changed_signal }) => {
                    let new_value = match program_data.camera.as_ref().unwrap().get_boolean_control(*c_widget.0) {
                        Ok(value) => value,
                        Err(e) => {
                            println!("WARNING: Failed to read value of {} (error: {:?}).", &c_widget.1.0.name, e);
                            continue;
                        }
                    };

                    state_checkbox.block_signal(&checkbox_changed_signal);
                    state_checkbox.set_active(new_value);
                    state_checkbox.unblock_signal(&checkbox_changed_signal);
                }
            }
        }
    }

    match program_data.camera.as_ref().unwrap().temperature() {
        Some(temp) => program_data.gui.as_ref().unwrap().status_bar.temperature.set_label(&format!("{:.1} °C", temp)),
        None => program_data.gui.as_ref().unwrap().status_bar.temperature.set_label("")
    }
}

fn update_recording_info(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let program_data = program_data_rc.borrow();
    let rec_widgets = &program_data.gui.as_ref().unwrap().rec_widgets;
    let sequence_next_start = rec_widgets.sequence_next_start;
    let (sequence_count, _) = rec_widgets.sequence();
    match sequence_next_start {
        Some(when) => {
            let now = std::time::Instant::now();
            if when > now {
                let total_secs = (when - now).as_secs();
                let hh = total_secs / 3600;
                let mm = (total_secs % 3600) / 60;
                let ss = ((total_secs % 3600) % 60) % 60;
                program_data.gui.as_ref().unwrap().status_bar.current_recording_info.set_label(&format!(
                    "Recording {}/{} starts in {:02}:{:02}:{:02}...",
                    rec_widgets.sequence_idx + 1, sequence_count,  hh, mm, ss
                ));
            }
        },
        _ => ()
    }
}

fn apply_gamma_correction<T>(image: &mut ga_image::Image, max_value: T, gamma: f32, fragment: Rect)
where T: Default + FromPrimitive + AsPrimitive<f32>
{
    let maxvf = AsPrimitive::<f32>::as_(max_value);
    let num_ch = image.pixel_format().num_channels() as i32;

    for y in fragment.y .. fragment.y + fragment.height as i32 {
        let line = image.line_mut::<T>(y as u32);
        for x in num_ch * fragment.x .. num_ch * (fragment.x + fragment.width as i32) {
            line[x as usize] = FromPrimitive::from_f32(
                (AsPrimitive::<f32>::as_(line[x as usize]) / maxvf).powf(gamma) * maxvf
            ).unwrap_or(max_value);
        }
    }
}

fn gamma_correct(image: &mut ga_image::Image, gamma: f32, fragment: Rect) {
    match image.pixel_format() {
        ga_image::PixelFormat::Mono8 |
        ga_image::PixelFormat::RGB8 |
        ga_image::PixelFormat::BGR8 |
        ga_image::PixelFormat::BGRA8 |
        ga_image::PixelFormat::CfaRGGB8 |
        ga_image::PixelFormat::CfaGRBG8 |
        ga_image::PixelFormat::CfaGBRG8 |
        ga_image::PixelFormat::CfaBGGR8 => apply_gamma_correction::<u8>(image, 0xFF, gamma, fragment),

        ga_image::PixelFormat::Mono16 |
        ga_image::PixelFormat::RGB16 |
        ga_image::PixelFormat::RGBA16 |
        ga_image::PixelFormat::CfaRGGB16 |
        ga_image::PixelFormat::CfaGRBG16 |
        ga_image::PixelFormat::CfaGBRG16 |
        ga_image::PixelFormat::CfaBGGR16 => apply_gamma_correction::<u16>(image, 0xFFFF, gamma, fragment),

        ga_image::PixelFormat::Mono32f |
        ga_image::PixelFormat::RGB32f => apply_gamma_correction::<f32>(image, 1.0, gamma, fragment),

        ga_image::PixelFormat::Mono64f |
        ga_image::PixelFormat::RGB64f => apply_gamma_correction::<f64>(image, 1.0, gamma, fragment),

        _ => unimplemented!()
    }
}

fn apply_gain(image: &mut ga_image::Image, gain_factor: f32, fragment: Rect) {
    match image.pixel_format() {
        ga_image::PixelFormat::Mono8 |
        ga_image::PixelFormat::RGB8 |
        ga_image::PixelFormat::BGR8 |
        ga_image::PixelFormat::BGRA8 |
        ga_image::PixelFormat::CfaRGGB8 |
        ga_image::PixelFormat::CfaGRBG8 |
        ga_image::PixelFormat::CfaGBRG8 |
        ga_image::PixelFormat::CfaBGGR8 => apply_gain_impl::<u8>(image, 0xFF, gain_factor, fragment),

        ga_image::PixelFormat::Mono16 |
        ga_image::PixelFormat::RGB16 |
        ga_image::PixelFormat::RGBA16 |
        ga_image::PixelFormat::CfaRGGB16 |
        ga_image::PixelFormat::CfaGRBG16 |
        ga_image::PixelFormat::CfaGBRG16 |
        ga_image::PixelFormat::CfaBGGR16 => apply_gain_impl::<u16>(image, 0xFFFF, gain_factor, fragment),

        ga_image::PixelFormat::Mono32f |
        ga_image::PixelFormat::RGB32f => apply_gain_impl::<f32>(image, 1.0, gain_factor, fragment),

        ga_image::PixelFormat::Mono64f |
        ga_image::PixelFormat::RGB64f => apply_gain_impl::<f64>(image, 1.0, gain_factor, fragment),

        _ => unimplemented!()
    }
}

fn apply_gain_impl<T>(image: &mut ga_image::Image, max_value: T, gain_factor: f32, fragment: Rect)
where T: Default + FromPrimitive + AsPrimitive<f32>
{
    let num_ch = image.pixel_format().num_channels() as i32;

    for y in fragment.y .. fragment.y + fragment.height as i32 {
        let line = image.line_mut::<T>(y as u32);
        for x in num_ch * fragment.x .. num_ch * (fragment.x + fragment.width as i32) {
            let new_value = (AsPrimitive::<f32>::as_(line[x as usize]) * gain_factor).max(0.0);
            line[x as usize] = FromPrimitive::from_f32(new_value).unwrap_or(max_value);
        }
    }
}

pub fn disconnect_camera(program_data_rc: &Rc<RefCell<ProgramData>>, finish_capture_thread: bool) {
    if finish_capture_thread {
        program_data_rc.borrow_mut().finish_capture_thread();
    }
    {
        let pd = program_data_rc.borrow();
        let gui = pd.gui.as_ref().unwrap();
        gui.rec_widgets.on_disconnect();
        gui.action_map.get(actions::TAKE_SNAPSHOT).unwrap().set_enabled(false);
        gui.action_map.get(actions::SET_ROI).unwrap().set_enabled(false);
        gui.stabilization.toggle_button.set_active(false);
    }

    let mut pd = program_data_rc.borrow_mut();
    pd.camera = None;
    if let Some(gui) = pd.gui.as_ref() {
        gui.status_bar.preview_fps.set_label("");
        gui.status_bar.capture_fps.set_label("");
        gui.status_bar.current_recording_info.set_label("");
        for (cam_item, activate_signal) in &gui.camera_menu_items {
            cam_item.set_sensitive(true);
            cam_item.block_signal(&activate_signal);
            cam_item.set_active(false);
            cam_item.unblock_signal(&activate_signal);
        }

        gui.action_map.get(actions::DISCONNECT_CAMERA).unwrap().set_enabled(false);
    }
    camera_gui::remove_camera_controls(&mut pd);

    pd.tracking = None;
    pd.crop_area = None;

    log::info!("disconnected from camera");
}

/// Returns new zoom factor chosen by user or `None` if the dialog was canceled or there was an invalid input.
fn show_custom_zoom_dialog(parent: &gtk::ApplicationWindow, old_value: f64, program_data_rc: &Rc<RefCell<ProgramData>>)
-> Option<f64> {
    let dialog = gtk::Dialog::with_buttons(
        Some("Custom zoom factor (%)"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    dialog.set_default_response(gtk::ResponseType::Accept);

    let _ddestr = DialogDestroyer::new(&dialog);

    let entry = gtk::EntryBuilder::new()
        .input_purpose(gtk::InputPurpose::Number)
        .text(&format!("{:.1}", 100.0 * old_value))
        .activates_default(true)
        .build();

    dialog.content_area().pack_start(&entry, false, true, PADDING);
    dialog.show_all();

    if dialog.run() == gtk::ResponseType::Accept {
        if let Ok(value_percent) = entry.text().parse::<f64>() {
            if value_percent >= 100.0 * MIN_ZOOM && value_percent <= 100.0 * MAX_ZOOM {
                Some(value_percent / 100.0)
            } else {
                show_message(
                    &format!("Specify value from {:.0} to {:.0}.", MIN_ZOOM * 100.0, MAX_ZOOM * 100.0),
                    "Error",
                    gtk::MessageType::Error,
                    program_data_rc
                );
                None
            }
        } else {
            show_message(
                &format!("Invalid value: {}", entry.text()),
                "Error",
                gtk::MessageType::Error,
                program_data_rc
            );
            None
        }
    } else {
        None
    }
}

/// Draws reticle on a context whose (0, 0) is the middle of the captured image.
fn draw_reticle(ctx: &cairo::Context, program_data: &ProgramData) {
    let reticle = &program_data.gui.as_ref().unwrap().reticle;

    if !reticle.enabled { return; }

    ctx.set_dash(&[], 0.0);
    ctx.set_line_width(reticle.line_width);
    ctx.set_antialias(cairo::Antialias::Default);

    ctx.set_source_rgba(1.0, 0.0, 0.0, reticle.opacity);
    let mut radius = 10.0;
    while radius < reticle.diameter {
        ctx.arc(0.0, 0.0, radius, 0.0, 2.0 * std::f64::consts::PI);
        ctx.stroke().unwrap();
        radius += reticle.step;
    }
}
