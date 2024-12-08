//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! GUI initialization.
//!

use cgmath::{EuclideanSpace, Point2};
use crate::{
    gui::{
        actions,
        camera_gui,
        create_preview_processing_dialog,
        create_reticle_dialog,
        Decibel,
        disconnect_camera,
        DispersionDialog,
        draw_info_overlay,
        draw_reticle,
        event_handlers,
        focuser_gui,
        GuiData,
        HistogramView,
        img_view::ImgView,
        InfoOverlay,
        mount_gui,
        MouseMode,
        PADDING,
        PreviewProcessing,
        PsfDialog,
        rec_gui,
        Reticle,
        show_about_dialog,
        show_custom_zoom_dialog,
        show_message,
        Stabilization,
        StatusBarFields,
        ZOOM_CHANGE_FACTOR,
    },
    MainToCaptureThreadMsg,
    OnCapturePauseAction,
    ProgramData,
    resources,
};
#[cfg(feature = "controller")]
use crate::gui::{ControllerDialog, controller::init_controller_menu};
use glib::clone;
use gtk::prelude::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const DEFAULT_TOOLBAR_ICON_SIZE: i32 = 32;

mod gtk_signals {
    pub const ACTIVATE: &'static str = "activate";
}

/// Returns (menu bar, camera menu, camera menu items).
fn init_menu(
    window: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> (gtk::MenuBar, gtk::Menu, Vec<(gtk::CheckMenuItem, glib::SignalHandlerId)>) {
    let accel_group = gtk::AccelGroup::new();
    window.add_accel_group(&accel_group);

    let about_item = gtk::MenuItem::with_label("About");
    about_item.connect_activate(
        clone!(@weak program_data_rc => @default-panic, move |_| show_about_dialog(&program_data_rc))
    );

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
    let (camera_menu, camera_menu_items) = camera_gui::init_camera_menu(program_data_rc);
    camera_menu_item.set_submenu(Some(&camera_menu));
    menu_bar.append(&camera_menu_item);

    let devices_menu_item = gtk::MenuItem::with_label("Devices");
    devices_menu_item.set_submenu(Some(&init_devices_menu(program_data_rc)));
    menu_bar.append(&devices_menu_item);

    let preview_menu_item = gtk::MenuItem::with_label("Preview");
    preview_menu_item.set_submenu(Some(&init_preview_menu(program_data_rc, &accel_group)));
    menu_bar.append(&preview_menu_item);

    #[cfg(feature = "controller")]
    {
        let controller_menu_item = gtk::MenuItem::with_label("Controller");
        controller_menu_item.set_submenu(Some(&init_controller_menu(program_data_rc)));
        menu_bar.append(&controller_menu_item);
    }

    (menu_bar, camera_menu, camera_menu_items)
}

fn init_devices_menu(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let mount_menu_item = gtk::MenuItem::with_label("Mount");
    mount_menu_item.set_submenu(Some(&mount_gui::init_mount_menu(program_data_rc)));
    menu.append(&mount_menu_item);

    let focuser_menu_item = gtk::MenuItem::with_label("Focuser");
    focuser_menu_item.set_submenu(Some(&focuser_gui::init_focuser_menu(program_data_rc)));
    menu.append(&focuser_menu_item);

    menu
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

    let undock = gtk::MenuItem::with_label("Undock preview area");
    undock.set_action_name(Some(&actions::prefixed(actions::UNDOCK_PREVIEW)));
    menu.append(&undock);

    menu
}

/// Returns "default mouse mode" button.
fn create_mouse_mode_tb_buttons(
    toolbar: &gtk::Toolbar,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    icon_size: i32
) -> gtk::RadioToolButton {
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

    let btn_mouse_measure = gtk::RadioToolButtonBuilder::new()
        .label("📏")
        .tooltip_text("Mouse mode: measure distance")
        .build();
    btn_mouse_measure.join_group(Some(&btn_mouse_none));
    btn_mouse_measure.connect_toggled(clone!(@weak program_data_rc => @default-panic, move |btn| {
        if btn.is_active() {
            program_data_rc.borrow_mut().gui.as_mut().unwrap().mouse_mode = MouseMode::MeasureDistance;
        }
    }));
    toolbar.insert(&btn_mouse_measure, -1);

    btn_mouse_none
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

fn set_up_actions(app_window: &gtk::ApplicationWindow, program_data_rc: &Rc<RefCell<ProgramData>>)
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
        event_handlers::on_snapshot(&program_data_rc);
    }));
    snapshot_action.set_enabled(false);
    action_group.add_action(&snapshot_action);
    action_map.insert(actions::TAKE_SNAPSHOT, snapshot_action);

    //-----------------------------
    let set_roi_action = gtk::gio::SimpleAction::new(actions::SET_ROI, None);
    set_roi_action.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_, _| {
        event_handlers::on_set_roi(&program_data_rc);
    }));
    set_roi_action.set_enabled(false);
    action_group.add_action(&set_roi_action);
    action_map.insert(actions::SET_ROI, set_roi_action);

    // ----------------------------
    let undock_preview_action = gtk::gio::SimpleAction::new(actions::UNDOCK_PREVIEW, None);
    undock_preview_action.set_enabled(true);
    undock_preview_action.connect_activate(clone!(@weak program_data_rc => @default-panic, move |action, _| {
        event_handlers::on_undock_preview_area(&program_data_rc);
        action.set_enabled(false);
    }));
    action_group.add_action(&undock_preview_action);
    action_map.insert(actions::UNDOCK_PREVIEW, undock_preview_action);

    // ----------------------------
    app_window.insert_action_group(actions::PREFIX, Some(&action_group));

    action_map
}

fn create_preview_tb_buttons(
    toolbar: &gtk::Toolbar,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    main_wnd: &gtk::ApplicationWindow,
    icon_size: i32
) {
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
        if let Some(new_zoom) = show_custom_zoom_dialog(&main_wnd, old_zoom, &program_data_rc) {
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

    create_preview_tb_buttons(&toolbar, program_data_rc, main_wnd, icon_size);

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
            show_message("Tracking is not enabled.", "Error", gtk::MessageType::Error, &program_data_rc);
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

    let btn_mouse_none = create_mouse_mode_tb_buttons(&toolbar, program_data_rc,  icon_size);

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

    let btn_undock_preview_area = gtk::ToolButtonBuilder::new()
        .label("⮹") // TODO create an icon
        .tooltip_text("Undock preview area")
        .build();
    btn_undock_preview_area.set_action_name(Some(&actions::prefixed(actions::UNDOCK_PREVIEW)));
    toolbar.insert(&btn_undock_preview_area, -1);

    (toolbar, btn_mouse_none, btn_toggle_stabilization)
}

pub fn init_main_window(app: &gtk::Application, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let app_window = gtk::ApplicationWindow::new(app);
    app_window.set_title("Vidoxide");

    let action_map = set_up_actions(&app_window, program_data_rc);

    {
        let config = &program_data_rc.borrow().config;

        if let Some(pos) = program_data_rc.borrow().config.main_window_pos() {
            app_window.move_(pos.x, pos.y);
            app_window.resize(pos.width, pos.height);
        } else {
            app_window.resize(800, 600);
        }

        if let Some(is_maximized) = config.main_window_maximized() {
            if is_maximized { app_window.maximize(); }
        }
    }

    let preview_area = ImgView::new(
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { event_handlers::on_preview_area_button_down(pos, &program_data_rc); })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { event_handlers::on_preview_area_button_up(pos, &program_data_rc); })),
        Box::new(clone!(@weak program_data_rc => @default-panic, move |pos| { event_handlers::on_preview_area_mouse_move(pos, &program_data_rc); })),
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
        cam_controls_and_histogram.set_position(app_window.size().1 / 2);
    }

    let controls_notebook = gtk::Notebook::new();

    controls_notebook.append_page(&cam_controls_and_histogram, Some(&gtk::Label::new(Some("Camera controls"))));

    let (rec_box, rec_widgets) = rec_gui::create_recording_panel(program_data_rc);
    controls_notebook.append_page(&rec_box, Some(&gtk::Label::new(Some("Recording"))));

    let mount_widgets = mount_gui::create_mount_box(program_data_rc);
    controls_notebook.append_page(mount_widgets.wbox(), Some(&gtk::Label::new(Some("Mount"))));

    let focuser_widgets = focuser_gui::create_focuser_box(program_data_rc);
    controls_notebook.append_page(focuser_widgets.wbox(), Some(&gtk::Label::new(Some("Focuser"))));

    let controls_notebook_scroller = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
    controls_notebook_scroller.add(&controls_notebook);

    let (menu_bar, camera_menu, camera_menu_items) = init_menu(&app_window, program_data_rc);

    let window_contents = gtk::Paned::new(gtk::Orientation::Horizontal);
    window_contents.set_wide_handle(true);
    window_contents.pack1(preview_area.top_widget(), true, true);
    window_contents.pack2(&controls_notebook_scroller, false, false);

    if let Some(paned_pos) = program_data_rc.borrow().config.main_window_paned_pos() {
        window_contents.set_position(paned_pos);
    } else {
        window_contents.set_position(app_window.size().0 - 400);
    }

    let (status_bar_frame, status_bar) = create_status_bar();

    let top_lvl_v_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top_lvl_v_box.pack_start(&menu_bar, false, false, PADDING);
    let (toolbar, default_mouse_mode_button, stabilization_button) = create_toolbar(&app_window, &program_data_rc);
    top_lvl_v_box.pack_start(&toolbar, false, false, 0);
    top_lvl_v_box.pack_start(&window_contents, true, true, PADDING);
    top_lvl_v_box.pack_start(&status_bar_frame, false, false, PADDING);

    app_window.add(&top_lvl_v_box);

    app_window.show_all();

    app_window.connect_delete_event(clone!(
        @weak program_data_rc,
        @weak window_contents,
        @weak cam_controls_and_histogram
        => @default-panic, move |wnd, _| {
            event_handlers::on_main_window_delete(
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
        app_window: app_window.clone(),
        controls_box: camera_controls_box,
        status_bar,
        control_widgets: Default::default(),
        camera_menu,
        camera_menu_items,
        preview_area,
        rec_widgets,
        focuser_widgets,
        mount_widgets,
        info_overlay: InfoOverlay::new(),
        reticle: Reticle{
            enabled: false,
            dialog: create_reticle_dialog(&app_window, &program_data_rc, rtc_opacity, rtc_diameter, rtc_step, rtc_line_width),
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
            dialog: create_preview_processing_dialog(&app_window, &program_data_rc),
            gamma: 1.0,
            gain: Decibel(0.0),
            stretch_histogram: false
        },
        #[cfg(feature = "controller")]
        controller_dialog: ControllerDialog::new(&app_window, &program_data_rc),
        dispersion_dialog: DispersionDialog::new(&app_window, &program_data_rc),
        psf_dialog: PsfDialog::new(&app_window, &program_data_rc),
        mouse_mode: MouseMode::None,
        default_mouse_mode_button,
        histogram_view,
        action_map,
        window_contents
    };

    program_data_rc.borrow_mut().gui = Some(gui);
}
