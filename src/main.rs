//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Entry point and main data structures of the `vidoxide` executable.
//!

mod args;
mod camera;
mod config;
#[cfg(feature = "controller")]
mod controller;
mod devices;
mod gui;
mod guiding;
mod input;
mod mount;
mod output;
mod resources;
mod timer;
mod tracking;
mod workers;

use camera::drivers;
use cgmath::{Point2, Vector2};
use config::Configuration;
#[cfg(feature = "controller")]
use controller::{TargetAction, SourceAction};
use crossbeam;
use ga_image::Rect;
use gtk::gio::prelude::*;
use glib::clone;
use mount::RadPerSec;
use std::{cell::RefCell, collections::HashMap, sync::{atomic::{AtomicBool, AtomicIsize}, Arc}, rc::Rc};
use timer::Timer;
use workers::capture::MainToCaptureThreadMsg;
use workers::histogram::MainToHistogramThreadMsg;
use workers::recording::{MainToRecordingThreadMsg, Job};

pub const VERSION_STRING: &'static str = include_str!(concat!(env!("OUT_DIR"), "/version"));

pub struct CaptureThreadData {
    pub join_handle: Option<std::thread::JoinHandle<()>>,
    pub sender: std::sync::mpsc::Sender<MainToCaptureThreadMsg>,
    pub new_preview_wanted: Arc<AtomicBool>
}

pub struct RecordingThreadData {
    pub jobs: Arc<crossbeam::queue::SegQueue<Job>>,
    pub join_handle: Option<std::thread::JoinHandle<()>>,
    pub sender: crossbeam::channel::Sender<MainToRecordingThreadMsg>,
    /// Approximate amount of image data currently buffered for recording.
    ///
    /// Increased after each captured frame, but decreased at a lower frequency. May be negative at times.
    pub buffered_kib: Arc<AtomicIsize>
}

#[derive(Copy, Clone)]
pub enum NewControlValue {
    ListOptionIndex(usize),
    Numerical(f64),
    Boolean(bool)
}

/// Describes a camera control change to be performed after the capture thread confirms it is paused.
#[derive(Copy, Clone)]
pub struct CameraControlChange {
    id: camera::CameraControlId,
    value: NewControlValue
}

pub struct MountCalibration {
    origin: Point2<i32>,
    /// Image-space unit vector corresponding to positive slew around primary axis.
    primary_dir: Option<Vector2<f64>>,
    /// Image-space unit vector corresponding to positive slew around secondary axis.
    secondary_dir: Option<Vector2<f64>>,
    /// Image-space-to-mount-axes-space slewing dir transformation matrix.
    img_to_mount_axes: Option<cgmath::Matrix2<f64>>,
    calibration_slew_speed: RadPerSec
}

pub struct MountData {
    mount: Option<Box<dyn mount::Mount>>,
    sky_tracking_on: bool,
    /// Desired tracking position. If `Some`, guiding is active and the mount will be slewed so that
    /// `ProgramData::tracking.pos` reaches this value.
    guiding_pos: Option<Point2<i32>>,
    guiding_timer: Timer,
    guide_slewing: bool,
    calibration: Option<MountCalibration>,
    calibration_timer: Timer
}

impl MountData {
    pub fn calibration_in_progress(&self) -> bool {
        if let Some(calibration) = &self.calibration {
            calibration.primary_dir.is_none() || calibration.secondary_dir.is_none()
        } else {
            false
        }
    }
}

mod sim_data {
    use std::sync::atomic::{AtomicBool};
    use std::sync::Arc;

    /// Data shared between camera and mount simulators.
    #[derive(Clone)]
    pub struct MountSimulatorData {
        pub mount_connected: Arc<AtomicBool>,
        /// Value in camera simulator's pixels per second.
        pub primary_axis_speed: Arc<atomic_float::AtomicF32>,
        /// Value in camera simulator's pixels per second.
        pub secondary_axis_speed: Arc<atomic_float::AtomicF32>,
        sky_rotation_dir_in_img_space: cgmath::Vector2<i32>,
        primary_axis_slew_dir_in_img_space: cgmath::Vector2<i32>,
        sky_rotation_speed_pix_per_sec: u32
        //primary_axis_slewing_speed: Arc<Atomic
    }

    impl Default for MountSimulatorData {
        fn default() -> MountSimulatorData {
            MountSimulatorData {
                mount_connected: Arc::new(AtomicBool::new(false)),
                primary_axis_speed: Arc::new(atomic_float::AtomicF32::new(0.0)),
                secondary_axis_speed: Arc::new(atomic_float::AtomicF32::new(0.0)),
                sky_rotation_dir_in_img_space: cgmath::Vector2::new(1, 0),
                primary_axis_slew_dir_in_img_space: cgmath::Vector2::new(-1, 0),
                sky_rotation_speed_pix_per_sec: 10
            }
        }
    }

    impl MountSimulatorData {
        pub fn new(
            sky_rotation_dir_in_img_space: cgmath::Vector2<i32>,
            primary_axis_slew_dir_in_img_space: cgmath::Vector2<i32>,
            sky_rotation_speed_pix_per_sec: u32
        ) -> MountSimulatorData {
            MountSimulatorData{
                sky_rotation_dir_in_img_space,
                primary_axis_slew_dir_in_img_space,
                sky_rotation_speed_pix_per_sec,
                ..Default::default()
            }
        }

        pub fn sky_rotation_dir_in_img_space(&self) -> cgmath::Vector2<i32> {
            self.sky_rotation_dir_in_img_space
        }

        pub fn primary_axis_slew_dir_in_img_space(&self) -> cgmath::Vector2<i32> {
            self.primary_axis_slew_dir_in_img_space
        }

        pub fn sky_rotation_speed_pix_per_sec(&self) -> u32 {
            self.sky_rotation_speed_pix_per_sec
        }
    }
}
pub use sim_data::MountSimulatorData;

pub struct FocuserData {
    focuser: Option<Box<dyn devices::focuser::Focuser>>
}

#[derive(Debug)]
pub enum TrackingMode {
    Centroid(Rect),
    Anchor(Point2<i32>)
}

#[derive(Debug)]
pub struct TrackingData {
    pos: Point2<i32>,
    mode: TrackingMode
}

#[derive(Copy, Clone)]
pub enum OnCapturePauseAction {
    ControlChange(CameraControlChange),
    SetROI(Rect),
    DisableROI
}

pub struct ProgramData {
    config: Configuration,
    drivers: Vec<Rc<RefCell<Box<dyn camera::Driver>>>>,
    camera: Option<Box<dyn camera::Camera>>,
    capture_thread_data: Option<CaptureThreadData>,
    histogram_sender: crossbeam::channel::Sender<MainToHistogramThreadMsg>,
    recording_thread_data: RecordingThreadData,
    on_capture_pause_action: Option<OnCapturePauseAction>,
    preview_fps_counter: usize,
    preview_fps_last_timestamp: Option<std::time::Instant>,
    focuser_data: FocuserData,
    /// Non-empty after the main window creation.
    gui: Option<gui::GuiData>,
    mount_data: MountData,
    tracking: Option<TrackingData>,
    /// Area to record.
    crop_area: Option<Rect>,
    /// Area to use for calculating the histogram and/or stretching it for preview.
    histogram_area: Option<Rect>,
    /// True if the capture thread is sending images to a recording job.
    rec_job_active: bool,
    t_last_histogram: Option<std::time::Instant>,
    /// If true, raw color images are demosaiced for preview.
    demosaic_preview: bool,
    preview_fps_limit: Option<i32>,
    last_displayed_preview_image_timestamp: Option<std::time::Instant>,
    last_displayed_preview_image: Option<ga_image::Image>,
    snapshot_counter: usize,
    /// Used to refresh/rebuild all controls after user modification.
    camera_controls_refresh_timer: timer::Timer,
    mount_simulator_data: MountSimulatorData,
    #[cfg(feature = "controller")]
    sel_dialog_ctrl_events: Option<Vec<(std::time::Instant, workers::controller::StickEvent)>>,
    #[cfg(feature = "controller")]
    ctrl_actions: controller::ActionAssignments,
    #[cfg(feature = "controller")]
    ctrl_names: HashMap<u64, String>
}

impl ProgramData {
    /// Requests the ending of the capture thread dand performs a blocking wait for it.
    pub fn finish_capture_thread(&mut self) {
        if let Some(ref mut capture_thread_data) = self.capture_thread_data {
            let _ = capture_thread_data.sender.send(MainToCaptureThreadMsg::Finish);
            let _ = capture_thread_data.join_handle.take().unwrap().join();
        }
        if self.capture_thread_data.is_some() {
            self.capture_thread_data = None;
        }
    }

    /// Requests the ending of the recording thread and performs a blocking wait for it.
    pub fn finish_recording_thread(&mut self) {
        self.recording_thread_data.sender.send(MainToRecordingThreadMsg::Finish).unwrap();
        self.recording_thread_data.join_handle.take().unwrap().join().unwrap();
    }
}

fn main() {
    let args = args::parse_command_line(std::env::args());

    if args.logging { set_up_logging(); }

    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    log::info!("Vidoxide ver. {} on {} started", VERSION_STRING, os_info::get());

    let main_context = glib::MainContext::default();
    let _guard = main_context.acquire().unwrap();

    let (histogram_sender_worker, histogram_receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let (histogram_sender_main, histogram_receiver_worker) = crossbeam::channel::bounded(1);

    let (rec_sender_main, rec_recv_worker) = crossbeam::channel::unbounded();
    let rec_jobs = Arc::new(crossbeam::queue::SegQueue::<Job>::new());

    let (rec_sender_worker, rec_receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let buffered_kib = Arc::new(AtomicIsize::new(0));

    let config = Configuration::new();
    let disabled_drivers_str = config.disabled_drivers();
    let disabled_drivers: Vec<&str> = if disabled_drivers_str.is_empty() {
        vec![]
    } else {
        disabled_drivers_str.split(',').collect()
    };

    let simulator_video_file = config.simulator_video_file();

    let preview_fps_limit = config.preview_fps_limit();

    let mount_simulator_data = MountSimulatorData::new(
        config.mount_simulator_sky_rotation_dir_in_img_space().unwrap_or(cgmath::Vector2::new(1, 0)),
        config.mount_simulator_primary_axis_slew_dir_in_img_space().unwrap_or(cgmath::Vector2::new(1, 0)),
        config.mount_simulator_sky_rotation_speed_pix_per_sec().unwrap_or(10)
    );

    #[cfg(feature = "controller")]
    let ctrl_actions = config.controller_actions();

    let program_data_rc = Rc::new(RefCell::new(ProgramData{
        config,
        camera: None,
        drivers: drivers::init_drivers(&disabled_drivers, simulator_video_file),
        capture_thread_data: None,
        histogram_sender: histogram_sender_main,
        recording_thread_data: RecordingThreadData {
            jobs: rec_jobs.clone(),
            join_handle: Some(std::thread::spawn(
                clone!(@weak buffered_kib => @default-panic,
                    move || workers::recording::recording_thread(rec_jobs, rec_sender_worker, rec_recv_worker, buffered_kib)
                )
            )),
            sender: rec_sender_main,
            buffered_kib
        },
        on_capture_pause_action: None,
        preview_fps_counter: 0,
        preview_fps_last_timestamp: None,
        focuser_data: FocuserData{ focuser: None },
        gui: None,
        mount_data: MountData{
            mount: None,
            sky_tracking_on: false,
            guiding_pos: None,
            guiding_timer: Timer::new(),
            guide_slewing: false,
            calibration: None,
            calibration_timer: Timer::new()
        },
        tracking: None,
        crop_area: None,
        histogram_area: None,
        rec_job_active: false,
        t_last_histogram: None,
        demosaic_preview: false,
        preview_fps_limit,
        last_displayed_preview_image_timestamp: None,
        last_displayed_preview_image: None,
        camera_controls_refresh_timer: timer::Timer::new(),
        snapshot_counter: 1,
        mount_simulator_data,
        #[cfg(feature = "controller")]
        sel_dialog_ctrl_events: None,
        #[cfg(feature = "controller")]
        ctrl_actions,
        #[cfg(feature = "controller")]
        ctrl_names: HashMap::new()
    }));

    if !disabled_drivers.is_empty() {
        println!("The following drivers are disabled in the configuration file:");
        for dd in &disabled_drivers {
            println!("  {}", dd);
        }
    }

    std::thread::spawn(move || workers::histogram::histogram_thread(histogram_sender_worker, histogram_receiver_worker));
    histogram_receiver_main.attach(None, clone!(@weak program_data_rc
        => @default-panic, move |msg| {
            gui::on_histogram_thread_message(msg, &program_data_rc);
            glib::Continue(true)
        }
    ));

    rec_receiver_main.attach(None, clone!(@weak program_data_rc
        => @default-panic, move |msg| {
            gui::on_recording_thread_message(msg, &program_data_rc);
            glib::Continue(true)
        }
    ));

    let application = gtk::Application::new(
        None,
        Default::default(),
    );

    application.connect_activate(clone!(
        @weak program_data_rc
        => @default-panic, move |app| {
        gui::init_main_window(&app, &program_data_rc);
    }));

    init_timer(std::time::Duration::from_secs(1), &program_data_rc);

    #[cfg(feature = "controller")]
    init_controller_thread(&program_data_rc);

    application.run_with_args::<String>(&[]); // make GTK ignore command-line arguments

    program_data_rc.borrow_mut().finish_capture_thread();
    program_data_rc.borrow_mut().finish_recording_thread();
    program_data_rc.borrow_mut().camera = None; // make sure the camera is dropped before the drivers are

    if program_data_rc.borrow().config.store().is_err() {
        println!("WARNING: Failed to save configuration.");
    }
}

fn init_timer(timer_step: std::time::Duration, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let (sender_worker, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    receiver_main.attach(None, clone!(@weak program_data_rc => @default-panic, move |_| {
        gui::on_timer(&program_data_rc);
        glib::Continue(true)
    }));

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(timer_step);
            let _ = sender_worker.send(());
        }
    });

    // no need to demand and wait for timer thread's termination; let the runtime end it when the program ends
}

fn on_capture_thread_failure(program_data_rc: &Rc<RefCell<ProgramData>>) {
    gui::show_message(
        "Capture thread ended with error. Try reconnecting to the camera.",
        "Error",
        gtk::MessageType::Error,
        program_data_rc
    );
    gui::disconnect_camera(program_data_rc, true);
}

#[cfg(feature = "controller")]
fn init_controller_thread(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let (sender_worker, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    receiver_main.attach(None, clone!(@weak program_data_rc => @default-panic, move |msg| {
        gui::on_controller_event(msg, &program_data_rc);
        glib::Continue(true)
    }));

    std::thread::spawn(move || {
        workers::controller::controller_thread(sender_worker);
    });
}

fn set_up_logging() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("{}\n\n{}", info, backtrace);
    }));

    let tz_offset = chrono::Local::now().offset().clone();
    let logfile = dirs::data_dir().unwrap_or(std::path::Path::new("").to_path_buf())
        .join(format!("vidoxide_{}.log", chrono::Local::now().format("%Y-%m-%d_%H%M%S")));
    println!("Logging to: {}", logfile.to_string_lossy());
    simplelog::WriteLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::ConfigBuilder::new()
            .set_target_level(simplelog::LevelFilter::Error)
            .set_time_offset(time::UtcOffset::from_whole_seconds(tz_offset.local_minus_utc()).unwrap())
            .set_time_format_custom(simplelog::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]"
            ))
            .build(),
        std::fs::File::create(logfile).unwrap()
    ).unwrap();
}
