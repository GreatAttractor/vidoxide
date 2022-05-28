//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! ZWO ASI camera driver.
//!

use crate::camera::*;
use ga_image;
use libasicamera_sys::*;
use std::collections::HashMap;
use std::ffi::CStr;

macro_rules! checked_call {
    ($func_call:expr) => {
        match unsafe { $func_call } as _ {
            ASI_ERROR_CODE_ASI_SUCCESS => (),
            error => return Err(ASIError::Internal(error).into())
        }
    }
}

const MAX_NO_FRAME_PERIOD: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug)]
pub enum ASIError {
    Internal(ASI_ERROR_CODE),
    UnsupportedPixelFormat(ASI_IMG_TYPE),
    UnknownBinningMethod(std::os::raw::c_int)
}

impl From<ASIError> for CameraError {
    fn from(asi_error: ASIError) -> CameraError {
        CameraError::ASIError(asi_error)
    }
}

fn from_asi_bool(value: ASI_BOOL) -> bool {
    value == ASI_BOOL_ASI_TRUE
}

fn to_asi_bool(value: bool) -> ASI_BOOL {
    if value { ASI_BOOL_ASI_TRUE } else { ASI_BOOL_ASI_FALSE }
}

fn to_pix_fmt(img_type: ASI_IMG_TYPE, cfa_pattern: ASI_BAYER_PATTERN) -> Result<ga_image::PixelFormat, CameraError> {
    match (img_type, cfa_pattern) {
        (ASI_IMG_TYPE_ASI_IMG_RAW8, ASI_BAYER_PATTERN_ASI_BAYER_RG) => Ok(ga_image::PixelFormat::CfaRGGB8),
        (ASI_IMG_TYPE_ASI_IMG_RAW8, ASI_BAYER_PATTERN_ASI_BAYER_BG) => Ok(ga_image::PixelFormat::CfaBGGR8),
        (ASI_IMG_TYPE_ASI_IMG_RAW8, ASI_BAYER_PATTERN_ASI_BAYER_GR) => Ok(ga_image::PixelFormat::CfaGRBG8),
        (ASI_IMG_TYPE_ASI_IMG_RAW8, ASI_BAYER_PATTERN_ASI_BAYER_GB) => Ok(ga_image::PixelFormat::CfaGBRG8),

        (ASI_IMG_TYPE_ASI_IMG_RAW16, ASI_BAYER_PATTERN_ASI_BAYER_RG) => Ok(ga_image::PixelFormat::CfaRGGB16),
        (ASI_IMG_TYPE_ASI_IMG_RAW16, ASI_BAYER_PATTERN_ASI_BAYER_BG) => Ok(ga_image::PixelFormat::CfaBGGR16),
        (ASI_IMG_TYPE_ASI_IMG_RAW16, ASI_BAYER_PATTERN_ASI_BAYER_GR) => Ok(ga_image::PixelFormat::CfaGRBG16),
        (ASI_IMG_TYPE_ASI_IMG_RAW16, ASI_BAYER_PATTERN_ASI_BAYER_GB) => Ok(ga_image::PixelFormat::CfaGBRG16),

        (ASI_IMG_TYPE_ASI_IMG_RGB24, _) => Ok(ga_image::PixelFormat::RGB8),
        (ASI_IMG_TYPE_ASI_IMG_Y8, _) => Ok(ga_image::PixelFormat::Mono8),

        _ => Err(ASIError::UnsupportedPixelFormat(img_type).into())
    }
}

/// Documentation does not say if the provided char arrays (e.g., `ASI_CONTROL_CAPS::Name`) are always NUL-terminated,
/// so let us make sure ourselves.
fn asi_char_array_to_string(chars: &[std::os::raw::c_char]) -> String {
    let nul_exists = chars.iter().find(|ch| **ch == 0).is_some();
    if nul_exists {
        String::from(unsafe { std::ffi::CStr::from_ptr(chars.as_ptr()) }.to_str().unwrap())
    } else {
        let u8_vec: Vec<u8> = chars.iter().map(|ch| *ch as u8).collect();
        String::from_utf8(u8_vec).unwrap()
    }
}

pub struct ASIDriver {}

impl ASIDriver {
    pub fn new() -> Option<ASIDriver> {
        let version = unsafe { std::ffi::CStr::from_ptr(ASIGetSDKVersion()) };
        println!("ASICamera2 version: {}", version.to_str().unwrap());

        Some(ASIDriver{})
    }
}

impl Driver for ASIDriver {
    fn name(&self) -> &'static str {
        "ASI"
    }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        let mut cameras = vec![];

        let num_cameras = unsafe { ASIGetNumOfConnectedCameras() };
        for i in 0..num_cameras {
            let mut camera_info = std::mem::MaybeUninit::uninit();
            checked_call!(ASIGetCameraProperty(camera_info.as_mut_ptr(), i));
            let camera_info = unsafe { camera_info.assume_init() };
            cameras.push(CameraInfo{
                id: CameraId{ id1: camera_info.CameraID as u64, id2: 0 },
                name: asi_char_array_to_string(&camera_info.Name)
            });
        }

        Ok(cameras)
    }

    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        let asi_id = id.id1 as std::os::raw::c_int;

        let mut camera_info = std::mem::MaybeUninit::uninit();
        checked_call!(ASIGetCameraProperty(camera_info.as_mut_ptr(), asi_id));
        let camera_info = unsafe { camera_info.assume_init() };

        checked_call!(ASIOpenCamera(asi_id));
        checked_call!(ASIInitCamera(asi_id));

        let (_, _, img_type) = get_roi_format(asi_id)?;
        // in case the camera has a pixel format enabled which we do not support
        to_pix_fmt(img_type, camera_info.BayerPattern)?;

        checked_call!(ASIStartVideoCapture(asi_id));

        Ok(Box::new(ASICamera{
            id: asi_id,
            full_frame_size: (camera_info.MaxWidth as _, camera_info.MaxHeight as _),
            name: asi_char_array_to_string(&camera_info.Name),
            cfa_pattern: camera_info.BayerPattern,
            control_auto_state: HashMap::new()
        }))
    }
}

pub struct ASICamera {
    id: std::os::raw::c_int,
    cfa_pattern: ASI_BAYER_PATTERN,
    full_frame_size: (u32, u32),
    name: String,
    control_auto_state: HashMap<u64, Option<bool>>
}

impl Drop for ASICamera {
    fn drop(&mut self) {
        unsafe { ASIStopVideoCapture(self.id) };
        unsafe { ASICloseCamera(self.id) };
    }
}

impl Camera for ASICamera {
    fn id(&self) -> CameraId {
        CameraId{
            id1: self.id as u64,
            id2: 0
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        let mut num_controls = std::mem::MaybeUninit::uninit();
        checked_call!(ASIGetNumOfControls(self.id, num_controls.as_mut_ptr()));
        let num_controls = unsafe { num_controls.assume_init() };

        self.control_auto_state.clear();

        let mut controls = vec![];

        for control_idx in 0..num_controls {
            let mut ccaps = std::mem::MaybeUninit::uninit();
            checked_call!(ASIGetControlCaps(self.id, control_idx, ccaps.as_mut_ptr()));
            let ccaps = unsafe { ccaps.assume_init() };

            let mut value = std::mem::MaybeUninit::uninit();
            let mut is_auto = std::mem::MaybeUninit::uninit();
            checked_call!(ASIGetControlValue(self.id, ccaps.ControlType as _, value.as_mut_ptr(), is_auto.as_mut_ptr()));
            let value = unsafe { value.assume_init() };
            let is_auto = unsafe { is_auto.assume_init() };

            let id = CameraControlId(ccaps.ControlType as u64);
            let mut control_added = false;
            let mut auto_state: Option<bool> = None;

            match ccaps.ControlType {
                ASI_CONTROL_TYPE_ASI_GAIN
                | ASI_CONTROL_TYPE_ASI_EXPOSURE
                | ASI_CONTROL_TYPE_ASI_GAMMA
                | ASI_CONTROL_TYPE_ASI_WB_R
                | ASI_CONTROL_TYPE_ASI_WB_B
                | ASI_CONTROL_TYPE_ASI_BANDWIDTHOVERLOAD
                | ASI_CONTROL_TYPE_ASI_OFFSET
                | ASI_CONTROL_TYPE_ASI_AUTO_MAX_GAIN
                | ASI_CONTROL_TYPE_ASI_AUTO_MAX_EXP
                | ASI_CONTROL_TYPE_ASI_AUTO_TARGET_BRIGHTNESS
                | ASI_CONTROL_TYPE_ASI_COOLER_POWER_PERC
                | ASI_CONTROL_TYPE_ASI_TARGET_TEMP => {
                    auto_state = if from_asi_bool(ccaps.IsAutoSupported) {
                        Some(from_asi_bool(is_auto as _))
                    } else {
                        None
                    };

                    controls.push(CameraControl::Number(NumberControl{
                        base: CameraControlBase{
                            id,
                            label: asi_char_array_to_string(&ccaps.Name),
                            refreshable:
                                ccaps.ControlType == ASI_CONTROL_TYPE_ASI_GAIN ||
                                ccaps.ControlType == ASI_CONTROL_TYPE_ASI_EXPOSURE,
                            access_mode:
                                if ccaps.IsWritable == ASI_BOOL_ASI_FALSE {
                                    ControlAccessMode::ReadOnly
                                } else {
                                    ControlAccessMode::ReadWrite
                                },
                            on_off_state: None,
                            auto_state: auto_state.clone(),
                            requires_capture_pause: false
                        },
                        value: value as f64,
                        min: ccaps.MinValue as f64,
                        max: ccaps.MaxValue as f64,
                        step: 1.0, //TODO
                        num_decimals: 0,
                        is_exposure_time: ccaps.ControlType == ASI_CONTROL_TYPE_ASI_EXPOSURE
                    }));

                    control_added = true;
                },

                // ASI_CONTROL_TYPE_ASI_OFFSET => ,
                // ASI_CONTROL_TYPE_ASI_OVERCLOCK => ,

                ASI_CONTROL_TYPE_ASI_TEMPERATURE => (), // temperature is handled separately

                // ASI_CONTROL_TYPE_ASI_FLIP => ,
                // ASI_CONTROL_TYPE_ASI_HARDWARE_BIN => ,

                ASI_CONTROL_TYPE_ASI_HIGH_SPEED_MODE => {
                    auto_state = None;

                    controls.push(CameraControl::Boolean(BooleanControl{
                        base: CameraControlBase{
                            id,
                            label: asi_char_array_to_string(&ccaps.Name),
                            refreshable: false,
                            access_mode: ControlAccessMode::WriteOnly,
                            auto_state: None,
                            on_off_state: None,
                            requires_capture_pause: true
                        },
                        state: from_asi_bool(value as _)
                    }));

                    control_added = true;
                },

                // ASI_CONTROL_TYPE_ASI_COOLER_ON => ,
                // ASI_CONTROL_TYPE_ASI_MONO_BIN => ,
                // ASI_CONTROL_TYPE_ASI_FAN_ON => ,
                // ASI_CONTROL_TYPE_ASI_PATTERN_ADJUST => ,
                // ASI_CONTROL_TYPE_ASI_ANTI_DEW_HEATER => ,

                _ => println!("Ignoring unsupported camera control {}.", ccaps.ControlType)
            }

            if control_added {
                self.control_auto_state.insert(id.0, auto_state);
            }
        }

        Ok(controls)
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        let (img_width, img_height, img_type) = get_roi_format(self.id)?;
        Ok(Box::new(ASIFrameCapturer{
            camera_id: self.id,
            cfa_pattern: self.cfa_pattern,
            img_width,
            img_height,
            pixel_format: to_pix_fmt(img_type, self.cfa_pattern)?,
            last_timeout: None
        }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError> {
        checked_call!(ASISetControlValue(
            self.id,
            id.0 as _,
            value as _,
            match self.control_auto_state.get(&id.0).unwrap() {
                Some(auto_state) => to_asi_bool(*auto_state),
                None => ASI_BOOL_ASI_FALSE
            } as _
        ));

        Ok(())
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn set_boolean_control(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        checked_call!(ASISetControlValue(
            self.id,
            id.0 as _,
            (if state { ASI_BOOL_ASI_TRUE } else { ASI_BOOL_ASI_FALSE }) as _,
            match self.control_auto_state.get(&id.0).unwrap() {
                Some(auto_state) => to_asi_bool(*auto_state),
                None => ASI_BOOL_ASI_FALSE
            } as _
        ));

        Ok(())
    }

    fn set_auto(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        let mut value = std::mem::MaybeUninit::uninit();
        let mut dummy = std::mem::MaybeUninit::uninit();
        checked_call!(ASIGetControlValue(
            self.id,
            id.0 as _,
            value.as_mut_ptr(),
            dummy.as_mut_ptr()
        ));
        let value = unsafe { value.assume_init() };

        checked_call!(ASISetControlValue(
            self.id,
            id.0 as _,
            value,
            to_asi_bool(state) as _
        ));

        *self.control_auto_state.get_mut(&id.0).unwrap() = Some(state);

        Ok(())
    }

    fn set_on_off(&self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        let mut value = std::mem::MaybeUninit::uninit();
        let mut is_auto = std::mem::MaybeUninit::uninit();
        checked_call!(ASIGetControlValue(
            self.id,
            id.0 as _,
            value.as_mut_ptr(),
            is_auto.as_mut_ptr()
        ));
        let value = unsafe { value.assume_init() };

        Ok(value as f64)
    }

    fn get_list_control(&self, id: CameraControlId) -> Result<usize, CameraError> {
        unimplemented!()
    }

    fn get_boolean_control(&self, id: CameraControlId) -> Result<bool, CameraError> {
        unimplemented!()
    }

    fn temperature(&self) -> Option<f64> {
        let mut temperature = std::mem::MaybeUninit::uninit();
        let mut dummy = std::mem::MaybeUninit::uninit();
        let result = unsafe { ASIGetControlValue(
            self.id,
            ASI_CONTROL_TYPE_ASI_TEMPERATURE as _,
            temperature.as_mut_ptr(),
            dummy.as_mut_ptr()
        ) };
        match result as _ {
            ASI_ERROR_CODE_ASI_SUCCESS => {
                let temperature = unsafe { temperature.assume_init() };
                Some(temperature as f64 / 10.0)
            },

            _ => None
        }
    }

    fn set_roi(&mut self, x0: u32, y0: u32, width: u32, height: u32) -> Result<(), CameraError> {
        // ASI 120 requires width * height divisible by 1024 (other cameras are less stringent; TODO: take it into account)
        let mut actual_w = width / 32 * 32;
        let mut actual_h = height / 32 * 32;

        let (_, _, img_type) = get_roi_format(self.id)?;

        checked_call!(ASISetROIFormat(self.id, actual_w as _, actual_h as _ , /*TODO: binning*/1, img_type as _));
        checked_call!(ASISetStartPos(self.id, x0 as _, y0 as _));

        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        let (_, _, img_type) = get_roi_format(self.id)?;

        checked_call!(ASISetStartPos(self.id, 0, 0));
        checked_call!(ASISetROIFormat(
            self.id,
            self.full_frame_size.0 as _,
            self.full_frame_size.1 as _,
            /*TODO*/1,
            img_type
        ));

        Ok(())
    }
}

pub struct ASIFrameCapturer {
    camera_id: std::os::raw::c_int,
    cfa_pattern: ASI_BAYER_PATTERN,
    img_width: u32,
    img_height: u32,
    pixel_format: ga_image::PixelFormat,
    last_timeout: Option<std::time::Instant>
}

impl FrameCapturer for ASIFrameCapturer {
    fn capture_frame(&mut self, dest_image: &mut ga_image::Image) -> Result<(), CameraError> {
        if dest_image.width() != self.img_width ||
           dest_image.height() != self.img_height ||
           dest_image.bytes_per_line() != self.img_width as usize * self.pixel_format.bytes_per_pixel() ||
           dest_image.pixel_format() != self.pixel_format {

            *dest_image = ga_image::Image::new(self.img_width, self.img_height, None, self.pixel_format, None, false);
        }

        let wait_timeout_ms = 500;

        let num_pixel_bytes = dest_image.pixels::<u8>().len();
        let result = unsafe {
            //TODO: use a proper timeout
            ASIGetVideoData(
                self.camera_id,
                dest_image.pixels_mut::<u8>().as_mut_ptr(),
                num_pixel_bytes as _,
                wait_timeout_ms
            )
        };
        match result as _ {
            ASI_ERROR_CODE_ASI_SUCCESS => self.last_timeout = None,

            ASI_ERROR_CODE_ASI_ERROR_TIMEOUT => {
                let now = std::time::Instant::now();
                if let Some(last_timeout) = self.last_timeout {
                    if last_timeout.elapsed() > MAX_NO_FRAME_PERIOD {
                        return Err(ASIError::Internal(result as _).into());
                    }
                } else {
                    self.last_timeout = Some(now);
                }
                println!(
                    "No data available for {} ms; skipping frame.",
                    self.last_timeout.as_ref().unwrap().elapsed().as_millis() + wait_timeout_ms as u128
                );
            },

            _ => return Err(ASIError::Internal(result as _).into())
        }

        Ok(())
    }

    fn pause(&mut self) -> Result<(), CameraError> {
        checked_call!(ASIStopVideoCapture(self.camera_id));
        Ok(())
    }

    fn resume(&mut self) -> Result<(), CameraError> {
        // one reason for pausing is a ROI change; re-read the image size and pixel format before resuming
        let (img_width, img_height, img_type) = get_roi_format(self.camera_id)?;
        self.img_width = img_width;
        self.img_height = img_height;
        self.pixel_format = to_pix_fmt(img_type, self.cfa_pattern)?;

        checked_call!(ASIStartVideoCapture(self.camera_id));
        Ok(())
    }
}

/// Returns (width, height, ASI image type).
fn get_roi_format(camera_id: std::os::raw::c_int)
-> Result<(u32, u32, ASI_IMG_TYPE), CameraError> {
    let mut img_width = std::mem::MaybeUninit::uninit();
    let mut img_height = std::mem::MaybeUninit::uninit();
    let mut binning = std::mem::MaybeUninit::uninit();
    let mut img_type = std::mem::MaybeUninit::uninit();
    checked_call!(ASIGetROIFormat(
        camera_id,
        img_width.as_mut_ptr(),
        img_height.as_mut_ptr(),
        binning.as_mut_ptr(),
        img_type.as_mut_ptr()
    ));
    let mut img_width = unsafe { img_width.assume_init() } as u32;
    let mut img_height = unsafe { img_height.assume_init() } as u32;
    let binning = unsafe { binning.assume_init() };
    let img_type = unsafe { img_type.assume_init() };

    match binning {
        1 => (),
        2 => {
            img_width /= 2;
            img_height /= 2;
        },
        _ => return Err(ASIError::UnknownBinningMethod(binning).into())
    }

    Ok((img_width, img_height, img_type))
}
