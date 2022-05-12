//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Common camera code.
//!

pub mod drivers;

use enum_dispatch::enum_dispatch;
use ga_image::Image;

#[derive(Debug)]
pub enum CameraError {
    FrameUnavailable,
    UnableToSetROI(String),
    SimulatorError(drivers::simulator::SimulatorError),
    #[cfg(feature = "camera_iidc")]
    IIDCError(drivers::iidc::IIDCError),
    #[cfg(feature = "camera_v4l2")]
    V4L2Error(drivers::v4l2::V4L2Error),
    #[cfg(feature = "camera_flycap2")]
    FlyCapture2Error(drivers::flycapture2::FlyCapture2Error),
    #[cfg(feature = "camera_spinnaker")]
    SpinnakerError(drivers::spinnaker::SpinnakerError),
    #[cfg(feature = "camera_asi")]
    ASIError(drivers::asi::ASIError),
}

#[derive(Clone, Copy)]
pub struct CameraId {
    pub id1: u64,
    pub id2: u64
}

pub struct CameraInfo {
    id: CameraId,
    name: String
}

impl CameraInfo {
    pub fn id(&self) -> CameraId { self.id }
    pub fn name(&self) -> &str { &self.name }
}

pub trait Camera {
    fn id(&self) -> CameraId;

    fn name(&self) -> &str;

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError>;

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError>;

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError>;

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError>;

    fn set_boolean_control(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError>;

    fn set_auto(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError>;

    fn set_on_off(&self, id: CameraControlId, state: bool) -> Result<(), CameraError>;

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError>;

    fn get_list_control(&self, id: CameraControlId) -> Result<usize, CameraError>;

    fn get_boolean_control(&self, id: CameraControlId) -> Result<bool, CameraError>;

    /// Returns temperature in degrees Celsius.
    fn temperature(&self) -> Option<f64>;

    /// Sets ROI (position is relative to the previously set ROI, if any).
    fn set_roi(&mut self, x0: u32, y0: u32, width: u32, height: u32) -> Result<(), CameraError>;

    /// Restores full frame size.
    fn unset_roi(&mut self) -> Result<(), CameraError>;
}

pub trait FrameCapturer {
    /// Captures a frame to the specified buffer.
    ///
    /// May change the dimensions, stride and pixel format of `dest_image`.
    // TODO: add policy (wait, poll)
    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError>;

    fn pause(&mut self) -> Result<(), CameraError>;

    fn resume(&mut self) -> Result<(), CameraError>;
}

pub trait Driver {
    fn name(&self) -> &'static str;

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError>;

    /// Returns camera with capture enabled.
    ///
    /// # Parameters
    ///
    /// * `id` - One of IDs returned by the most recent call to `enumerate_cameras`.
    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError>;
}

#[enum_dispatch]
#[derive(Debug)]
pub enum CameraControl {
    Number(NumberControl),
    List(ListControl),
    Boolean(BooleanControl)
}

#[enum_dispatch(CameraControl)]
pub trait BaseProperties {
    fn base(&self) -> &CameraControlBase;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControlAccessMode {
    /// Can be read by `Camera::get_*_control` and written by `Camera::set_*_control`.
    ReadWrite,
    /// Can be read by `Camera::get_*_control`.
    ReadOnly,
    /// Can be written by `Camera::set_*_control`.
    WriteOnly,
    /// No read/write possible; value is obtained via `Camera::enumerate_controls`.
    None
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CameraControlId(u64);

#[derive(Clone, Debug)]
pub struct CameraControlBase {
    pub id: CameraControlId,
    pub label: String,
    /// If true, the value may change on its own (e.g., exposure time and/or gain if automatic exposure is enabled),
    /// and should be periodically read and refreshed on-screen.
    pub refreshable: bool,
    pub access_mode: ControlAccessMode,
    /// If `None`, "auto" mode is not supported.
    pub auto_state: Option<bool>,
    /// If `None`, on/off toggling is not supported.
    pub on_off_state: Option<bool>,
    /// If true, capture thread must be paused before changing this control
    /// and resumed afterwards.
    pub requires_capture_pause: bool
}

#[derive(Clone, Debug)]
pub struct NumberControl {
    base: CameraControlBase,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    num_decimals: usize,
    is_exposure_time: bool
}

impl NumberControl {
    pub fn value(&self) -> f64 { self.value }
    pub fn min(&self) -> f64 { self.min }
    pub fn max(&self) -> f64 { self.max }
    pub fn step(&self) -> f64 { self.step }
    pub fn num_decimals(&self) -> usize { self.num_decimals }
    pub fn is_exposure_time(&self) -> bool { self.is_exposure_time }
}

impl BaseProperties for NumberControl {
    fn base(&self) -> &CameraControlBase { &self.base }
}

#[derive(Debug)]
pub struct ListControl {
    base: CameraControlBase,
    items: Vec<String>,
    current_idx: usize
}

impl ListControl {
    pub fn items(&self) -> &Vec<String> { &self.items }
    pub fn current_idx(&self) -> usize { self.current_idx }
}

impl BaseProperties for ListControl {
    fn base(&self) -> &CameraControlBase { &self.base }
}

#[derive(Debug)]
pub struct BooleanControl {
    base: CameraControlBase,
    state: bool
}

impl BooleanControl {
    pub fn base(&self) -> &CameraControlBase { &self.base }
    pub fn state(&self) -> bool { self.state }
}

impl BaseProperties for BooleanControl {
    fn base(&self) -> &CameraControlBase { &self.base }
}
