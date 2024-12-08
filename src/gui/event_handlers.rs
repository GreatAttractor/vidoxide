//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Event handlers.
//!

use cgmath::{Point2, Vector2, Zero};
use crate::{
    CameraControlChange,
    gui::{
        actions,
        apply_gain,
        camera_gui,
        CameraError,
        disconnect_camera,
        gamma_correct,
        histogram_utils,
        mount_gui,
        MouseMode,
        rec_gui,
        roi_dialog,
        ScreenSelection,
        show_message,
        update_preview_info,
        update_recording_info,
        update_refreshable_camera_controls,
    },
    MainToCaptureThreadMsg,
    MainToHistogramThreadMsg,
    NewControlValue,
    mount,
    OnCapturePauseAction,
    ProgramData,
    RadPerSec,
    workers::{
        capture::CaptureToMainThreadMsg,
        histogram::{Histogram, HistogramRequest},
        recording::RecordingToMainThreadMsg,
    }
};
use ga_image::Rect;
use glib::clone;
use gtk::{cairo, prelude::*};
use std::{cell::RefCell, sync::atomic::Ordering, path::Path, rc::Rc};

pub fn on_preview_area_button_down(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();
    if program_data.gui.as_ref().unwrap().mouse_mode.is_selection() {
        program_data.gui.as_mut().unwrap().info_overlay.screen_sel =
            Some(ScreenSelection{ start: pos, end: pos });
    }
}

pub fn on_preview_area_button_up(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
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

                    MouseMode::MeasureDistance => (),

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
        show_message("Cannot set crop area during recording.", "Error", gtk::MessageType::Error, program_data_rc);
    }
}

pub fn on_preview_area_mouse_move(pos: Point2<i32>, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();
    let gui = program_data.gui.as_mut().unwrap();
    if let Some(screen_sel) = &mut gui.info_overlay.screen_sel {
        screen_sel.end = pos;
        gui.preview_area.refresh();
    }
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

pub fn on_set_roi(program_data_rc: &Rc<RefCell<ProgramData>>) {
    if let Some(roi_rect) = roi_dialog::show_roi_dialog(program_data_rc) {
        let result = initiate_set_roi(roi_rect, &mut program_data_rc.borrow_mut());
        if result.is_err() {
            crate::on_capture_thread_failure(program_data_rc);
        }
    }
}

pub fn on_snapshot(program_data_rc: &Rc<RefCell<ProgramData>>) {
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

pub fn on_undock_preview_area(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let preview_wnd = gtk::WindowBuilder::new()
        .type_(gtk::WindowType::Toplevel)
        .title("Vidoxide - preview")
        .build();

    let pd = program_data_rc.borrow();
    let gui = pd.gui.as_ref().unwrap();

    let pw = gui.preview_area.top_widget();
    gui.window_contents.remove(pw);
    preview_wnd.add(pw);
    preview_wnd.show_all();

    preview_wnd.connect_delete_event(clone!(
        @weak program_data_rc
        => @default-panic, move |wnd, _| {
            let pd = program_data_rc.borrow();
            let gui = pd.gui.as_ref().unwrap();
            let pw = gui.preview_area.top_widget();
            wnd.remove(pw);
            gui.window_contents.pack1(pw, true, true);
            gui.action_map.get(actions::UNDOCK_PREVIEW).unwrap().set_enabled(true);
            gtk::Inhibit(false)
        }
    ));
}

pub fn on_main_window_delete(
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
            show_message(
                &format!("Error during recording:\n{}", err),
                "Recording error",
                gtk::MessageType::Error,
                program_data_rc
            );
        }
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
        'block: {
            let mut pd = program_data_rc.borrow_mut();
            let mount = pd.mount_data.mount.as_mut().unwrap();

            error = mount.guide(RadPerSec(0.0), RadPerSec(0.0));
            if error.is_err() { break 'block; }

            error = mount.slew(mount::Axis::Primary, mount::SlewSpeed::zero());
            if error.is_err() { break 'block; }

            error = mount.slew(mount::Axis::Secondary, mount::SlewSpeed::zero());
        }

        if let Err(e) = &error {
            mount_gui::on_mount_error(e, program_data_rc);
        }
    }

    let pd = program_data_rc.borrow();
    let gui = pd.gui.as_ref().unwrap();
    gui.mount_widgets.on_target_tracking_ended(reenable_calibration);
    gui.stabilization.toggle_button.set_active(false);

    log::info!("target tracking disabled");
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
                        gtk::MessageType::Error,
                        program_data_rc
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
        show_message(
            &format!("Failed to set ROI:\n{:?}", error),
            "Error",
            gtk::MessageType::Error,
            program_data_rc
        );
    }
}

pub fn on_capture_thread_message(
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

pub fn on_histogram_thread_message(
    msg: Histogram,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    program_data_rc.borrow_mut().gui.as_mut().unwrap().histogram_view.set_histogram(msg);
}
