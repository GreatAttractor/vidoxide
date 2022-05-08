//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! IIDC (DC1394) camera driver.
//!

use crate::camera::*;
use ga_image;
use ga_image::Image;
use libdc1394_sys::*;
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::Arc;

macro_rules! checked_call {
    ($func_call:expr) => {
        match unsafe { $func_call } {
            dc1394error_t::DC1394_SUCCESS => (),
            error => return Err(CameraError::IIDCError(error))
        }
    }
}

pub type IIDCError = i32;

const NUM_DMA_BUFFERS: u32 = 4;

/// Additional ids to use beside the ones from `libdc1394_sys::dc1394feature_t`.
///
/// The Rust crate does not include `DC1394_FEATURE_MAX`, so we start counting from `u64::MAX`
/// down to avoid collisions with existing IIDC feature ids.
///
mod control_ids {
    pub const VIDEO_MODE: u64 = std::u64::MAX - 1;

    /// Selects one of the fixed framerates from the `dc1394framerate_t` enum
    /// (only for non-scalable = non-Format7 modes); a camera may also support `DC1394_FEATURE_FRAME_RATE`,
    /// which can change the frame rate independently and with finer granularity.
    pub const FIXED_FRAME_RATE: u64 = std::u64::MAX - 2;

    /// Selects pixel format (color coding) for the current video mode (non-Format7 modes have only one pixel format).
    pub const PIXEL_FORMAT: u64 = std::u64::MAX - 3;
}

// TODO: handle raw color properly
fn to_pix_fmt(color_coding: libdc1394_sys::dc1394color_coding_t::Type) -> ga_image::PixelFormat {
    match color_coding {
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_MONO8 => ga_image::PixelFormat::Mono8,
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_MONO16 => ga_image::PixelFormat::Mono16,
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_RGB8 => ga_image::PixelFormat::RGB8,
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_RGB16 => ga_image::PixelFormat::RGB16,
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_RAW8 => ga_image::PixelFormat::Mono8,
        libdc1394_sys::dc1394color_coding_t::DC1394_COLOR_CODING_RAW16 => ga_image::PixelFormat::Mono16,
        _ => panic!("Unsupported IIDC color coding: {:?}", color_coding)
    }
}

impl From<IIDCError> for CameraError {
    fn from(iidc_error: IIDCError) -> CameraError {
        CameraError::IIDCError(iidc_error)
    }
}

impl From<CameraId> for libdc1394_sys::dc1394camera_id_t {
    fn from(id: CameraId) -> libdc1394_sys::dc1394camera_id_t {
        libdc1394_sys::dc1394camera_id_t{ guid: id.id1, unit: id.id2 as u16 }
    }
}

impl From<libdc1394_sys::dc1394camera_id_t> for CameraId  {
    fn from(id: libdc1394_sys::dc1394camera_id_t) -> CameraId {
        CameraId{ id1: id.guid, id2: id.unit as u64 }
    }
}

pub struct CameraHandle {
    handle: *mut dc1394camera_t,
}

unsafe impl Send for CameraHandle {}

impl Drop for CameraHandle {
    fn drop(&mut self) {
        unsafe {
            dc1394_video_set_transmission(self.handle, dc1394switch_t::DC1394_OFF);
            dc1394_capture_stop(self.handle);
            dc1394_camera_free(self.handle);
        }
    }
}

unsafe impl Sync for CameraHandle {}

struct Context {
    handle: *mut dc1394_t
}

impl Context {
    fn new() -> Option<Context> {
        let handle = unsafe { dc1394_new() };
        if handle.is_null() {
            None
        } else {
            Some(Context{ handle })
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { dc1394_free(self.handle); }
    }
}

/// Returns true if the specified video mode is scalable (i.e. a FORMAT7 mode).
fn is_scalable(video_mode: dc1394video_mode_t::Type) -> bool {
    dc1394bool_t::DC1394_TRUE == unsafe { dc1394_is_video_mode_scalable(video_mode) }
}

fn color_coding_name(color_coding: dc1394color_coding_t::Type) -> &'static str {
    match color_coding {
        dc1394color_coding_t::DC1394_COLOR_CODING_MONO8   => "Mono 8-bit",
        dc1394color_coding_t::DC1394_COLOR_CODING_YUV411  => "YUV411",
        dc1394color_coding_t::DC1394_COLOR_CODING_YUV422  => "YUV422",
        dc1394color_coding_t::DC1394_COLOR_CODING_YUV444  => "YUV444",
        dc1394color_coding_t::DC1394_COLOR_CODING_RGB8    => "RGB 8-bit",
        dc1394color_coding_t::DC1394_COLOR_CODING_MONO16  => "Mono 16-bit",
        dc1394color_coding_t::DC1394_COLOR_CODING_RGB16   => "RGB 16-bit",
        dc1394color_coding_t::DC1394_COLOR_CODING_MONO16S => "Mono 16-bit (signed)",
        dc1394color_coding_t::DC1394_COLOR_CODING_RGB16S  => "RGB 16-bit (signed)",
        dc1394color_coding_t::DC1394_COLOR_CODING_RAW8    => "RAW 8-bit",
        dc1394color_coding_t::DC1394_COLOR_CODING_RAW16   => "RAW 16-bit",
        _ => panic!("Invalid color coding: {}", color_coding)
    }
}

fn frame_rate_name(frame_rate: dc1394framerate_t::Type) -> &'static str {
    match frame_rate {
        dc1394framerate_t::DC1394_FRAMERATE_1_875 => "1.875 fps",
        dc1394framerate_t::DC1394_FRAMERATE_3_75 => "3.75 fps",
        dc1394framerate_t::DC1394_FRAMERATE_7_5 => "7.5 fps",
        dc1394framerate_t::DC1394_FRAMERATE_15 => "15 fps",
        dc1394framerate_t::DC1394_FRAMERATE_30 => "30 fps",
        dc1394framerate_t::DC1394_FRAMERATE_60 => "60 fps",
        dc1394framerate_t::DC1394_FRAMERATE_120 => "120 fps",
        dc1394framerate_t::DC1394_FRAMERATE_240 => "240 fps",
        _ => panic!("Invalid fixed frame rate: {}", frame_rate)
    }
}

pub struct IIDCCamera {
    camera_handle: Arc<CameraHandle>,
    video_modes: Vec<dc1394video_mode_t::Type>,
    current_vid_mode: dc1394video_mode_t::Type,
    current_color_coding: dc1394color_coding_t::Type,
    /// Frame rates for non-scalable video modes from `video_modes`.
    frame_rates: HashMap<dc1394video_mode_t::Type, Vec<dc1394framerate_t::Type>>,
    /// Details of scalable video modes from `video_modes`.
    fmt7_info: HashMap<dc1394video_mode_t::Type, dc1394format7mode_t>,
    features: dc1394featureset_t,
    /// Copy of `DC1394_FEATURE_SHUTTER` control; used to notify the GUI about changes in value and range.
    shutter_ctrl: Option<NumberControl>,
    /// Copy of `DC1394_FEATURE_FRAME_RATE` control; used to notify the GUI about changes in value and range.
    fps_ctrl: Option<NumberControl>,
    temperature_abs_supported: Option<bool>,
    roi_offset: (u32, u32)
}

pub struct IIDCFrameCapturer {
    camera_handle: Arc<CameraHandle>,
    is_machine_big_endian: bool
}

impl IIDCCamera {
    fn get_raw_range(&self, id: dc1394feature_t::Type) -> Result<(u32, u32), CameraError> {
        let mut min_out = std::mem::MaybeUninit::uninit();
        let mut max_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_feature_get_boundaries(
            self.camera_handle.handle,
            id, min_out.as_mut_ptr(),
            max_out.as_mut_ptr()
        ));
        let mut raw_min = unsafe { min_out.assume_init() };
        let mut raw_max = unsafe { max_out.assume_init() };
        if raw_min > raw_max { std::mem::swap(&mut raw_min, &mut raw_max); }

        Ok((raw_min, raw_max))
    }

    fn get_absolute_range(&self, id: dc1394feature_t::Type) -> Result<(f32, f32), CameraError> {
        let mut min_out = std::mem::MaybeUninit::uninit();
        let mut max_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_feature_get_absolute_boundaries(
            self.camera_handle.handle,
            id, min_out.as_mut_ptr(),
            max_out.as_mut_ptr()
        ));
        let mut abs_min = unsafe { min_out.assume_init() };
        let mut abs_max = unsafe { max_out.assume_init() };
        if abs_min > abs_max { std::mem::swap(&mut abs_min, &mut abs_max); }

        Ok((abs_min, abs_max))
    }

    fn create_video_mode_control(&self) -> Result<ListControl, CameraError> {
        let mut items = vec![];

        for vid_mode in &self.video_modes {
            let mut width_out = std::mem::MaybeUninit::uninit();
            let mut height_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_get_image_size_from_video_mode(
                self.camera_handle.handle,
                *vid_mode,
                width_out.as_mut_ptr(),
                height_out.as_mut_ptr()
            ));
            let width = unsafe { width_out.assume_init() };
            let height = unsafe { height_out.assume_init() };

            let color_coding = get_color_coding_from_vid_mode(self.camera_handle.handle, *vid_mode)?;

            let mode_name = match self.fmt7_info.get(vid_mode) {
                Some(ref fmt7) => {
                    format!(
                        "{}x{} (FMT7: {})",
                        fmt7.max_size_x,
                        fmt7.max_size_y,
                        *vid_mode - dc1394video_mode_t::DC1394_VIDEO_MODE_FORMAT7_0
                    )
                },
                None => {
                    format!("{}x{} {}", width, height, color_coding_name(color_coding))
                }
            };

            items.push(mode_name);
        }

        Ok(ListControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::VIDEO_MODE),
                label: "Video Mode".to_string(),
                refreshable: false,
                // It is possible to check the current mode, but there is is no need;
                // the camera will never change it when other controls are manipulated.
                access_mode: ControlAccessMode::WriteOnly,
                auto_state: None,
                on_off_state: None,
                requires_capture_pause: true
            },
            items,
            current_idx: self.video_modes.iter().enumerate().find(|x| *x.1 == self.current_vid_mode).unwrap().0
        })
    }

    fn create_fixed_frame_rate_control_for_video_mode(
        &self,
        vid_mode: dc1394video_mode_t::Type
    ) -> Result<ListControl, CameraError> {
        let mut frame_rate_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_video_get_framerate(self.camera_handle.handle, frame_rate_out.as_mut_ptr()));
        let current_fr = unsafe { frame_rate_out.assume_init() };

        let mut items = vec![];
        let mut current_idx: Option<usize> = None;
        for (idx, fr) in self.frame_rates[&vid_mode].iter().enumerate() {
            items.push(frame_rate_name(*fr).to_string());
            if *fr == current_fr {
                current_idx = Some(idx);
            }
        }

        Ok(ListControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::FIXED_FRAME_RATE),
                label: "Fixed Frame Rate".to_string(),
                refreshable: false,
                // It is possible to check the current frame rate, but there is no need;
                // the camera will never change it when other controls are manipulated.
                access_mode: ControlAccessMode::WriteOnly,
                auto_state: None,
                on_off_state: None,
                requires_capture_pause: false
            },
            items,
            current_idx: current_idx.unwrap()
        })
    }

    fn create_pixel_format_control_for_video_mode(
        &self,
        vid_mode: dc1394video_mode_t::Type
    ) -> Result<ListControl, CameraError> {
        if is_scalable(vid_mode) {
            let fmt7 = &self.fmt7_info[&vid_mode];
            let mut format_names = vec![];
            let mut current_idx = 0;
            for i in 0..fmt7.color_codings.num {
                format_names.push(color_coding_name(fmt7.color_codings.codings[i as usize]).to_string());
                if self.current_color_coding == fmt7.color_codings.codings[i as usize] {
                    current_idx = i as usize;
                }
            }
            Ok(ListControl{
                base: CameraControlBase{
                    id: CameraControlId(control_ids::PIXEL_FORMAT),
                    label: "Pixel Format".to_string(),
                    refreshable: false,
                    access_mode: ControlAccessMode::WriteOnly,
                    auto_state: None,
                    on_off_state: None,
                    requires_capture_pause: true
                },
                items: format_names,
                current_idx
            })
        } else {
            Ok(ListControl{
                base: CameraControlBase{
                    id: CameraControlId(control_ids::PIXEL_FORMAT),
                    label: "Pixel Format".to_string(),
                    refreshable: false,
                    access_mode: ControlAccessMode::None,
                    auto_state: None,
                    on_off_state: None,
                    requires_capture_pause: false
                },
                items: vec![color_coding_name(
                    get_color_coding_from_vid_mode(self.camera_handle.handle, self.current_vid_mode)?
                ).to_string()],
                current_idx: 0
            })
        }
    }
}

impl Camera for IIDCCamera {
    fn id(&self) -> CameraId {
        CameraId{
            id1: unsafe { (*self.camera_handle.handle).guid },
            id2: unsafe { (*self.camera_handle.handle).unit as u64 }
        }
    }

    fn name(&self) -> &str {
        unsafe { CStr::from_ptr((*self.camera_handle.handle).model).to_str().unwrap() }
    }

    fn temperature(&self) -> Option<f64> {
        if !self.temperature_abs_supported.is_some() { return None; }

        let cam = self.camera_handle.handle;

        // IIDC does not report units of temperature; PGR CM3-U3-13S2M returns values in kelvins.

        if *self.temperature_abs_supported.as_ref().unwrap() {
            let mut val_out = std::mem::MaybeUninit::uninit();
            if dc1394error_t::DC1394_SUCCESS != unsafe { dc1394_feature_get_absolute_value(
                cam, dc1394feature_t::DC1394_FEATURE_TEMPERATURE, val_out.as_mut_ptr()
            ) } {
                None
            } else {
                let temp_kelvin = unsafe { val_out.assume_init() } as f64;
                Some(temp_kelvin - 273.15)
            }
        } else {
            let mut val_out = std::mem::MaybeUninit::uninit();
            if dc1394error_t::DC1394_SUCCESS != unsafe { dc1394_feature_get_value(
                cam, dc1394feature_t::DC1394_FEATURE_TEMPERATURE, val_out.as_mut_ptr()
            ) } {
                None
            } else {
                let temp_kelvin = unsafe { val_out.assume_init() } as f64;
                Some(temp_kelvin - 273.15)
            }
        }
    }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        let cam = self.camera_handle.handle;

        let mut controls = vec![
            CameraControl::List(self.create_video_mode_control()?),
            CameraControl::List(self.create_pixel_format_control_for_video_mode(self.current_vid_mode)?)
        ];

        if !is_scalable(self.current_vid_mode) {
            controls.push(
                CameraControl::List(self.create_fixed_frame_rate_control_for_video_mode(self.current_vid_mode)?)
            );
        }

        for feature in &self.features.feature {
            if feature.available != dc1394bool_t::DC1394_TRUE { continue; }

            let label = match feature.id {
                dc1394feature_t::DC1394_FEATURE_BRIGHTNESS => "Brightness",
                dc1394feature_t::DC1394_FEATURE_EXPOSURE => "Exposure",
                dc1394feature_t::DC1394_FEATURE_SHARPNESS => "Sharpness",

                // TODO: this is in fact a pair of controls
                // dc1394feature_t::DC1394_FEATURE_WHITE_BALANCE

                dc1394feature_t::DC1394_FEATURE_HUE => "Hue",
                dc1394feature_t::DC1394_FEATURE_SATURATION => "Saturation",
                dc1394feature_t::DC1394_FEATURE_GAMMA => "Gamma",
                dc1394feature_t::DC1394_FEATURE_SHUTTER => "Shutter",
                dc1394feature_t::DC1394_FEATURE_GAIN => "Gain",
                dc1394feature_t::DC1394_FEATURE_IRIS => "Iris",
                dc1394feature_t::DC1394_FEATURE_FOCUS => "Focus",
                dc1394feature_t::DC1394_FEATURE_TEMPERATURE => { continue; },

                // TODO: requires special handling
                // dc1394feature_t::DC1394_FEATURE_TRIGGER
                // dc1394feature_t::DC1394_FEATURE_TRIGGER_DELAY

                // TODO: this is in fact a triple of controls
                // dc1394feature_t::DC1394_FEATURE_WHITE_SHADING

                dc1394feature_t::DC1394_FEATURE_FRAME_RATE => "Frame Rate",
                dc1394feature_t::DC1394_FEATURE_ZOOM => "Zoom",
                dc1394feature_t::DC1394_FEATURE_PAN => "Pan",
                dc1394feature_t::DC1394_FEATURE_TILT => "Tilt",
                dc1394feature_t::DC1394_FEATURE_OPTICAL_FILTER => "Optical Filter",
                dc1394feature_t::DC1394_FEATURE_CAPTURE_SIZE => "Capture Size",
                dc1394feature_t::DC1394_FEATURE_CAPTURE_QUALITY => "Capture Quality",

                _ => { continue; }
            }.to_string();

            let supports_auto = feature.modes.modes.iter().take(feature.modes.num as usize).find(
                |&&x| x == dc1394feature_mode_t::DC1394_FEATURE_MODE_AUTO
            ).is_some();

            let mut mode_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_feature_get_mode(cam, feature.id, mode_out.as_mut_ptr()));
            let mode = unsafe { mode_out.assume_init() };

            let read_only = feature.modes.num == 0 && feature.readout_capable == dc1394bool_t::DC1394_TRUE;

            let supports_on_off = dc1394bool_t::DC1394_TRUE == feature.on_off_capable;

            let mut on_off_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_feature_get_power(cam, feature.id, on_off_out.as_mut_ptr()));
            let on_off_state = unsafe { on_off_out.assume_init() };

            let (raw_min, raw_max) = self.get_raw_range(feature.id)?;

            // A feature is "absolute control-capable" if its value can be set using floating-point arguments,
            // not just the integer "raw/driver" values. E.g., SHUTTER can be set in fractional "absolute" values
            // expressed in seconds.

            let value;
            let mut abs_min = 0.0;
            let mut abs_max = 0.0;

            if feature.absolute_capable == dc1394bool_t::DC1394_TRUE {
                checked_call!(dc1394_feature_set_absolute_control(cam, feature.id, dc1394switch_t::DC1394_ON));
                let abs_range = self.get_absolute_range(feature.id)?;
                abs_min = abs_range.0;
                abs_max = abs_range.1;

                if feature.readout_capable == dc1394bool_t::DC1394_TRUE {
                    let mut val_out = std::mem::MaybeUninit::uninit();
                    checked_call!(dc1394_feature_get_absolute_value(cam, feature.id, val_out.as_mut_ptr()));
                    value = unsafe { val_out.assume_init() } as f64;
                } else {
                    value = abs_min as f64;
                }
            } else {
                if feature.readout_capable == dc1394bool_t::DC1394_TRUE {
                    let mut val_out = std::mem::MaybeUninit::uninit();
                    checked_call!(dc1394_feature_get_value(cam, feature.id, val_out.as_mut_ptr()));
                    value = unsafe { val_out.assume_init() } as f64;
                } else {
                    value = raw_min as f64;
                }
            }

            let (min, max, step) = get_control_range_and_step(
                feature.absolute_capable == dc1394bool_t::DC1394_TRUE,
                raw_min,
                raw_max,
                abs_min,
                abs_max
            );

            let num_decimals = if feature.absolute_capable == dc1394bool_t::DC1394_TRUE && step < 1.0 {
                -step.log10() as usize + 2
            } else {
                0
            };

            let number_control = NumberControl{
                base: CameraControlBase{
                    id: CameraControlId(feature.id as u64),
                    label,
                    refreshable:
                        feature.id == dc1394feature_t::DC1394_FEATURE_SHUTTER ||
                        feature.id == dc1394feature_t::DC1394_FEATURE_GAIN,
                    access_mode: if read_only {
                        ControlAccessMode::ReadOnly
                    } else if feature.readout_capable == dc1394bool_t::DC1394_TRUE {
                        ControlAccessMode::ReadWrite
                    } else {
                        ControlAccessMode::WriteOnly
                    },
                    on_off_state: if supports_on_off { Some(on_off_state == dc1394switch_t::DC1394_ON) } else { None },
                    auto_state: if supports_auto {
                        Some(mode == dc1394feature_mode_t::DC1394_FEATURE_MODE_AUTO)
                    } else {
                        None
                    },
                    requires_capture_pause: false
                },
                value,
                min,
                max,
                step,
                num_decimals,
                is_exposure_time: feature.id == dc1394feature_t::DC1394_FEATURE_SHUTTER
            };

            if feature.id == dc1394feature_t::DC1394_FEATURE_SHUTTER {
                self.shutter_ctrl = Some(number_control.clone());
            } else if feature.id == dc1394feature_t::DC1394_FEATURE_FRAME_RATE {
                self.fps_ctrl = Some(number_control.clone());
            }

            controls.push(CameraControl::Number(number_control));
        }

        Ok(controls)
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        // The returned frame capturer will share `camera_handle` (via `Arc`). IIDC allows using its functions
        // from multiple threads without additional synchronization. The `IIDCCamera`'s instance will be used
        // by the main thread, and the `FrameCapturer`'s instance - by the capture thread.
        Ok(Box::new(IIDCFrameCapturer{
            camera_handle: self.camera_handle.clone(),
            is_machine_big_endian: 0x1122u16.to_be() == 0x1122
        }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError> {
        let is_absolute_capable = self.features.feature.iter().find(
            |x| x.id == id.0 as u32
        ).unwrap().absolute_capable == dc1394bool_t::DC1394_TRUE;

        let cam = self.camera_handle.handle;
        if is_absolute_capable {
            checked_call!(dc1394_feature_set_absolute_value(cam, id.0 as u32, value as f32));
        } else {
            checked_call!(dc1394_feature_set_value(cam, id.0 as u32, value as u32));
        }

        Ok(())
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError> {
        let cam = self.camera_handle.handle;

        match id.0 {
            control_ids::VIDEO_MODE => {
                checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_OFF));
                checked_call!(dc1394_capture_stop(cam));

                let vid_mode = self.video_modes[option_idx];
                self.current_vid_mode = vid_mode;
                checked_call!(dc1394_video_set_mode(cam, vid_mode));
                if !is_scalable(vid_mode) {
                    // set the last (probably the highest) framerate
                    checked_call!(dc1394_video_set_framerate(cam, *self.frame_rates[&vid_mode].last().unwrap()));
                } else {
                    let color_coding = get_color_coding_from_vid_mode(cam, vid_mode)?;

                    let fmt7 = &self.fmt7_info[&vid_mode];
                    checked_call!(dc1394_format7_set_roi(
                        cam,
                        vid_mode,
                        color_coding,
                        DC1394_USE_MAX_AVAIL,
                        0,
                        0,
                        fmt7.max_size_x as i32,
                        fmt7.max_size_y as i32
                    ));
                }

                checked_call!(dc1394_capture_setup(cam, NUM_DMA_BUFFERS, DC1394_CAPTURE_FLAGS_DEFAULT));
                checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_ON));
            },

            control_ids::FIXED_FRAME_RATE => {
                assert!(!is_scalable(self.current_vid_mode));
                checked_call!(dc1394_video_set_framerate(cam, self.frame_rates[&self.current_vid_mode][option_idx]));
            },

            control_ids::PIXEL_FORMAT => {
                checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_OFF));
                checked_call!(dc1394_capture_stop(cam));

                let fmt7 = &self.fmt7_info[&self.current_vid_mode];
                self.current_color_coding = fmt7.color_codings.codings[option_idx];

                checked_call!(dc1394_format7_set_roi(
                    self.camera_handle.handle,
                    self.current_vid_mode,
                    self.current_color_coding,
                    DC1394_USE_MAX_AVAIL,
                    0,
                    0,
                    fmt7.max_size_x as i32,
                    fmt7.max_size_y as i32
                ));

                checked_call!(dc1394_capture_setup(cam, NUM_DMA_BUFFERS, DC1394_CAPTURE_FLAGS_DEFAULT));
                checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_ON));
            },

            _ => panic!("Not implemented.")
        }

        Ok(())
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        let is_absolute_capable = self.features.feature.iter().find(
            |x| x.id == id.0 as u32
        ).unwrap().absolute_capable == dc1394bool_t::DC1394_TRUE;

        let cam = self.camera_handle.handle;
        if is_absolute_capable {
            let mut val_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_feature_get_absolute_value(cam, id.0 as u32, val_out.as_mut_ptr()));
            Ok(unsafe { val_out.assume_init() } as f64)
        } else {
            let mut val_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_feature_get_value(cam, id.0 as u32, val_out.as_mut_ptr()));
            Ok(unsafe { val_out.assume_init() } as f64)
        }
    }

    fn get_list_control(&self, _id: CameraControlId) -> Result<usize, CameraError> {
        panic!("Not implemented yet.")
    }

    fn set_auto(&self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        let new_mode = if state {
            dc1394feature_mode_t::DC1394_FEATURE_MODE_AUTO
        } else {
            dc1394feature_mode_t::DC1394_FEATURE_MODE_MANUAL
        };
        checked_call!(dc1394_feature_set_mode(self.camera_handle.handle, id.0 as u32, new_mode));

        Ok(())
    }

    fn set_on_off(&self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        let switch = if state { dc1394switch_t::DC1394_ON } else { dc1394switch_t::DC1394_OFF };
        checked_call!(dc1394_feature_set_power(self.camera_handle.handle, id.0 as u32, switch));

        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        self.roi_offset = (0, 0);

        if !is_scalable(self.current_vid_mode) {
            return Ok(())
        }

        let cam = self.camera_handle.handle;

        checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_OFF));
        checked_call!(dc1394_capture_stop(cam));

        let fmt7 = &self.fmt7_info[&self.current_vid_mode];

        checked_call!(dc1394_format7_set_roi(
            self.camera_handle.handle,
            self.current_vid_mode,
            self.current_color_coding,
            DC1394_USE_MAX_AVAIL,
            0,
            0,
            fmt7.max_size_x as i32,
            fmt7.max_size_y as i32
        ));

        checked_call!(dc1394_capture_setup(cam, NUM_DMA_BUFFERS, DC1394_CAPTURE_FLAGS_DEFAULT));
        checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_ON));

        Ok(())
    }

    fn set_roi(&mut self, x0: u32, y0: u32, width: u32, height: u32) -> Result<(), CameraError> {
        if !is_scalable(self.current_vid_mode) {
            return Err(CameraError::UnableToSetROI("ROI can only be set for Format7 video modes".to_string()));
        }

        let cam = self.camera_handle.handle;

        checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_OFF));
        checked_call!(dc1394_capture_stop(cam));

        let fmt7 = &self.fmt7_info[&self.current_vid_mode];

        // rounds `x` down to the closest multiple of `n`
        macro_rules! downmult { ($x:expr, $n:expr) => { ($x) / ($n) * ($n) } }

        checked_call!(dc1394_format7_set_roi(
            self.camera_handle.handle,
            self.current_vid_mode,
            self.current_color_coding,
            DC1394_USE_MAX_AVAIL,
            downmult!(x0 + self.roi_offset.0, fmt7.unit_pos_x) as i32,
            downmult!(y0 + self.roi_offset.1, fmt7.unit_pos_y) as i32,
            downmult!(width, fmt7.unit_size_x) as i32,
            downmult!(height, fmt7.unit_size_y) as i32
        ));

        checked_call!(dc1394_capture_setup(cam, NUM_DMA_BUFFERS, DC1394_CAPTURE_FLAGS_DEFAULT));
        checked_call!(dc1394_video_set_transmission(cam, dc1394switch_t::DC1394_ON));

        self.roi_offset.0 += x0;
        self.roi_offset.1 += y0;

        Ok(())
    }

    fn set_boolean_control(&mut self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_boolean_control(&self, _id: CameraControlId) -> Result<bool, CameraError> {
        unimplemented!()
    }
}

impl FrameCapturer for IIDCFrameCapturer {

    fn pause(&mut self) {}

    fn resume(&mut self) {}

    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        let mut frame_ptr: *mut dc1394video_frame_t = std::ptr::null_mut();
        checked_call!(dc1394_capture_dequeue(
            self.camera_handle.handle,
            dc1394capture_policy_t::DC1394_CAPTURE_POLICY_WAIT,
            &mut frame_ptr
        ));
        if frame_ptr.is_null() {
            return Err(CameraError::FrameUnavailable);
        }

        let frame = unsafe { frame_ptr.as_ref() }.unwrap();

        let frame_pixels: &[u8] = unsafe { std::slice::from_raw_parts(
            (*frame_ptr).image,
            (*frame_ptr).image_bytes as usize
        ) };

        if dest_image.bytes_per_line() != frame.stride as usize ||
            dest_image.width() != frame.size[0] ||
            dest_image.height() != frame.size[1] ||
            dest_image.pixel_format() != to_pix_fmt(frame.color_coding) {

            *dest_image = Image::new_from_pixels(
                frame.size[0],
                frame.size[1],
                Some(frame.stride as usize),
                to_pix_fmt(frame.color_coding),
                None,
                frame_pixels.to_vec()
            );
        } else {
            dest_image.raw_pixels_mut().copy_from_slice(frame_pixels);
        }

        if self.is_machine_big_endian ^ (frame.little_endian != dc1394bool_t::DC1394_TRUE) {
            dest_image.reverse_byte_order();
        }

        checked_call!(dc1394_capture_enqueue(self.camera_handle.handle, frame_ptr));

        Ok(())
    }
}

pub struct IIDCDriver {
    context: Context
}

impl IIDCDriver {
    pub fn new() -> Option<IIDCDriver> {
        match Context::new() {
            Some(context) => Some(IIDCDriver { context }),
            None => None
        }
    }

    fn camera_name_by_id(&self, id: dc1394camera_id_t) -> Result<String, dc1394error_t::Type> {
        let camera = self.create_camera(id)?;
        let name = unsafe { CStr::from_ptr((*camera.handle).model).to_str().unwrap().to_string() };
        Ok(name)
    }

    fn create_camera(&self, id: dc1394camera_id_t) -> Result<CameraHandle, dc1394error_t::Type> {
        let camera_ptr = unsafe {
            dc1394_camera_new_unit(self.context.handle, id.guid, id.unit as std::os::raw::c_int)
        };
        if camera_ptr.is_null() {
            return Err(dc1394error_t::DC1394_FAILURE);
        } else {
            Ok(CameraHandle{ handle: camera_ptr })
        }
    }
}

impl Driver for IIDCDriver {
    fn name(&self) -> &'static str { "IIDC" }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        let mut camera_list_ptr = std::ptr::null_mut();
        checked_call!(dc1394_camera_enumerate(self.context.handle, &mut camera_list_ptr));
        if camera_list_ptr.is_null() { return Err(CameraError::IIDCError(dc1394error_t::DC1394_FAILURE)); }
        let camera_list = unsafe { camera_list_ptr.as_ref() }.unwrap();
        let camera_ids = unsafe { std::slice::from_raw_parts(camera_list.ids, camera_list.num as usize) };

        Ok(camera_ids.iter().map(
            |id| CameraInfo{ id: CameraId::from(*id), name: self.camera_name_by_id(*id).unwrap() }
        ).collect())
    }

    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        let camera_handle = self.create_camera(dc1394camera_id_t::from(id))?;

        let mut video_modes_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_video_get_supported_modes(camera_handle.handle, video_modes_out.as_mut_ptr()));
        let video_modes_struct = unsafe { video_modes_out.assume_init() };
        let video_modes = &video_modes_struct.modes[0..video_modes_struct.num as usize];

        let mut frame_rates = HashMap::new();
        let mut fmt7_info = HashMap::new();

        for vmode in video_modes {
            if is_scalable(*vmode) {
                let mut fmt7_struct = unsafe { std::mem::MaybeUninit::<dc1394format7mode_t>::zeroed().assume_init() };
                fmt7_struct.present = dc1394bool_t::DC1394_TRUE;
                checked_call!(dc1394_format7_get_mode_info(camera_handle.handle, *vmode, &mut fmt7_struct));
                fmt7_info.insert(*vmode, fmt7_struct);
            } else {
                let mut mode_frame_rates_out = std::mem::MaybeUninit::uninit();
                checked_call!(dc1394_video_get_supported_framerates(
                    camera_handle.handle,
                    *vmode,
                    mode_frame_rates_out.as_mut_ptr()
                ));
                let mode_frame_rates = unsafe { mode_frame_rates_out.assume_init() };
                frame_rates.insert(
                    *vmode,
                    mode_frame_rates.framerates[0..mode_frame_rates.num as usize].to_vec()
                );
            }
        }

        checked_call!(dc1394_capture_setup(camera_handle.handle, NUM_DMA_BUFFERS, DC1394_CAPTURE_FLAGS_DEFAULT));
        checked_call!(dc1394_video_set_transmission(camera_handle.handle, dc1394switch_t::DC1394_ON));

        let mut cvm_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_video_get_mode(camera_handle.handle, cvm_out.as_mut_ptr()));
        let current_vid_mode = unsafe { cvm_out.assume_init() };

        let current_color_coding = if is_scalable(current_vid_mode) {
            let mut cc_out = std::mem::MaybeUninit::uninit();
            checked_call!(dc1394_format7_get_color_coding(
                camera_handle.handle,
                current_vid_mode,
                cc_out.as_mut_ptr()
            ));
            unsafe { cc_out.assume_init() }
        } else {
            get_color_coding_from_vid_mode(camera_handle.handle, current_vid_mode)?
        };

        let mut features_out = std::mem::MaybeUninit::uninit();
        checked_call!(dc1394_feature_get_all(camera_handle.handle, features_out.as_mut_ptr()));
        let features = unsafe { features_out.assume_init() };

        let temperature_abs_supported = match features.feature.iter().find(
            |f| f.available == dc1394bool_t::DC1394_TRUE && f.id == dc1394feature_t::DC1394_FEATURE_TEMPERATURE
        ) {
            None => None,
            Some(f) => Some(f.absolute_capable == dc1394bool_t::DC1394_TRUE)
        };

        Ok(Box::new(IIDCCamera{
            camera_handle: Arc::new(camera_handle),
            video_modes: video_modes.to_vec(),
            current_vid_mode,
            current_color_coding,
            frame_rates,
            fmt7_info,
            features,
            shutter_ctrl: None,
            fps_ctrl: None,
            temperature_abs_supported,
            roi_offset: (0, 0)
        }))
    }
}

fn get_color_coding_from_vid_mode(
    camera: *mut dc1394camera_t,
    vid_mode: dc1394video_mode_t::Type
) -> Result<dc1394color_coding_t::Type, CameraError> {
    let mut color_coding_out = std::mem::MaybeUninit::uninit();
    checked_call!(dc1394_get_color_coding_from_video_mode(
        camera,
        vid_mode,
        color_coding_out.as_mut_ptr()
    ));
    Ok(unsafe { color_coding_out.assume_init() })
}

/// Returns (min, max, step) for a number camera control.
fn get_control_range_and_step(absolute_capable: bool, raw_min: u32, raw_max: u32, abs_min: f32, abs_max: f32)
-> (f64, f64, f64) {
    let min;
    let max;
    let step;

    if absolute_capable {
        if raw_max != raw_min {
            step = (abs_max - abs_min) as f64 / (raw_max - raw_min + 1) as f64;
        } else {
            step = 0.0;
        }

        min = abs_min as f64;
        max = abs_max as f64;
    }
    else
    {
        min = raw_min as f64;
        max = raw_max as f64;
        step = if raw_min != raw_max { 1.0 } else { 0.0 };
    }

    (min, max, step)
}
