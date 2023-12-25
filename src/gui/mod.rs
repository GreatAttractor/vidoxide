//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! GUI module.
//!

mod camera_gui;
mod checked_listbox;
#[cfg(feature = "controller")]
mod controller;
mod dec_intervals;
mod dispersion_dialog;
mod freezeable;
mod preview_processing;
mod histogram_utils;
mod histogram_view;
mod img_view;
mod info_overlay;
mod mount_gui;
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
use cgmath::{EuclideanSpace, Point2, Vector2, Zero};
#[cfg(feature = "controller")]
use controller::{ControllerDialog, init_controller_menu};
use crate::{CameraControlChange, NewControlValue, OnCapturePauseAction, ProgramData};
use crate::camera;
use crate::camera::CameraError;
use crate::mount;
use crate::mount::RadPerSec;
use crate::resources;
use crate::workers::capture::{CaptureToMainThreadMsg, MainToCaptureThreadMsg};
use crate::workers::histogram::{Histogram, HistogramRequest, MainToHistogramThreadMsg};
use crate::workers::recording::RecordingToMainThreadMsg;
use dispersion_dialog::DispersionDialog;
use ga_image;
use ga_image::Rect;
use preview_processing::create_preview_processing_dialog;
use glib::clone;
use gtk::cairo;
use gtk::prelude::*;
use histogram_view::HistogramView;
use img_view::ImgView;
use info_overlay::{InfoOverlay, ScreenSelection, draw_info_overlay};
use mount_gui::MountWidgets;
use num_traits::cast::{FromPrimitive, AsPrimitive};
use psf_dialog::PsfDialog;
use rec_gui::RecWidgets;
use reticle_dialog::create_reticle_dialog;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::Ordering;

#[cfg(feature = "controller")]
pub use crate::controller::on_controller_event;
pub use mount_gui::on_mount_error;

/// Control padding in pixels.
const PADDING: u32 = 10;

const MOUSE_BUTTON_LEFT: u32 = 1;
const MOUSE_BUTTON_RIGHT: u32 = 3;

const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 20.0;

const ZOOM_CHANGE_FACTOR: f64 = 1.10;

const DEFAULT_TOOLBAR_ICON_SIZE: i32 = 32;

mod gtk_signals {
    pub const ACTIVATE: &'static str = "activate";
}

mod actions {
    // action group name
    pub const PREFIX: &'static str = "vidoxide";

    // action names to be used for constructing `gio::SimpleAction`
    pub const DISCONNECT_CAMERA: &'static str = "disconnect camera";
    pub const TAKE_SNAPSHOT:     &'static str = "take snapshot";
    pub const SET_ROI:           &'static str = "set roi";

    /// Returns prefixed action name to be used with `ActionableExt::set_action_name`.
    pub fn prefixed(s: &str) -> String {
        format!("{}.{}", PREFIX, s)
    }
}

struct StatusBarFields {
    preview_fps: gtk::Label,
    capture_fps: gtk::Label,
    temperature: gtk::Label,
    current_recording_info: gtk::Label,
    recording_overview: gtk::Label
}

/// Current mode of behavior of the left mouse button for the preview area.
enum MouseMode {
    None,
    SelectROI,
    SelectCentroidArea,
    PlaceTrackingAnchor,
    SelectCropArea,
    SelectHistogramArea
}

impl MouseMode {
    pub fn is_rect_selection(&self) -> bool {
        match self {
            MouseMode::SelectROI
            | MouseMode::SelectCentroidArea
            | MouseMode::SelectCropArea
            | MouseMode::SelectHistogramArea => true,

            MouseMode::None
            | MouseMode::PlaceTrackingAnchor => false
        }
    }
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

pub struct Stabilization {
    toggle_button: gtk::ToggleToolButton,
    position: Point2<i32>
}

pub struct GuiData {
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
    mount_widgets: MountWidgets,
    mouse_mode: MouseMode,
    info_overlay: InfoOverlay,
    default_mouse_mode_button: gtk::RadioToolButton,
    histogram_view: HistogramView,
    // We must store an action map ourselves (and not e.g. reuse `SimpleActionGroup`), because currently (0.14.0) with
    // `gio` one cannot access a group's action in a way allowing to change its enabled state.
    action_map: HashMap<&'static str, gtk::gio::SimpleAction>
}

impl GuiData {
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

fn create_status_bar() -> (gtk::Frame, StatusBarFields) {
    let status_bar_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let preview_fps = gtk::Label::new(None);
    let capture_fps = gtk::Label::new(None);
    let temperature = gtk::Label::new(None);
    let current_recording_info = gtk::LabelBuilder::new().justify(gtk::Justification::Left).build();
    let recording_overview = gtk::LabelBuilder::new().justify(gtk::Justification::Left).build();

    status_bar_box.pack_start(&preview_fps, false, false, PADDING);
    status_bar_box.pack_start(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, PADDING);
    status_bar_box.pack_start(&capture_fps, false, false, PADDING);
    status_bar_box.pack_start(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, PADDING);
    status_bar_box.pack_start(&temperature, false, false, PADDING);
    status_bar_box.pack_start(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, PADDING);
    status_bar_box.pack_start(&current_recording_info, false, false, PADDING);
    status_bar_box.pack_start(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, PADDING);
    status_bar_box.pack_start(&recording_overview, false, false, PADDING);

    let status_bar_frame = gtk::Frame::new(None);
    status_bar_frame.set_shadow_type(gtk::ShadowType::In);
    status_bar_frame.add(&status_bar_box);

    (status_bar_frame, StatusBarFields{ preview_fps, capture_fps, temperature, current_recording_info, recording_overview })
}

fn on_preview_area_button_down(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();
    if program_data.gui.as_ref().unwrap().mouse_mode.is_rect_selection() {
        program_data.gui.as_mut().unwrap().info_overlay.screen_sel =
            Some(ScreenSelection{ start: pos, end: pos });
    }
}

fn on_preview_area_button_up(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let preview_img_size = program_data_rc.borrow().gui.as_ref().unwrap().preview_area.image_size();

    let sel_rect: Option<Rect> = if let Some(ssel) = program_data_rc.borrow().gui.as_ref().unwrap().info_overlay.screen_sel.as_ref() {
        let mut rect = Rect{
            x: ssel.start.x.min(ssel.end.x).max(0),
            y: ssel.start.y.min(ssel.end.y).max(0),
            width: (ssel.start.x - ssel.end.x).abs() as u32,
            height: (ssel.start.y - ssel.end.y).abs() as u32
        };

        if let Some((img_w, img_h)) = preview_img_size {
            if rect.x as u32 + rect.width > img_w as u32 { rect.width = (img_w - rect.x) as u32 }
            if rect.y as u32 + rect.height > img_h as u32 { rect.height = (img_h - rect.y) as u32 }
        }

        Some(rect)
    } else {
        None
    };

    let mut show_crop_error = false;
    let mut send_to_cap_thread_res = Ok(());

    {
        let mut program_data = program_data_rc.borrow_mut();
        program_data.gui.as_mut().unwrap().info_overlay.screen_sel = None;
        if let Some(ref data) = program_data.capture_thread_data {
            if let Some(sel_rect) = sel_rect {
                match program_data.gui.as_ref().unwrap().mouse_mode {
                    MouseMode::SelectCentroidArea =>
                    {
                        send_to_cap_thread_res =
                            data.sender.send(MainToCaptureThreadMsg::EnableCentroidTracking(sel_rect));
                        log::info!("enabled target tracking via centroid");
                    },

                    MouseMode::SelectCropArea => {
                        if !program_data.rec_job_active {
                            send_to_cap_thread_res =
                                data.sender.send(MainToCaptureThreadMsg::EnableRecordingCrop(sel_rect));

                            if send_to_cap_thread_res.is_ok() {
                                program_data.crop_area = Some(sel_rect);
                            }
                        } else {
                            // cannot call `show_message` here, as it would start calling event handlers in a nested
                            // event loop, and we still have an active borrow of `program_data`
                            show_crop_error = true;
                        }
                    },

                    MouseMode::SelectHistogramArea => {
                        program_data.histogram_area = Some(sel_rect);
                    },

                    MouseMode::SelectROI => send_to_cap_thread_res = initiate_set_roi(sel_rect, &mut program_data),

                    MouseMode::None | MouseMode::PlaceTrackingAnchor => ()
                }
            } else {
                match program_data.gui.as_ref().unwrap().mouse_mode {
                    MouseMode::PlaceTrackingAnchor => {
                        send_to_cap_thread_res = data.sender.send(MainToCaptureThreadMsg::EnableAnchorTracking(pos));
                        log::info!("enabled target tracking via anchor");
                    },

                    _ => ()
                }
            }
        };
    }

    {
        // need to clone the button handle first, so that `program_data_rc` is no longer borrowed
        // when button's toggle handler runs due to `set_active` call below
        let btn = program_data_rc.borrow().gui.as_ref().unwrap().default_mouse_mode_button.clone();
        btn.set_active(true);
    }

    if send_to_cap_thread_res.is_err() {
        crate::on_capture_thread_failure(program_data_rc);
    } else if show_crop_error {
        show_message("Cannot set crop area during recording.", "Error", gtk::MessageType::Error);
    }
}

fn on_preview_area_mouse_move(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();
    let gui = program_data.gui.as_mut().unwrap();
    if let Some(screen_sel) = &mut gui.info_overlay.screen_sel {
        screen_sel.end = pos;
        gui.preview_area.refresh();
    }
}

pub fn init_main_window(app: &gtk::Application, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let window = gtk::ApplicationWindow::new(app);
    window.set_title("Vidoxide");

    let action_map = setup_actions(&window, program_data_rc);

    {
        let config = &program_data_rc.borrow().config;

        if let Some(pos) = program_data_rc.borrow().config.main_window_pos() {
            window.move_(pos.x, pos.y);
            window.resize(pos.width, pos.height);
        } else {
            window.resize(800, 600);
        }

        if let Some(is_maximized) = config.main_window_maximized() {
            if is_maximized { window.maximize(); }
        }
    }

    let preview_area = ImgView::new(
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { on_preview_area_button_down(pos, &program_data_rc); })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { on_preview_area_button_up(pos, &program_data_rc); })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { on_preview_area_mouse_move(pos, &program_data_rc); })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |ctx, zoom| {
            draw_info_overlay(ctx, zoom, &mut program_data_rc.borrow_mut());
        })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |ctx| {
            draw_reticle(ctx, &program_data_rc.borrow());
        })),
    );

    let camera_controls_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    camera_controls_box.set_baseline_position(gtk::BaselinePosition::Top);//?

    let camera_controls_scroller = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
    camera_controls_scroller.add(&camera_controls_box);

    let histogram_view = HistogramView::new();

    let cam_controls_and_histogram = gtk::Paned::new(gtk::Orientation::Vertical);
    cam_controls_and_histogram.pack1(&camera_controls_scroller, false, false);
    cam_controls_and_histogram.pack2(histogram_view.top_widget(), true, true);
    if let Some(paned_pos) = program_data_rc.borrow().config.camera_controls_paned_pos() {
        cam_controls_and_histogram.set_position(paned_pos);
    } else {
        cam_controls_and_histogram.set_position(window.size().1 / 2);
    }

    let controls_notebook = gtk::Notebook::new();

    controls_notebook.append_page(&cam_controls_and_histogram, Some(&gtk::Label::new(Some("Camera controls"))));

    let (rec_box, rec_widgets) = rec_gui::create_recording_panel(program_data_rc);
    controls_notebook.append_page(&rec_box, Some(&gtk::Label::new(Some("Recording"))));

    let mount_widgets = mount_gui::create_mount_box(program_data_rc);
    controls_notebook.append_page(mount_widgets.wbox(), Some(&gtk::Label::new(Some("Mount"))));

    let controls_notebook_scroller = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
    controls_notebook_scroller.add(&controls_notebook);

    let (menu_bar, camera_menu, camera_menu_items) = init_menu(&window, program_data_rc);

    let window_contents = gtk::Paned::new(gtk::Orientation::Horizontal);
    window_contents.set_wide_handle(true);
    window_contents.pack1(preview_area.top_widget(), true, true);
    window_contents.pack2(&controls_notebook_scroller, false, false);
    if let Some(paned_pos) = program_data_rc.borrow().config.main_window_paned_pos() {
        window_contents.set_position(paned_pos);
    } else {
        window_contents.set_position(window.size().0 - 400);
    }

    let (status_bar_frame, status_bar) = create_status_bar();

    let top_lvl_v_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top_lvl_v_box.pack_start(&menu_bar, false, false, PADDING);
    let (toolbar, default_mouse_mode_button, stabilization_button) = create_toolbar(&window, &program_data_rc);
    top_lvl_v_box.pack_start(&toolbar, false, false, 0);
    top_lvl_v_box.pack_start(&window_contents, true, true, PADDING);
    top_lvl_v_box.pack_start(&status_bar_frame, false, false, PADDING);

    window.add(&top_lvl_v_box);

    window.show_all();

    window.connect_delete_event(clone!(
        @weak program_data_rc,
        @weak window_contents,
        @weak cam_controls_and_histogram
        => @default-panic, move |wnd, _| {
            on_main_window_delete(
                wnd,
                &window_contents,
                &cam_controls_and_histogram,
                &program_data_rc);
            gtk::Inhibit(false)
        }
    ));

    let rtc_opacity = 1.0;
    let rtc_diameter = 100.0;
    let rtc_step = 10.0;
    let rtc_line_width = 2.0;

    let gui = GuiData{
        controls_box: camera_controls_box,
        status_bar,
        control_widgets: Default::default(),
        camera_menu,
        camera_menu_items,
        preview_area,
        rec_widgets,
        mount_widgets,
        info_overlay: InfoOverlay::new(),
        reticle: Reticle{
            enabled: false,
            dialog: create_reticle_dialog(&window, &program_data_rc, rtc_opacity, rtc_diameter, rtc_step, rtc_line_width),
            diameter: rtc_diameter,
            opacity: rtc_opacity,
            step: rtc_step,
            line_width: rtc_line_width
        },
        stabilization: Stabilization{
            position: Point2::origin(),
            toggle_button: stabilization_button
        },
        preview_processing: PreviewProcessing {
            dialog: create_preview_processing_dialog(&window, &program_data_rc),
            gamma: 1.0,
            gain: Decibel(0.0),
            stretch_histogram: false
        },
        #[cfg(feature = "controller")]
        controller_dialog: ControllerDialog::new(&window, &program_data_rc),
        dispersion_dialog: DispersionDialog::new(&window, &program_data_rc),
        psf_dialog: PsfDialog::new(&window, &program_data_rc),
        mouse_mode: MouseMode::None,
        default_mouse_mode_button,
        histogram_view,
        action_map
    };

    program_data_rc.borrow_mut().gui = Some(gui);
}

fn setup_actions(app_window: &gtk::ApplicationWindow, program_data_rc: &Rc<RefCell<ProgramData>>)
-> HashMap<&'static str, gtk::gio::SimpleAction> {
    let action_group = gtk::gio::SimpleActionGroup::new();
    let mut action_map = HashMap::new();

    // ----------------------------
    let disconnect_action = gtk::gio::SimpleAction::new(actions::DISCONNECT_CAMERA, None);
    disconnect_action.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        disconnect_camera(&program_data_rc, true);
    }));
    disconnect_action.set_enabled(false);
    action_group.add_action(&disconnect_action);
    action_map.insert(actions::DISCONNECT_CAMERA, disconnect_action);

    // ----------------------------
    let snapshot_action = gtk::gio::SimpleAction::new(actions::TAKE_SNAPSHOT, None);
    snapshot_action.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        on_snapshot(&program_data_rc);
    }));
    snapshot_action.set_enabled(false);
    action_group.add_action(&snapshot_action);
    action_map.insert(actions::TAKE_SNAPSHOT, snapshot_action);

    //-----------------------------
    let set_roi_action = gtk::gio::SimpleAction::new(actions::SET_ROI, None);
    set_roi_action.connect_activate(clone!(@weak app_window, @weak program_data_rc => @default-panic, move |_, _| {
        on_set_roi(&app_window, &program_data_rc);
    }));
    set_roi_action.set_enabled(false);
    action_group.add_action(&set_roi_action);
    action_map.insert(actions::SET_ROI, set_roi_action);

    // ----------------------------
    app_window.insert_action_group(actions::PREFIX, Some(&action_group));

    action_map
}

fn initiate_set_roi(rect: Rect, program_data: &mut ProgramData)
-> Result<(), std::sync::mpsc::SendError<crate::workers::capture::MainToCaptureThreadMsg>> {
    let result = program_data.capture_thread_data.as_mut().unwrap().sender.send(
        MainToCaptureThreadMsg::Pause
    );

    if result.is_ok() {
        program_data.on_capture_pause_action = Some(OnCapturePauseAction::SetROI(rect));
    }

    result
}

fn on_set_roi(app_window: &gtk::ApplicationWindow, program_data_rc: &Rc<RefCell<ProgramData>>) {
    if let Some(roi_rect) = roi_dialog::show_roi_dialog(app_window) {
        let result = initiate_set_roi(roi_rect, &mut program_data_rc.borrow_mut());
        if result.is_err() {
            crate::on_capture_thread_failure(program_data_rc);
        }
    }
}

fn on_snapshot(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();
    let gui_data = program_data.gui.as_ref().unwrap();

    if program_data.last_displayed_preview_image.is_none() {
        println!("WARNING: No image captured yet, cannot take a snapshot.");
        return;
    }

    let dest_dir = gui_data.rec_widgets.dest_dir();
    let mut dest_path;
    loop {
        dest_path = Path::new(&dest_dir).join(format!("snapshot_{:04}.tif", program_data.snapshot_counter));
        if !dest_path.exists() {
            break
        }
        program_data.snapshot_counter += 1;
    }

    //TODO: demosaic raw color first
    program_data.last_displayed_preview_image.as_ref().unwrap()
        .view().save(&dest_path.to_str().unwrap().to_string(), ga_image::FileType::Tiff).unwrap();
}

/// Returns (toolbar, default mouse mode button, stabilization button).
fn create_toolbar(
    main_wnd: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> (gtk::Toolbar, gtk::RadioToolButton, gtk::ToggleToolButton) {
    let toolbar = gtk::Toolbar::new();

    let icon_size = if let Some(s) = program_data_rc.borrow().config.toolbar_icon_size() {
        s
    } else {
        program_data_rc.borrow().config.set_toolbar_icon_size(DEFAULT_TOOLBAR_ICON_SIZE);
        DEFAULT_TOOLBAR_ICON_SIZE
    };

    let btn_zoom_in = gtk::ToolButton::new(Some(&resources::load_svg(resources::ToolbarIcon::ZoomIn, icon_size).unwrap()), None);
    btn_zoom_in.set_tooltip_text(Some("Zoom in"));
    btn_zoom_in.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().preview_area.change_zoom(ZOOM_CHANGE_FACTOR);
    }));

    let btn_zoom_out = gtk::ToolButton::new(
        Some(&resources::load_svg(resources::ToolbarIcon::ZoomOut, icon_size).unwrap()),
        None
    );
    btn_zoom_out.set_tooltip_text(Some("Zoom out"));
    btn_zoom_out.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().preview_area.change_zoom(1.0 / ZOOM_CHANGE_FACTOR);
    }));

    let btn_zoom_custom = gtk::ToolButton::new(
        Some(&resources::load_svg(resources::ToolbarIcon::ZoomCustom, icon_size).unwrap()),
        None
    );
    btn_zoom_custom.set_tooltip_text(Some("Custom zoom level"));
    btn_zoom_custom.connect_clicked(clone!(@weak program_data_rc, @weak main_wnd => @default-panic, move |_| {
        let old_zoom = program_data_rc.borrow().gui.as_ref().unwrap().preview_area.get_zoom();
        if let Some(new_zoom) = show_custom_zoom_dialog(&main_wnd, old_zoom) {
            program_data_rc.borrow_mut().gui.as_mut().unwrap().preview_area.set_zoom(new_zoom);
        }
    }));

    let btn_zoom_reset = gtk::ToolButton::new(
        None::<&gtk::Widget>,
        Some("1:1")
    );
    btn_zoom_reset.set_tooltip_text(Some("Reset to 100%"));
    btn_zoom_reset.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().preview_area.set_zoom(1.0);
    }));


    toolbar.insert(&btn_zoom_in, -1);
    toolbar.insert(&btn_zoom_out, -1);
    toolbar.insert(&btn_zoom_custom, -1);
    toolbar.insert(&btn_zoom_reset, -1);

    toolbar.insert(&gtk::SeparatorToolItem::new(), -1);

    let btn_toggle_info_overlay = gtk::ToggleToolButtonBuilder::new()
        .label("i")
        .tooltip_text("Toggle informational overlay")
        .active(true)
        .build();
    btn_toggle_info_overlay.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().info_overlay.enabled = btn.is_active();
        program_data_rc.borrow_mut().gui.as_ref().unwrap().preview_area.refresh();
    }));
    toolbar.insert(&btn_toggle_info_overlay, -1);

    let btn_toggle_reticle = gtk::ToggleToolButtonBuilder::new()
        .label("⊚")
        .tooltip_text("Toggle reticle")
        .active(false)
        .build();
    btn_toggle_reticle.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().reticle.enabled = btn.is_active();
        program_data_rc.borrow_mut().gui.as_ref().unwrap().preview_area.refresh();
    }));
    toolbar.insert(&btn_toggle_reticle, -1);

    let btn_toggle_stabilization = gtk::ToggleToolButtonBuilder::new()
        .label("⚓") // TODO: use some image
        .tooltip_text("Toggle video stabilization")
        .active(false)
        .build();
    btn_toggle_stabilization.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        let tracking_enabled = program_data_rc.borrow().tracking.is_some();
        if btn.is_active() && !tracking_enabled {
            btn.set_active(false);
            show_message("Tracking is not enabled.", "Error", gtk::MessageType::Error);
            return;
        }

        if btn.is_active() {
            let mut pd = program_data_rc.borrow_mut();
            let tracking_pos = pd.tracking.as_ref().unwrap().pos;
            let gui = pd.gui.as_mut().unwrap();
            gui.stabilization.position = tracking_pos;
        }

        log::info!("preview image stabilization {}", if btn.is_active() { "enabled" } else { "disabled" });
    }));
    toolbar.insert(&btn_toggle_stabilization, -1);

    toolbar.insert(&gtk::SeparatorToolItem::new(), -1);

    let btn_mouse_none = gtk::RadioToolButtonBuilder::new()
        .label("⨉")
        .tooltip_text("Mouse mode: none")
        .build();
    btn_mouse_none.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::None; }
    }));
    toolbar.insert(&btn_mouse_none, -1);

    let btn_mouse_roi = gtk::RadioToolButtonBuilder::new()
        .icon_widget(&resources::load_svg(resources::ToolbarIcon::SelectRoi, icon_size).unwrap())
        .tooltip_text("Mouse mode: select ROI")
        .build();
    btn_mouse_roi.join_group(Some(&btn_mouse_none));
    btn_mouse_roi.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::SelectROI; }
    }));
    toolbar.insert(&btn_mouse_roi, -1);

    let btn_mouse_centroid = gtk::RadioToolButtonBuilder::new()
        .label("✹")
        .tooltip_text("Mouse mode: select centroid tracking area")
        .build();
    btn_mouse_centroid.join_group(Some(&btn_mouse_none));
    btn_mouse_centroid.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::SelectCentroidArea; }
    }));
    toolbar.insert(&btn_mouse_centroid, -1);

    let btn_mouse_anchor = gtk::RadioToolButtonBuilder::new()
        .label("✛")
        .tooltip_text("Mouse mode: place tracking anchor")
        .build();
    btn_mouse_anchor.join_group(Some(&btn_mouse_none));
    btn_mouse_anchor.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::PlaceTrackingAnchor; }
    }));
    toolbar.insert(&btn_mouse_anchor, -1);

    let btn_mouse_crop = gtk::RadioToolButtonBuilder::new()
        .label("✂")
        .tooltip_text("Mouse mode: select recording crop area")
        .build();
    btn_mouse_crop.join_group(Some(&btn_mouse_none));
    btn_mouse_crop.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::SelectCropArea; }
    }));
    toolbar.insert(&btn_mouse_crop, -1);

    let btn_mouse_histogram = gtk::RadioToolButtonBuilder::new()
        .label("H")
        .tooltip_text("Mouse mode: select histogram calculation area")
        .build();
    btn_mouse_histogram.join_group(Some(&btn_mouse_none));
    btn_mouse_histogram.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() { program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::SelectHistogramArea; }
    }));
    toolbar.insert(&btn_mouse_histogram, -1);

    toolbar.insert(&gtk::SeparatorToolItem::new(), -1);

    let btn_set_roi = gtk::ToolButtonBuilder::new()
        .label("ROI")
        .tooltip_text("Set ROI by providing its position and size")
        .build();
    btn_set_roi.set_action_name(Some(&actions::prefixed(actions::SET_ROI)));
    toolbar.insert(&btn_set_roi, -1);


    let btn_unset_roi = gtk::ToolButton::new(
        Some(&resources::load_svg(resources::ToolbarIcon::RoiOff, icon_size).unwrap()), None
    );
    btn_unset_roi.set_tooltip_text(Some("Disable ROI"));
    btn_unset_roi.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        let mut cap_send_result = Ok(());

        {
            let mut pd = program_data_rc.borrow_mut();
            if pd.capture_thread_data.is_some() {
                cap_send_result = pd.capture_thread_data.as_mut().unwrap().sender.send(MainToCaptureThreadMsg::Pause);
                if cap_send_result.is_ok() {
                    pd.on_capture_pause_action = Some(OnCapturePauseAction::DisableROI);
                }
            }
        } // end borrow of `program_data_rc`

        if cap_send_result.is_err() {
            crate::on_capture_thread_failure(&program_data_rc);
        }
    }));
    toolbar.insert(&btn_unset_roi, -1);

    (toolbar, btn_mouse_none, btn_toggle_stabilization)
}

fn on_main_window_delete(
    wnd: &gtk::ApplicationWindow,
    main_wnd_contents: &gtk::Paned,
    cam_controls_and_histogram: &gtk::Paned,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    let (x, y) = wnd.position();
    let (width, height) = wnd.size();
    let config = &program_data_rc.borrow().config;
    config.set_main_window_pos(gtk::Rectangle{ x, y, width, height });
    config.set_main_window_maximized(wnd.is_maximized());
    config.set_main_window_paned_pos(main_wnd_contents.position());
    config.set_camera_controls_paned_pos(cam_controls_and_histogram.position());
    //TODO: encode a `Path` somehow;  config.set_recording_dest_path(&program_data_rc.borrow().gui.as_ref().unwrap().rec_widgets.dest_dir());
}

/// WARNING: this recursively enters the main event loop until the message dialog closes; therefore active borrows
/// of `program_data_rc` MUST NOT be held when calling this function.
pub fn show_message(msg: &str, title: &str, msg_type: gtk::MessageType) {
    let dialog = gtk::MessageDialog::new::<gtk::Window>(None, gtk::DialogFlags::MODAL, msg_type, gtk::ButtonsType::Close, msg);
    dialog.set_title(title);
    dialog.set_use_markup(true);
    dialog.run();
    dialog.close();
}

/// Returns (menu bar, camera menu, camera menu items).
fn init_menu(
    window: &gtk::ApplicationWindow,
    program_data: &Rc<RefCell<ProgramData>>
) -> (gtk::MenuBar, gtk::Menu, Vec<(gtk::CheckMenuItem, glib::SignalHandlerId)>) {
    let accel_group = gtk::AccelGroup::new();
    window.add_accel_group(&accel_group);

    let about_item = gtk::MenuItem::with_label("About");
    about_item.connect_activate(move |_| show_about_dialog());

    let quit_item = gtk::MenuItem::with_label("Quit");
    quit_item.connect_activate(clone!(@weak window => @default-panic, move |_| {
        window.close();
    }));
    // `Primary` is `Ctrl` on Windows and Linux, and `command` on macOS
    // It isn't available directly through `gdk::ModifierType`, since it has
    // different values on different platforms.
    let (key, modifier) = gtk::accelerator_parse("<Primary>Q");
    quit_item.add_accelerator(gtk_signals::ACTIVATE, &accel_group, key, modifier, gtk::AccelFlags::VISIBLE);

    let file_menu = gtk::Menu::new();
    file_menu.append(&about_item);
    file_menu.append(&quit_item);

    let file_menu_item = gtk::MenuItem::with_label("File");
    file_menu_item.set_submenu(Some(&file_menu));

    let menu_bar = gtk::MenuBar::new();
    menu_bar.append(&file_menu_item);

    let camera_menu_item = gtk::MenuItem::with_label("Camera");
    let (camera_menu, camera_menu_items) = camera_gui::init_camera_menu(program_data);
    camera_menu_item.set_submenu(Some(&camera_menu));
    menu_bar.append(&camera_menu_item);

    let mount_menu_item = gtk::MenuItem::with_label("Mount");
    mount_menu_item.set_submenu(Some(&mount_gui::init_mount_menu(program_data, window)));
    menu_bar.append(&mount_menu_item);

    let preview_menu_item = gtk::MenuItem::with_label("Preview");
    preview_menu_item.set_submenu(Some(&init_preview_menu(program_data, &accel_group)));
    menu_bar.append(&preview_menu_item);

    #[cfg(feature = "controller")]
    {
        let controller_menu_item = gtk::MenuItem::with_label("Controller");
        controller_menu_item.set_submenu(Some(&init_controller_menu(program_data)));
        menu_bar.append(&controller_menu_item);
    }

    (menu_bar, camera_menu, camera_menu_items)
}

fn init_preview_menu(
    program_data_rc: &Rc<RefCell<ProgramData>>,
    accel_group: &gtk::AccelGroup
) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let snapshot = gtk::MenuItem::with_label("Take snapshot");
    snapshot.set_action_name(Some(&actions::prefixed(actions::TAKE_SNAPSHOT)));
    let (key, modifier) = gtk::accelerator_parse("F12");
    snapshot.add_accelerator(gtk_signals::ACTIVATE, accel_group, key, modifier, gtk::AccelFlags::VISIBLE);
    menu.append(&snapshot);

    let demosaic_raw_color = gtk::CheckMenuItem::with_label("Demosaic raw color");
    demosaic_raw_color.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().demosaic_preview ^= true;
    }));
    menu.append(&demosaic_raw_color);

    let disable_histogram_area = gtk::MenuItem::with_label("Disable histogram area");
    disable_histogram_area.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().histogram_area = None;
    }));
    menu.append(&disable_histogram_area);

    let reticle_settings = gtk::MenuItem::with_label("Reticle settings...");
    reticle_settings.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow().gui.as_ref().unwrap().reticle.dialog.show();
    }));
    menu.append(&reticle_settings);

    let preview_processing = gtk::MenuItem::with_label("Processing...");
    preview_processing.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow().gui.as_ref().unwrap().preview_processing.dialog.show();
    }));
    menu.append(&preview_processing);

    let dispersion = gtk::MenuItem::with_label("Atmospheric dispersion indicator...");
    dispersion.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow().gui.as_ref().unwrap().dispersion_dialog.show();
    }));
    menu.append(&dispersion);

    let psf = gtk::MenuItem::with_label("Collimation assistant...");
    psf.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow().gui.as_ref().unwrap().psf_dialog.show();
    }));
    menu.append(&psf);

    menu
}

fn show_about_dialog() {
    show_message(
        &format!(
            "<big><big><b>Vidoxide</b></big></big>\n\n\
            Copyright © 2020-2023 Filip Szczerek (ga.software@yahoo.com)\n\n\
            This project is licensed under the terms of the MIT license (see the LICENSE file for details).\n\n\
            version: {}\n\
            OS: {}",
            crate::VERSION_STRING,
            os_info::get()
        ),
        "About Vidoxide",
        gtk::MessageType::Info
    );
}

pub fn on_recording_thread_message(
    msg: RecordingToMainThreadMsg,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    match msg {
        RecordingToMainThreadMsg::Info(msg_str) => {
            program_data_rc.borrow().gui.as_ref().unwrap().status_bar.recording_overview.set_label(
                &format!("{}", msg_str)
            )
        },

        RecordingToMainThreadMsg::CaptureThreadEnded => {
            rec_gui::on_stop_recording(program_data_rc);
            crate::on_capture_thread_failure(program_data_rc);
        },

        RecordingToMainThreadMsg::Error(err) => {
            rec_gui::on_stop_recording(program_data_rc);
            show_message(&format!("Error during recording:\n{}", err), "Recording error", gtk::MessageType::Error);
        }
    }
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

/// Called ca. once per second to update the status bar and refresh any readable camera controls.
pub fn on_timer(program_data_rc: &Rc<RefCell<ProgramData>>) {
    if !program_data_rc.borrow().camera.is_some() { return; }

    update_preview_info(program_data_rc);
    update_refreshable_camera_controls(program_data_rc);
    update_recording_info(program_data_rc);
}

fn on_tracking_ended(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut reenable_calibration = false;
    {
        let mut pd = program_data_rc.borrow_mut();
        if pd.mount_data.calibration_in_progress() {
            pd.mount_data.calibration_timer.stop();
            pd.mount_data.calibration = None;
            reenable_calibration = true;
        }
        pd.mount_data.guiding_timer.stop();
        pd.mount_data.guiding_pos = None;
        pd.tracking = None;
    }

    let sd_on = program_data_rc.borrow().mount_data.sky_tracking_on;
    let has_mount = program_data_rc.borrow().mount_data.mount.is_some();

    if has_mount {
        program_data_rc.borrow_mut().mount_data.guide_slewing = false;

        //TODO: stop only if a guiding or calibration slew is in progress, not one started by user via an arrow button

        let mut error;
        loop { // no actual loop, just for early exit
            let mut pd = program_data_rc.borrow_mut();
            let mount = pd.mount_data.mount.as_mut().unwrap();

            error = mount.guide(RadPerSec(0.0), RadPerSec(0.0));
            if error.is_err() { break; }

            error = mount.slew(mount::Axis::Primary, mount::SlewSpeed::zero());
            if error.is_err() { break; }

            error = mount.slew(mount::Axis::Secondary, mount::SlewSpeed::zero());

            break;
        }

        if let Err(e) = &error {
            mount_gui::on_mount_error(e);
        }
    }

    let pd = program_data_rc.borrow();
    let gui = pd.gui.as_ref().unwrap();
    gui.mount_widgets.on_target_tracking_ended(reenable_calibration);
    gui.stabilization.toggle_button.set_active(false);

    log::info!("target tracking disabled");
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

fn on_preview_image_ready(
    program_data_rc: &Rc<RefCell<ProgramData>>,
    img: std::sync::Arc<ga_image::Image>,
    tracking_pos: Option<Point2<i32>>
) {
    let mut program_data = program_data_rc.borrow_mut();

    let now = std::time::Instant::now();
    if let Some(fps_limit) = program_data.preview_fps_limit {
        if let Some(last_preview_ts) = program_data.last_displayed_preview_image_timestamp {
            if (now - last_preview_ts).as_secs_f64() < 1.0 / fps_limit as f64 {
                return;
            }
        }
    }
    program_data.last_displayed_preview_image_timestamp = Some(now);

    if let Some(area) = program_data.histogram_area {
        if !img.img_rect().contains_rect(&area) {
            println!("WARNING: histogram calculation area outside image boundaries; disabling.");
            program_data.histogram_area = None;
        }
    }

    let helpers_update_area: Option<Rect> = match &program_data.tracking {
        Some(tracking) => match tracking.mode {
            crate::TrackingMode::Centroid(centroid_area) => Some(centroid_area),
            _ => None
        },
        _ => None
    };
    let dispersion_img_view = ga_image::ImageView::new(&*img, helpers_update_area);

    program_data.gui.as_mut().unwrap().dispersion_dialog.update(&dispersion_img_view);

    program_data.gui.as_mut().unwrap().psf_dialog.update(&*img, helpers_update_area);

    let preview_processing = program_data.gui.as_ref().unwrap().preview_processing.clone();

    let mut displayed_img = std::sync::Arc::clone(&img);

    let mut processed_img: Option<ga_image::Image> = if preview_processing.is_effective() {
        Some((*displayed_img).clone())
    } else {
        None
    };

    if preview_processing.gain.0 != 0.0 {
        let gf = preview_processing.gain.get_gain_factor();
        apply_gain(processed_img.as_mut().unwrap(), gf, program_data.histogram_area.unwrap_or(img.img_rect()));
    }

    if preview_processing.gamma != 1.0 {
        gamma_correct(
            processed_img.as_mut().unwrap(),
            preview_processing.gamma,
            program_data.histogram_area.unwrap_or(img.img_rect())
        );
    }

    if preview_processing.stretch_histogram {
        processed_img =
            Some(histogram_utils::stretch_histogram(processed_img.as_ref().unwrap(), &program_data.histogram_area));
    }

    if let Some(processed_img) = processed_img {
        displayed_img = std::sync::Arc::new(processed_img);
    }

    let stabilization_offset = if program_data.gui.as_ref().unwrap().stabilization.toggle_button.is_active() {
        if let Some(t_pos) = &tracking_pos {
            t_pos - program_data.gui.as_ref().unwrap().stabilization.position
        } else {
            // tracking has been disabled, `on_tracking_ended` will be called shortly
            Vector2::zero()
        }
    } else {
        Vector2::zero()
    };

    let img_bgra24 = displayed_img.convert_pix_fmt(
        ga_image::PixelFormat::BGRA8,
        if program_data.demosaic_preview { Some(ga_image::DemosaicMethod::Simple) } else { None }
    );

    let stride = img_bgra24.bytes_per_line() as i32;
    program_data.gui.as_ref().unwrap().preview_area.set_image(
        cairo::ImageSurface::create_for_data(
            img_bgra24.take_pixel_data(),
            cairo::Format::Rgb24, // actually means: BGRA
            img.width() as i32,
            img.height() as i32,
            stride
        ).unwrap(),
        stabilization_offset
    );
    program_data.gui.as_ref().unwrap().preview_area.refresh();

    const HISTOGRAM_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
    if program_data.t_last_histogram.is_none() ||
       program_data.t_last_histogram.as_ref().unwrap().elapsed() >= HISTOGRAM_UPDATE_INTERVAL {

        program_data.histogram_sender.send(MainToHistogramThreadMsg::CalculateHistogram(HistogramRequest{
            image: (*img).clone(),
            fragment: program_data.histogram_area.clone()
        })).unwrap();

        program_data.t_last_histogram = Some(std::time::Instant::now());
    }

    program_data.last_displayed_preview_image = Some((*img).clone());

    program_data.preview_fps_counter += 1;
}

fn on_capture_paused(
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    let mut show_error: Option<CameraError> = None;
    let action = program_data_rc.borrow().on_capture_pause_action;
    match action {
        Some(action) => match action {
            OnCapturePauseAction::ControlChange(CameraControlChange{ id, value }) => {
                let res = match value {
                    NewControlValue::ListOptionIndex(option_idx) =>
                        program_data_rc.borrow_mut().camera.as_mut().unwrap().set_list_control(id, option_idx),

                    NewControlValue::Boolean(state) =>
                        program_data_rc.borrow_mut().camera.as_mut().unwrap().set_boolean_control(id, state),

                    NewControlValue::Numerical(num_val) =>
                        program_data_rc.borrow_mut().camera.as_mut().unwrap().set_number_control(id, num_val),
                };

                if let Err(e) = res {
                    show_message(
                        &format!("Failed to set camera control:\n{:?}", e),
                        "Error",
                        gtk::MessageType::Error
                    );
                } else {
                    camera_gui::schedule_refresh(program_data_rc);
                }
            },

            OnCapturePauseAction::SetROI(rect) => {
                let result = program_data_rc.borrow_mut().camera.as_mut().unwrap().set_roi(
                    rect.x as u32,
                    rect.y as u32,
                    rect.width,
                    rect.height
                );
                match result {
                    Err(err) => show_error = Some(err),
                    _ => camera_gui::schedule_refresh(program_data_rc)
                }
            },

            OnCapturePauseAction::DisableROI => {
                program_data_rc.borrow_mut().camera.as_mut().unwrap().unset_roi().unwrap();
                camera_gui::schedule_refresh(program_data_rc);
            }
        },
        _ => ()
    }

    if program_data_rc.borrow_mut().capture_thread_data.as_mut().unwrap().sender.send(
        MainToCaptureThreadMsg::Resume
    ).is_err() {
        crate::on_capture_thread_failure(program_data_rc);
    }

    if let Some(error) = show_error {
        show_message(&format!("Failed to set ROI:\n{:?}", error), "Error", gtk::MessageType::Error);
    }
}

fn on_capture_thread_message(
    msg: CaptureToMainThreadMsg,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    let mut received_preview_image = false;

    loop { match msg {
        CaptureToMainThreadMsg::PreviewImageReady((img, tracking_pos)) => {
            received_preview_image = true;
            on_preview_image_ready(program_data_rc, img, tracking_pos);
        },

        CaptureToMainThreadMsg::TrackingUpdate((tracking, crop_area)) => if program_data_rc.borrow().capture_thread_data.is_some() {
            program_data_rc.borrow_mut().tracking = Some(tracking);
            program_data_rc.borrow_mut().crop_area = crop_area;
        },

        CaptureToMainThreadMsg::TrackingFailed => on_tracking_ended(program_data_rc),

        CaptureToMainThreadMsg::Paused => on_capture_paused(program_data_rc),

        CaptureToMainThreadMsg::CaptureError(error) => {
            //TODO: show a message box
            println!("Capture error: {:?}", error);
            let _ = program_data_rc.borrow_mut().capture_thread_data.take().unwrap().join_handle.take().unwrap().join();
            disconnect_camera(&program_data_rc, false);
        },

        CaptureToMainThreadMsg::RecordingFinished => rec_gui::on_recording_finished(&program_data_rc),

        CaptureToMainThreadMsg::Info(info) => {
            let pd = program_data_rc.borrow();
            let status_bar = &pd.gui.as_ref().unwrap().status_bar;

            status_bar.capture_fps.set_label(&format!("Capture: {:.1} fps", info.capture_fps));

            if let Some(msg) = info.recording_info {
                status_bar.current_recording_info.set_label(&msg);
            }
        }
    } break; }

    if let Some(ref mut capture_thread_data) = program_data_rc.borrow_mut().capture_thread_data {
        if received_preview_image  {
            // doing it here, to make sure the `Arc` received in `PreviewImageReady` is already released
            capture_thread_data.new_preview_wanted.store(true, Ordering::Relaxed);
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

pub fn on_histogram_thread_message(
    msg: Histogram,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    program_data_rc.borrow_mut().gui.as_mut().unwrap().histogram_view.set_histogram(msg);
}

/// Returns new zoom factor chosen by user or `None` if the dialog was canceled or there was an invalid input.
fn show_custom_zoom_dialog(parent: &gtk::ApplicationWindow, old_value: f64) -> Option<f64> {
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
                    gtk::MessageType::Error
                );
                None
            }
        } else {
            show_message(&format!("Invalid value: {}", entry.text()), "Error", gtk::MessageType::Error);
            None
        }
    } else {
        None
    }
}

/// Draws reticle on a context whose (0, 0) is the middle of the visible part of the preview area.
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
