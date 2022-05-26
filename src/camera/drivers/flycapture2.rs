//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! FlyCapture2 camera driver.
//!

use crate::camera::*;
use ga_image;
use ga_image::Image;
use libflycapture2_sys::*;
use std::collections::HashMap;
use std::sync::Arc;

const FALSE: BOOL = 0;
const TRUE: BOOL = 1;

macro_rules! checked_call {
    ($func_call:expr) => {
        match unsafe { $func_call } {
            fc2Error::FC2_ERROR_OK => (),
            error => return Err(CameraError::FlyCapture2Error(FlyCapture2Error::Internal(error)))
        }
    }
}

/// Additional ids to use beside the ones from `fc2PropertyType`.
mod control_ids {
    use libflycapture2_sys::fc2PropertyType;

    pub const VIDEO_MODE: u64 = fc2PropertyType::FC2_UNSPECIFIED_PROPERTY_TYPE as u64 + 1;

    /// Selects one of the fixed framerates from the `fc2FrameRate` enum
    /// (only for non-scalable = non-Format7 modes); a camera may also support `FC2_FRAME_RATE`,
    /// which can change the frame rate independently and with finer granularity.
    pub const FIXED_FRAME_RATE: u64 = fc2PropertyType::FC2_UNSPECIFIED_PROPERTY_TYPE as u64 + 2;

    /// Selects pixel format for the current video mode (non-Format7 modes have only one pixel format).
    pub const PIXEL_FORMAT: u64 = fc2PropertyType::FC2_UNSPECIFIED_PROPERTY_TYPE as u64 + 3;
}

// Captured frames may be "inconsistent" (have damaged contents), e.g., sometimes when using GigE cameras
// on Linux with its network stack (instead of PGR's Ethernet Filter Driver under Windows). Allow a few
// of them before reporting error.
const MAX_NUM_INCONSISTENT_FRAMES_TO_SKIP: usize = 0; //15;

//TODO: support 12-bit modes
fn to_pix_fmt(fc2_pix_fmt: (fc2PixelFormat, fc2BayerTileFormat)) -> Result<ga_image::PixelFormat, CameraError> {
    match fc2_pix_fmt {
        (fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8, _) => Ok(ga_image::PixelFormat::Mono8),

        (fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16, _) |
        (fc2PixelFormat::FC2_PIXEL_FORMAT_S_MONO16, _) => Ok(ga_image::PixelFormat::Mono16),

        // It is unknown why `fc2BayerTileFormat::FC2_BT_NONE` is sometimes set in a received image;
        // perhaps it is connected to image consistency errors frequent under Linux when using a GigE camera.
        // For now just treat it as BGGR.

        (fc2PixelFormat::FC2_PIXEL_FORMAT_RAW8, cfa) => match cfa {
            fc2BayerTileFormat::FC2_BT_BGGR | fc2BayerTileFormat::FC2_BT_NONE => Ok(ga_image::PixelFormat::CfaBGGR8),
            fc2BayerTileFormat::FC2_BT_GBRG => Ok(ga_image::PixelFormat::CfaGBRG8),
            fc2BayerTileFormat::FC2_BT_GRBG => Ok(ga_image::PixelFormat::CfaGRBG8),
            fc2BayerTileFormat::FC2_BT_RGGB => Ok(ga_image::PixelFormat::CfaRGGB8),
            _ => Err(FlyCapture2Error::UnsupportedPixelFormat(fc2_pix_fmt.0).into())
        },

        (fc2PixelFormat::FC2_PIXEL_FORMAT_RAW16, cfa) => match cfa {
            fc2BayerTileFormat::FC2_BT_BGGR | fc2BayerTileFormat::FC2_BT_NONE => Ok(ga_image::PixelFormat::CfaBGGR16),
            fc2BayerTileFormat::FC2_BT_GBRG => Ok(ga_image::PixelFormat::CfaGBRG16),
            fc2BayerTileFormat::FC2_BT_GRBG => Ok(ga_image::PixelFormat::CfaGRBG16),
            fc2BayerTileFormat::FC2_BT_RGGB => Ok(ga_image::PixelFormat::CfaRGGB16),
            _ => Err(FlyCapture2Error::UnsupportedPixelFormat(fc2_pix_fmt.0).into())
        },

        _ => Err(FlyCapture2Error::UnsupportedPixelFormat(fc2_pix_fmt.0).into())
    }
}

fn video_mode_description(mode: fc2VideoMode) -> &'static str {
    match mode {
        fc2VideoMode::FC2_VIDEOMODE_160x120YUV444   => "160x120 YUV444",
        fc2VideoMode::FC2_VIDEOMODE_320x240YUV422   => "320x240 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_640x480YUV411   => "640x480 YUV411",
        fc2VideoMode::FC2_VIDEOMODE_640x480YUV422   => "640x480 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_640x480RGB      => "640x480 RGB 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_640x480Y8       => "640x480 Mono 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_640x480Y16      => "640x480 Mono 16-bit",
        fc2VideoMode::FC2_VIDEOMODE_800x600YUV422   => "800x600 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_800x600RGB      => "800x600 RGB 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_800x600Y8       => "800x600 Mono 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_800x600Y16      => "800x600 Mono 16-bit",
        fc2VideoMode::FC2_VIDEOMODE_1024x768YUV422  => "1024x768 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_1024x768RGB     => "1024x768 RGB 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1024x768Y8      => "1024x768 Mono 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1024x768Y16     => "1024x768 Mono 16-bit",
        fc2VideoMode::FC2_VIDEOMODE_1280x960YUV422  => "1280x960 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_1280x960RGB     => "1280x960 RGB 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1280x960Y8      => "1280x960 Mono 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1280x960Y16     => "1280x960 Mono 16-bit",
        fc2VideoMode::FC2_VIDEOMODE_1600x1200YUV422 => "1600x1200 YUV422",
        fc2VideoMode::FC2_VIDEOMODE_1600x1200RGB    => "1600x1200 RGB 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1600x1200Y8     => "1600x1200 Mono 8-bit",
        fc2VideoMode::FC2_VIDEOMODE_1600x1200Y16    => "1600x1200 Mono 16-bit",
        _ => panic!("Invalid video mode: {}", mode as u32)
    }
}

fn pixel_format_from_video_mode(mode: fc2VideoMode) -> fc2PixelFormat {
    match mode {
        fc2VideoMode::FC2_VIDEOMODE_160x120YUV444   => fc2PixelFormat::FC2_PIXEL_FORMAT_444YUV8,
        fc2VideoMode::FC2_VIDEOMODE_320x240YUV422   => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_640x480YUV411   => fc2PixelFormat::FC2_PIXEL_FORMAT_411YUV8,
        fc2VideoMode::FC2_VIDEOMODE_640x480YUV422   => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_640x480RGB      => fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8,
        fc2VideoMode::FC2_VIDEOMODE_640x480Y8       => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2VideoMode::FC2_VIDEOMODE_640x480Y16      => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,
        fc2VideoMode::FC2_VIDEOMODE_800x600YUV422   => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_800x600RGB      => fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8,
        fc2VideoMode::FC2_VIDEOMODE_800x600Y8       => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2VideoMode::FC2_VIDEOMODE_800x600Y16      => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,
        fc2VideoMode::FC2_VIDEOMODE_1024x768YUV422  => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_1024x768RGB     => fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8,
        fc2VideoMode::FC2_VIDEOMODE_1024x768Y8      => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2VideoMode::FC2_VIDEOMODE_1024x768Y16     => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,
        fc2VideoMode::FC2_VIDEOMODE_1280x960YUV422  => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_1280x960RGB     => fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8,
        fc2VideoMode::FC2_VIDEOMODE_1280x960Y8      => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2VideoMode::FC2_VIDEOMODE_1280x960Y16     => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,
        fc2VideoMode::FC2_VIDEOMODE_1600x1200YUV422 => fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8,
        fc2VideoMode::FC2_VIDEOMODE_1600x1200RGB    => fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8,
        fc2VideoMode::FC2_VIDEOMODE_1600x1200Y8     => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2VideoMode::FC2_VIDEOMODE_1600x1200Y16    => fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,

        fc2VideoMode::FC2_VIDEOMODE_FORMAT7  => panic!("Expected non-Format7 video mode."),

        _ => panic!("Invalid video mode: {}", mode as u32)
    }
}

fn pixel_format_name(fc2_pixel_format: fc2PixelFormat) -> &'static str {
    match fc2_pixel_format {
        fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8        => "Mono 8-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_411YUV8      => "YUV411",
        fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8      => "YUV422",
        fc2PixelFormat::FC2_PIXEL_FORMAT_444YUV8      => "YUV444",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RGB8         => "RGB 8-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16       => "Mono 16-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RGB16        => "RGB 16-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_S_MONO16     => "Mono 16-bit (signed)",
        fc2PixelFormat::FC2_PIXEL_FORMAT_S_RGB16      => "RGB 16-bit (signed)",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RAW8         => "RAW 8-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RAW16        => "RAW 16-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_MONO12       => "Mono 12-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RAW12        => "RAW 12-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_BGR          => "BGR",
        fc2PixelFormat::FC2_PIXEL_FORMAT_BGRU         => "BGRU",
        fc2PixelFormat::FC2_PIXEL_FORMAT_RGBU         => "RGBU",
        fc2PixelFormat::FC2_PIXEL_FORMAT_BGR16        => "BGR 16-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_BGRU16       => "BGRU 16-bit",
        fc2PixelFormat::FC2_PIXEL_FORMAT_422YUV8_JPEG => "JPEG YUV422",
        _ => panic!("Invalid FC2 pixel format: {}", fc2_pixel_format as u32)
    }
}

fn frame_rate_name(fr: fc2FrameRate) -> &'static str {
    match fr {
        fc2FrameRate::FC2_FRAMERATE_1_875 => "1.875 fps",
        fc2FrameRate::FC2_FRAMERATE_3_75 => "3.75 fps",
        fc2FrameRate::FC2_FRAMERATE_7_5 => "7.5 fps",
        fc2FrameRate::FC2_FRAMERATE_15 => "15 fps",
        fc2FrameRate::FC2_FRAMERATE_30 => "30 fps",
        fc2FrameRate::FC2_FRAMERATE_60 => "60 fps",
        fc2FrameRate::FC2_FRAMERATE_120 => "120 fps",
        fc2FrameRate::FC2_FRAMERATE_240 => "240 fps",
        _ => panic!("Invalid frame rate: {}", fr as u32)
    }
}

fn as_video_mode(mode: u32) -> fc2VideoMode {
    match mode {
        0 => fc2VideoMode::FC2_VIDEOMODE_160x120YUV444,
        1 => fc2VideoMode::FC2_VIDEOMODE_320x240YUV422,
        2 => fc2VideoMode::FC2_VIDEOMODE_640x480YUV411,
        3 => fc2VideoMode::FC2_VIDEOMODE_640x480YUV422,
        4 => fc2VideoMode::FC2_VIDEOMODE_640x480RGB,
        5 => fc2VideoMode::FC2_VIDEOMODE_640x480Y8,
        6 => fc2VideoMode::FC2_VIDEOMODE_640x480Y16,
        7 => fc2VideoMode::FC2_VIDEOMODE_800x600YUV422,
        8 => fc2VideoMode::FC2_VIDEOMODE_800x600RGB,
        9 => fc2VideoMode::FC2_VIDEOMODE_800x600Y8,
       10 => fc2VideoMode::FC2_VIDEOMODE_800x600Y16,
       11 => fc2VideoMode::FC2_VIDEOMODE_1024x768YUV422,
       12 => fc2VideoMode::FC2_VIDEOMODE_1024x768RGB,
       13 => fc2VideoMode::FC2_VIDEOMODE_1024x768Y8,
       14 => fc2VideoMode::FC2_VIDEOMODE_1024x768Y16,
       15 => fc2VideoMode::FC2_VIDEOMODE_1280x960YUV422,
       16 => fc2VideoMode::FC2_VIDEOMODE_1280x960RGB,
       17 => fc2VideoMode::FC2_VIDEOMODE_1280x960Y8,
       18 => fc2VideoMode::FC2_VIDEOMODE_1280x960Y16,
       19 => fc2VideoMode::FC2_VIDEOMODE_1600x1200YUV422,
       20 => fc2VideoMode::FC2_VIDEOMODE_1600x1200RGB,
       21 => fc2VideoMode::FC2_VIDEOMODE_1600x1200Y8,
       22 => fc2VideoMode::FC2_VIDEOMODE_1600x1200Y16,
       _ => panic!("Invalid video mode: {}", mode)
    }
}

fn as_frame_rate(frame_rate: u32) -> fc2FrameRate {
    match frame_rate {
        0 => fc2FrameRate::FC2_FRAMERATE_1_875,
        1 => fc2FrameRate::FC2_FRAMERATE_3_75,
        2 => fc2FrameRate::FC2_FRAMERATE_7_5,
        3 => fc2FrameRate::FC2_FRAMERATE_15,
        4 => fc2FrameRate::FC2_FRAMERATE_30,
        5 => fc2FrameRate::FC2_FRAMERATE_60,
        6 => fc2FrameRate::FC2_FRAMERATE_120,
        7 => fc2FrameRate::FC2_FRAMERATE_240,
        _ => panic!("Invalid frame rate: {}", frame_rate)
    }
}

fn as_fmt7_mode(mode: u32) -> fc2Mode {
    match mode {
        0 => fc2Mode::FC2_MODE_0,
        1 => fc2Mode::FC2_MODE_1,
        2 => fc2Mode::FC2_MODE_2,
        3 => fc2Mode::FC2_MODE_3,
        4 => fc2Mode::FC2_MODE_4,
        5 => fc2Mode::FC2_MODE_5,
        6 => fc2Mode::FC2_MODE_6,
        7 => fc2Mode::FC2_MODE_7,
        8 => fc2Mode::FC2_MODE_8,
        9 => fc2Mode::FC2_MODE_9,
        10 => fc2Mode::FC2_MODE_10,
        11 => fc2Mode::FC2_MODE_11,
        12 => fc2Mode::FC2_MODE_12,
        13 => fc2Mode::FC2_MODE_13,
        14 => fc2Mode::FC2_MODE_14,
        15 => fc2Mode::FC2_MODE_15,
        16 => fc2Mode::FC2_MODE_16,
        17 => fc2Mode::FC2_MODE_17,
        18 => fc2Mode::FC2_MODE_18,
        19 => fc2Mode::FC2_MODE_19,
        20 => fc2Mode::FC2_MODE_20,
        21 => fc2Mode::FC2_MODE_21,
        22 => fc2Mode::FC2_MODE_22,
        23 => fc2Mode::FC2_MODE_23,
        24 => fc2Mode::FC2_MODE_24,
        25 => fc2Mode::FC2_MODE_25,
        26 => fc2Mode::FC2_MODE_26,
        27 => fc2Mode::FC2_MODE_27,
        28 => fc2Mode::FC2_MODE_28,
        29 => fc2Mode::FC2_MODE_29,
        30 => fc2Mode::FC2_MODE_30,
        31 => fc2Mode::FC2_MODE_31,
        _ => panic!("Invalid Format7 mode: {}", mode)
    }
}

fn as_property_type(prop: u32) -> fc2PropertyType {
    match prop {
        0 => fc2PropertyType::FC2_BRIGHTNESS,
        1 => fc2PropertyType::FC2_AUTO_EXPOSURE,
        2 => fc2PropertyType::FC2_SHARPNESS,
        3 => fc2PropertyType::FC2_WHITE_BALANCE,
        4 => fc2PropertyType::FC2_HUE,
        5 => fc2PropertyType::FC2_SATURATION,
        6 => fc2PropertyType::FC2_GAMMA,
        7 => fc2PropertyType::FC2_IRIS,
        8 => fc2PropertyType::FC2_FOCUS,
        9 => fc2PropertyType::FC2_ZOOM,
        10 => fc2PropertyType::FC2_PAN,
        11 => fc2PropertyType::FC2_TILT,
        12 => fc2PropertyType::FC2_SHUTTER,
        13 => fc2PropertyType::FC2_GAIN,
        14 => fc2PropertyType::FC2_TRIGGER_MODE,
        15 => fc2PropertyType::FC2_TRIGGER_DELAY,
        16 => fc2PropertyType::FC2_FRAME_RATE,
        17 => fc2PropertyType::FC2_TEMPERATURE,

        _ => panic!("Invalid property type: {}", prop)
    }
}

impl From<FlyCapture2Error> for CameraError {
    fn from(fc2_error: FlyCapture2Error) -> CameraError {
        CameraError::FlyCapture2Error(fc2_error)
    }
}

#[derive(Debug)]
pub enum FlyCapture2Error {
    Internal(_fc2Error),
    ContextCreationFailed,
    UnsupportedPixelFormat(fc2PixelFormat)
}

struct Context {
    handle: fc2Context,
    connected: bool
}

unsafe impl Sync for Context {}

impl Context {
    fn new() -> Result<Context, CameraError> {
        let mut handle = std::ptr::null_mut();

        checked_call!(fc2CreateContext(&mut handle));

        if handle.is_null() {
            Err(FlyCapture2Error::ContextCreationFailed).map_err(CameraError::FlyCapture2Error)
        } else {
            Ok(Context{ handle, connected: false })
        }
    }

    fn connect(&mut self, guid: &fc2PGRGuid) -> Result<(), CameraError> {
        let mut local_guid = *guid;
        checked_call!(fc2Connect(self.handle, &mut local_guid));
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), CameraError> {
        checked_call!(fc2Disconnect(self.handle));
        self.connected = false;
        Ok(())
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if self.connected { let _ = self.disconnect(); }
        unsafe { fc2DestroyContext(self.handle); }
    }
}

pub struct FlyCapture2Driver {
    camera_guids: Vec<fc2PGRGuid>
}

fn camera_name_by_guid(context: &mut Context, guid: fc2PGRGuid) -> Result<String, CameraError> {
    context.connect(&guid)?;

    let mut cam_info: fc2CameraInfo = unsafe { std::mem::zeroed() };
    checked_call!(fc2GetCameraInfo(context.handle, &mut cam_info));

    context.disconnect()?;

    Ok(std::str::from_utf8(unsafe { &*(&cam_info.modelName as *const [i8] as *const [u8]) }).unwrap().trim_end_matches(char::from(0)).to_string())
}

impl FlyCapture2Driver {
    pub fn new() -> Result<FlyCapture2Driver, CameraError> {
        let mut version: fc2Version = unsafe { std::mem::zeroed() };
        checked_call!(fc2GetLibraryVersion(&mut version));
        println!("FlyCapture2 version: {}.{}.{}", version.major, version.minor, version.build);
        Ok(FlyCapture2Driver{ camera_guids: vec![] })
    }
}

impl Driver for FlyCapture2Driver {
    fn name(&self) -> &'static str { "Flycap2" }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        let mut context = Context::new()?;

        let mut num_cameras = 0;
        checked_call!(fc2GetNumOfCameras(context.handle, &mut num_cameras));

        let mut guids = vec![];
        let mut cameras = vec![];
        for i in 0..num_cameras {
            let mut guid: fc2PGRGuid = unsafe { std::mem::zeroed() };
            checked_call!(fc2GetCameraFromIndex(context.handle, i, &mut guid));
            guids.push(guid);
            cameras.push(CameraInfo{
                id: CameraId{ id1: i as u64, id2: 0 },
                name: camera_name_by_guid(&mut context, guid)?
            });
        }

        self.camera_guids = guids;

        Ok(cameras)
    }

    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        let mut context = Context::new()?;
        context.connect(&self.camera_guids[id.id1 as usize])?;

        let mut cam_info: fc2CameraInfo = unsafe { std::mem::zeroed() };
        checked_call!(fc2GetCameraInfo(context.handle, &mut cam_info));
        let name = std::str::from_utf8(unsafe { &*(&cam_info.modelName as *const [i8] as *const [u8]) }).unwrap().trim_end_matches(char::from(0)).to_string();

        let mut prop_info: fc2PropertyInfo = unsafe { std::mem::zeroed() };
        prop_info.type_ = fc2PropertyType::FC2_TEMPERATURE;
        checked_call!(fc2GetPropertyInfo(context.handle, &mut prop_info));
        let temperature_available = prop_info.present == TRUE;

        let mut frame_rates: HashMap<u32, Vec<fc2FrameRate>> = HashMap::new();
        let mut video_modes: Vec<FC2VideoModeEnum> = vec![];
        let mut fmt7_info: HashMap<u32, fc2Format7Info> = HashMap::new();

        for vid_mode in 0..fc2VideoMode::FC2_VIDEOMODE_FORMAT7 as u32 {
            let mut added = false;
            for frame_rate in 0..fc2FrameRate::FC2_FRAMERATE_FORMAT7 as u32 {
                let mut supported = FALSE;
                checked_call!(fc2GetVideoModeAndFrameRateInfo(
                    context.handle,
                    as_video_mode(vid_mode),
                    as_frame_rate(frame_rate),
                    &mut supported
                ));
                if supported == TRUE {
                    frame_rates.entry(vid_mode).or_insert(vec![]).push(as_frame_rate(frame_rate));
                    added = true;
                }
            }

            if added {
                video_modes.push(FC2VideoModeEnum::NonFormat7(as_video_mode(vid_mode)));
            }
        }

        for fmt7_mode in fc2Mode::FC2_MODE_0 as u32..fc2Mode::FC2_NUM_MODES as u32 {
            let mut f7: fc2Format7Info = unsafe { std::mem::zeroed() };
            f7.mode = as_fmt7_mode(fmt7_mode);
            let mut supported = FALSE;
            checked_call!(fc2GetFormat7Info(context.handle, &mut f7, &mut supported));
            if supported == TRUE {
                video_modes.push(FC2VideoModeEnum::Format7(as_fmt7_mode(fmt7_mode)));
                fmt7_info.insert(fmt7_mode, f7);
            }
        }

        let mut vid_mode = std::mem::MaybeUninit::uninit();
        let mut frame_rate = std::mem::MaybeUninit::uninit();
        checked_call!(fc2GetVideoModeAndFrameRate(context.handle, vid_mode.as_mut_ptr(), frame_rate.as_mut_ptr()));
        let vid_mode = unsafe { vid_mode.assume_init() };

        let (current_vid_mode, current_pix_fmt) = match vid_mode {
             fc2VideoMode::FC2_VIDEOMODE_FORMAT7 => {
                let mut fmt7_settings = std::mem::MaybeUninit::uninit();
                let mut packet_size = 0;
                let mut percentage = 0.0;
                checked_call!(fc2GetFormat7Configuration(
                    context.handle, fmt7_settings.as_mut_ptr(), &mut packet_size, &mut percentage
                ));
                let fmt7_settings = unsafe { fmt7_settings.assume_init() };

                (FC2VideoModeEnum::Format7(fmt7_settings.mode), fmt7_settings.pixelFormat)
            },

            _ => (FC2VideoModeEnum::NonFormat7(vid_mode), pixel_format_from_video_mode(vid_mode))
        };

        // in case the camera has a pixel format enabled which we do not support
        to_pix_fmt((current_pix_fmt, fc2BayerTileFormat::FC2_BT_NONE))?;

        checked_call!(fc2StartCapture(context.handle));

        Ok(Box::new(FlyCapture2Camera{
            context: Arc::new(context),
            id,
            name,
            temperature_available,
            video_modes,
            frame_rates,
            fmt7_info,
            current_vid_mode,
            current_pix_fmt,
            roi_offset: (0, 0),
            shutter_ctrl: None,
            fps_ctrl: None
        }))
    }
}

#[derive(Copy, Clone)]
enum FC2VideoModeEnum {
    NonFormat7(fc2VideoMode),
    Format7(fc2Mode)
}

impl PartialEq for FC2VideoModeEnum {
    fn eq(&self, other: &FC2VideoModeEnum) -> bool {
        match self {
            FC2VideoModeEnum::NonFormat7(lhs) => match other {
                FC2VideoModeEnum::NonFormat7(rhs) => *lhs as u32 == *rhs as u32,
                _ => false
            },
            FC2VideoModeEnum::Format7(lhs) => match other {
                FC2VideoModeEnum::Format7(rhs) => *lhs as u32 == *rhs as u32,
                _ => false
            }
        }
    }
}

pub struct FlyCapture2Camera {
    context: Arc<Context>,
    id: CameraId,
    name: String,
    temperature_available: bool,
    video_modes: Vec<FC2VideoModeEnum>,
    /// Fixed frame rates for non-Format7 video modes.
    frame_rates: HashMap<u32, Vec<fc2FrameRate>>,
    fmt7_info: HashMap<u32, fc2Format7Info>,
    current_vid_mode: FC2VideoModeEnum,
    current_pix_fmt: fc2PixelFormat,
    roi_offset: (u32, u32),
    /// Copy of `FC2_SHUTTER` control; used to notify the GUI about changes in value and range.
    shutter_ctrl: Option<NumberControl>,
    /// Copy of `FC2_FRAME_RATE` control; used to notify the GUI about changes in value and range.
    fps_ctrl: Option<NumberControl>
}

impl Drop for FlyCapture2Camera {
    fn drop(&mut self) {
        unsafe { fc2StopCapture(self.context.handle) };
    }
}

impl FlyCapture2Camera {
    fn create_fixed_frame_rate_control_for_video_mode(
        &self, vid_mode: fc2VideoMode
    ) -> Result<ListControl, CameraError> {
        let mut dummy = std::mem::MaybeUninit::uninit();
        let mut frame_rate = std::mem::MaybeUninit::uninit();
        checked_call!(fc2GetVideoModeAndFrameRate(self.context.handle, dummy.as_mut_ptr(), frame_rate.as_mut_ptr()));
        let frame_rate = unsafe { frame_rate.assume_init() };

        let mut items = vec![];
        let mut current_idx: Option<usize> = None;
        for (idx, fr) in self.frame_rates[&(vid_mode as u32)].iter().enumerate() {
            items.push(frame_rate_name(*fr).to_string());
            if *fr as u32 == frame_rate as u32 {
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

    fn create_video_mode_control(&self) -> Result<ListControl, CameraError> {
        let mut items: Vec<String> = vec![];

        for vid_mode in &self.video_modes {
            match vid_mode {
                FC2VideoModeEnum::NonFormat7(mode) => {
                    items.push(video_mode_description(*mode).to_string());
                },

                FC2VideoModeEnum::Format7(mode) => {
                    let f7 = self.fmt7_info.get(&(*mode as u32)).unwrap();
                    items.push(format!(
                        "{}x{} (FMT7: {})", f7.maxWidth, f7.maxHeight, *mode as u32 - fc2Mode::FC2_MODE_0 as u32
                    ));
                }
            }
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

    fn create_pixel_format_control_for_video_mode(
        &self,
        vid_mode: FC2VideoModeEnum
    ) -> Result<ListControl, CameraError> {
        match vid_mode {
            FC2VideoModeEnum::Format7(vid_mode) => {
                let fmt7 = &self.fmt7_info[&(vid_mode as u32)];
                let mut format_names = vec![];
                let mut current_idx = 0;

                for (i, pix_fmt) in get_all_supported_pixel_formats(fmt7.pixelFormatBitField).iter().enumerate() {
                    format_names.push(pixel_format_name(*pix_fmt).to_string());

                    if self.current_pix_fmt as u32 == *pix_fmt as u32 {
                        current_idx = i;
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

            },

            FC2VideoModeEnum::NonFormat7(vid_mode) => {
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
                    items: vec![pixel_format_name(pixel_format_from_video_mode(vid_mode)).to_string()],
                    current_idx: 0
                })
            }
        }
    }
}

impl Camera for FlyCapture2Camera {
    fn id(&self) -> CameraId { self.id }

    fn temperature(&self) -> Option<f64> {
        if !self.temperature_available { return None; }

        let mut prop: fc2Property = unsafe { std::mem::zeroed() };
        prop.type_ = fc2PropertyType::FC2_TEMPERATURE;
        match unsafe { fc2GetProperty(self.context.handle, &mut prop) } {
            fc2Error::FC2_ERROR_OK => {
                // Calculation as in CamSettingsPage.cpp from FlyCapture2 SDK. Strangely, both "absolute capable"
                // and "unit abbreviation" fields are not taken into account (e.g., on Chameleon3 CM3-U3-13S2M
                // the indicated unit is Celsius; yet, the formula below must be used anyway).
                let temp_celsius = prop.valueA as f64 / 10.0 - 273.15;

                Some(temp_celsius)
            },

            _ => None
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        let mut controls = vec![
            CameraControl::List(self.create_video_mode_control()?),
            CameraControl::List(self.create_pixel_format_control_for_video_mode(self.current_vid_mode)?)
        ];

        match self.current_vid_mode {
            FC2VideoModeEnum::NonFormat7(mode) =>
                controls.push(CameraControl::List(self.create_fixed_frame_rate_control_for_video_mode(mode)?)),
            _ => ()
        }

        for i in fc2PropertyType::FC2_BRIGHTNESS as u32..fc2PropertyType::FC2_UNSPECIFIED_PROPERTY_TYPE as u32 {
            let mut prop_info: fc2PropertyInfo = unsafe { std::mem::zeroed() };
            prop_info.type_ = as_property_type(i);
            checked_call!(fc2GetPropertyInfo(self.context.handle, &mut prop_info));
            if prop_info.present == FALSE { continue; }

            let mut prop: fc2Property = unsafe { std::mem::zeroed() };
            prop.type_ = as_property_type(i);
            checked_call!(fc2GetProperty(self.context.handle, &mut prop));

            let label = match as_property_type(i) {
                fc2PropertyType::FC2_BRIGHTNESS => "Brightness",
                fc2PropertyType::FC2_AUTO_EXPOSURE => "Exposure",
                fc2PropertyType::FC2_SHARPNESS => "Sharpness",

                //TODO: handle this
                // fc2PropertyType::FC2_WHITE_BALANCE => "White Balance",

                fc2PropertyType::FC2_HUE => "Hue",
                fc2PropertyType::FC2_SATURATION => "Saturation",
                fc2PropertyType::FC2_GAMMA => "Gamma",
                fc2PropertyType::FC2_IRIS => "Iris",
                fc2PropertyType::FC2_FOCUS => "Focus",
                fc2PropertyType::FC2_ZOOM => "Zoom",
                fc2PropertyType::FC2_PAN => "Pan",
                fc2PropertyType::FC2_TILT => "Tilt",
                fc2PropertyType::FC2_SHUTTER => "Shutter",
                fc2PropertyType::FC2_GAIN => "Gain",
                fc2PropertyType::FC2_FRAME_RATE => "Frame Rate",

                _ => { continue; }
            }.to_string();

            let supports_auto = prop_info.autoSupported == TRUE;

            let is_auto = prop.autoManualMode == TRUE;

            let read_only = prop_info.manualSupported == FALSE && prop_info.manualSupported == TRUE;

            let supports_on_off = prop_info.onOffSupported == TRUE;

            let is_on = prop.onOff == TRUE;

            let (mut raw_min, mut raw_max) = (prop_info.min as u32, prop_info.max as u32);
            if raw_min > raw_max { std::mem::swap(&mut raw_min, &mut raw_max); }

            let mut abs_min = prop_info.absMin as f64;
            let mut abs_max = prop_info.absMax as f64;
            if abs_min > abs_max { std::mem::swap(&mut abs_min, &mut abs_max); }

            let value = if prop_info.absValSupported == TRUE {
                prop.absControl = TRUE;
                checked_call!(fc2SetProperty(self.context.handle, &mut prop));
                prop.absValue as f64
            } else {
                prop.valueA as f64
            };

            let (min, max, step) = {
                let min;
                let max;
                let step;
                if prop_info.absValSupported == TRUE {
                    if raw_max != raw_min {
                        step = (abs_max - abs_min) as f64 / (raw_max - raw_min + 1) as f64;
                    } else {
                        step = 0.0;
                    }

                    min = abs_min;
                    max = abs_max;
                }
                else
                {
                    min = raw_min as f64;
                    max = raw_max as f64;
                    step = if raw_min != raw_max { 1.0 } else { 0.0 };
                }

                (min, max, step)
            };

            let num_decimals = if prop_info.absValSupported == TRUE && step < 1.0 {
                -step.log10() as usize + 2
            } else {
                0
            };

            let number_control = NumberControl{
                base: CameraControlBase{
                    id: CameraControlId(i as u64),
                    label,
                    refreshable:
                        as_property_type(i) as u32 == fc2PropertyType::FC2_SHUTTER as u32 ||
                        as_property_type(i) as u32 == fc2PropertyType::FC2_GAIN as u32,
                    access_mode: if read_only {
                        ControlAccessMode::ReadOnly
                    } else if prop_info.readOutSupported == TRUE {
                        ControlAccessMode::ReadWrite
                    } else {
                        ControlAccessMode::WriteOnly
                    },
                    on_off_state: if supports_on_off { Some(is_on) } else { None },
                    auto_state: if supports_auto { Some(is_auto) } else { None },
                    requires_capture_pause: false
                },
                value,
                min,
                max,
                step,
                num_decimals,
                is_exposure_time: as_property_type(i) as u32 == fc2PropertyType::FC2_SHUTTER as u32
            };

            if i == fc2PropertyType::FC2_SHUTTER as u32 {
                self.shutter_ctrl = Some(number_control.clone());
            } else if i == fc2PropertyType::FC2_FRAME_RATE as u32 {
                self.fps_ctrl = Some(number_control.clone());
            }

            controls.push(CameraControl::Number(number_control));
        }


        Ok(controls)
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        let mut fc2_image: fc2Image = unsafe { std::mem::zeroed() };
        checked_call!(fc2CreateImage(&mut fc2_image));

        // The returned frame capturer will share `context` (via `Arc`). FlyCapture2 allows using its functions
        // from multiple threads without additional synchronization. The `FlyCapture2Camera`'s instance will be used
        // by the main thread, and the `FlyCapture2FrameCapturer`'s instance - by the capture thread.

        Ok(Box::new(FlyCapture2FrameCapturer{ context: Arc::clone(&self.context), fc2_image }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError> {
        let mut prop: fc2Property = unsafe { std::mem::zeroed() };
        prop.type_ = as_property_type(id.0 as u32);
        checked_call!(fc2GetProperty(self.context.handle, &mut prop));
        if prop.absControl == TRUE {
            prop.absValue = value as f32;
        } else {
            prop.valueA = value as u32;
        }
        checked_call!(fc2SetProperty(self.context.handle, &mut prop));

        Ok(())
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError> {
        let context = self.context.handle;

        match id.0 {
            control_ids::VIDEO_MODE => {
                checked_call!(fc2StopCapture(context));

                let vid_mode = self.video_modes[option_idx];
                self.current_vid_mode = vid_mode;

                match vid_mode {
                    FC2VideoModeEnum::NonFormat7(mode) => {
                        checked_call!(fc2SetVideoModeAndFrameRate(
                            context,
                            mode,
                            // set the last (probably the highest) framerate
                            *self.frame_rates.get(&(mode as u32)).unwrap().last().unwrap()
                        ));
                    },

                    FC2VideoModeEnum::Format7(mode) => {
                        let f7_info = self.fmt7_info.get(&(mode as u32)).unwrap();

                        let mut f7_settings: fc2Format7ImageSettings = unsafe { std::mem::zeroed() };
                        f7_settings.mode = mode;
                        f7_settings.offsetX = 0;
                        f7_settings.offsetY = 0;
                        f7_settings.width = f7_info.maxWidth;
                        f7_settings.height = f7_info.maxHeight;
                        f7_settings.pixelFormat = get_first_supported_pixel_format(f7_info.pixelFormatBitField);

                        checked_call!(fc2SetFormat7Configuration(context, &mut f7_settings, 100.0));
                    }
                }

                checked_call!(fc2StartCapture(context));
            },

            control_ids::PIXEL_FORMAT => {
                checked_call!(fc2StopCapture(context));

                if let FC2VideoModeEnum::Format7(vid_mode) = self.current_vid_mode {
                    let f7_info = self.fmt7_info[&(vid_mode as u32)];

                    self.current_pix_fmt = get_all_supported_pixel_formats(f7_info.pixelFormatBitField)[option_idx];

                    let mut f7_settings: fc2Format7ImageSettings = unsafe { std::mem::zeroed() };
                    let mut packet_size = 0;
                    let mut percentage = 0.0;
                    checked_call!(fc2GetFormat7Configuration(
                        context, &mut f7_settings as *mut _, &mut packet_size, &mut percentage
                    ));
                    f7_settings.pixelFormat = self.current_pix_fmt;

                    checked_call!(fc2SetFormat7Configuration(context, &mut f7_settings, 100.0));
                } else {
                    unreachable!();
                }

                checked_call!(fc2StartCapture(context));
            },

            control_ids::FIXED_FRAME_RATE => {
                if let FC2VideoModeEnum::NonFormat7(mode) = self.current_vid_mode {
                    checked_call!(fc2SetVideoModeAndFrameRate(context, mode, self.frame_rates[&(mode as u32)][option_idx]));
                } else {
                    panic!("Cannot set fixed frame rate for Format7 video mode.");
                }
            },

            _ => panic!("Not implemented.")
        }

        Ok(())
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        let mut prop: fc2Property = unsafe { std::mem::zeroed() };
        prop.type_ = as_property_type(id.0 as u32);
        checked_call!(fc2GetProperty(self.context.handle, &mut prop));

        if prop.absControl == TRUE {
            Ok(prop.absValue as f64)
        } else {
            Ok(prop.valueA as f64)
        }
    }

    fn get_list_control(&self, _id: CameraControlId) -> Result<usize, CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_auto(&mut self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        let mut prop: fc2Property = unsafe { std::mem::zeroed() };
        prop.type_ = as_property_type(id.0 as u32);
        checked_call!(fc2GetProperty(self.context.handle, &mut prop));

        prop.autoManualMode = if state { TRUE } else { FALSE };
        checked_call!(fc2SetProperty(self.context.handle, &mut prop));

        Ok(())
    }

    fn set_on_off(&self, id: CameraControlId, state: bool) -> Result<(), CameraError> {
        let mut prop: fc2Property = unsafe { std::mem::zeroed() };
        prop.type_ = as_property_type(id.0 as u32);
        checked_call!(fc2GetProperty(self.context.handle, &mut prop));

        prop.onOff = if state { TRUE } else { FALSE };
        checked_call!(fc2SetProperty(self.context.handle, &mut prop));

        Ok(())
    }

    fn set_roi(&mut self, x0: u32, y0: u32, width: u32, height: u32) -> Result<(), CameraError> {
        if let FC2VideoModeEnum::Format7(mode) = self.current_vid_mode {
            checked_call!(fc2StopCapture(self.context.handle));

            let mut fmt7_settings = std::mem::MaybeUninit::uninit();
            let mut packet_size = 0;
            let mut percentage = 0.0;
            checked_call!(fc2GetFormat7Configuration(
                self.context.handle, fmt7_settings.as_mut_ptr(), &mut packet_size, &mut percentage
            ));
            let mut fmt7_settings = unsafe { fmt7_settings.assume_init() };

            let f7_info = self.fmt7_info.get(&(mode as u32)).unwrap();

            // rounds `x` down to the closest multiple of `n`
            macro_rules! downmult { ($x:expr, $n:expr) => { ($x) / ($n) * ($n) } }

            fmt7_settings.offsetX = downmult!((x0 + self.roi_offset.0) as std::os::raw::c_uint, f7_info.offsetHStepSize);
            fmt7_settings.offsetY = downmult!((y0 + self.roi_offset.1) as std::os::raw::c_uint, f7_info.offsetVStepSize);
            fmt7_settings.width   = downmult!(width  as std::os::raw::c_uint, f7_info.imageHStepSize);
            fmt7_settings.height  = downmult!(height as std::os::raw::c_uint, f7_info.imageVStepSize);
            checked_call!(fc2SetFormat7Configuration(self.context.handle, &mut fmt7_settings, 100.0));

            self.roi_offset.0 += x0;
            self.roi_offset.1 += y0;

            checked_call!(fc2StartCapture(self.context.handle));
        }

        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        if let FC2VideoModeEnum::Format7(mode) = self.current_vid_mode {
            checked_call!(fc2StopCapture(self.context.handle));

            let mut fmt7_settings = std::mem::MaybeUninit::uninit();
            let mut packet_size = 0;
            let mut percentage = 0.0;
            checked_call!(fc2GetFormat7Configuration(
                self.context.handle, fmt7_settings.as_mut_ptr(), &mut packet_size, &mut percentage
            ));
            let mut fmt7_settings = unsafe { fmt7_settings.assume_init() };

            let f7_info = self.fmt7_info.get(&(mode as u32)).unwrap();

            fmt7_settings.offsetX = 0;
            fmt7_settings.offsetY = 0;
            fmt7_settings.width = f7_info.maxWidth;
            fmt7_settings.height = f7_info.maxHeight;
            checked_call!(fc2SetFormat7Configuration(self.context.handle, &mut fmt7_settings, 100.0));

            self.roi_offset = (0, 0);

            checked_call!(fc2StartCapture(self.context.handle));
        }

        Ok(())
    }

    fn set_boolean_control(&mut self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_boolean_control(&self, _id: CameraControlId) -> Result<bool, CameraError> {
        unimplemented!()
    }
}

pub struct FlyCapture2FrameCapturer {
    context: Arc<Context>,
    fc2_image: fc2Image
}

impl Drop for FlyCapture2FrameCapturer {
    fn drop(&mut self) {
        unsafe { fc2DestroyImage(&mut self.fc2_image) };
    }
}

unsafe impl Send for FlyCapture2FrameCapturer {}

impl FrameCapturer for FlyCapture2FrameCapturer {
    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        let mut inconsistent_counter = 0;
        loop {
            match unsafe { fc2RetrieveBuffer(self.context.handle, &mut self.fc2_image) } {
                fc2Error::FC2_ERROR_OK => break,
                fc2Error::FC2_ERROR_IMAGE_CONSISTENCY_ERROR =>
                    println!("WARNING: Inconsistent image captured, skipping."),
                result => return Err(CameraError::FlyCapture2Error(FlyCapture2Error::Internal(result)))
            }

            inconsistent_counter += 1;
            if inconsistent_counter > MAX_NUM_INCONSISTENT_FRAMES_TO_SKIP {
                break;
            }
        }

        let pix_fmt = to_pix_fmt((self.fc2_image.format, self.fc2_image.bayerFormat))?;

        let frame_pixels: &[u8] = unsafe { std::slice::from_raw_parts(
            self.fc2_image.pData,
            self.fc2_image.dataSize as usize
        ) };

        if dest_image.bytes_per_line() != self.fc2_image.stride as usize ||
            dest_image.width() != self.fc2_image.cols ||
            dest_image.height() != self.fc2_image.rows ||
            dest_image.pixel_format() != pix_fmt {

            *dest_image = Image::new_from_pixels(
                self.fc2_image.cols,
                self.fc2_image.rows,
                Some(self.fc2_image.stride as usize),
                pix_fmt,
                None,
                frame_pixels.to_vec()
            );
        } else {
            dest_image.raw_pixels_mut().copy_from_slice(frame_pixels);
        }

        Ok(())
    }

    fn pause(&mut self) -> Result<(), CameraError> { Ok(()) }

    fn resume(&mut self) -> Result<(), CameraError> { Ok(()) }
}

fn get_first_supported_pixel_format(mask: std::os::raw::c_uint) -> fc2PixelFormat {
    for pix_fmt in supported_pixel_formats() {
        if mask & pix_fmt as u32 != 0 {
            return pix_fmt;
        }
    }

    panic!("There are no supported pixel formats for the selected video mode.");
}

fn get_all_supported_pixel_formats(mask: std::os::raw::c_uint) -> Vec<fc2PixelFormat> {
    let mut result = vec![];
    for pix_fmt in supported_pixel_formats() {
        if mask & pix_fmt as u32 != 0 { result.push(pix_fmt); }
    }

    if result.is_empty() {
        panic!("There are no supported pixel formats for the selected video mode.");
    } else {
        result
    }
}

fn supported_pixel_formats() -> Vec<fc2PixelFormat> {
    vec![
        fc2PixelFormat::FC2_PIXEL_FORMAT_MONO8,
        fc2PixelFormat::FC2_PIXEL_FORMAT_RAW8,
        fc2PixelFormat::FC2_PIXEL_FORMAT_MONO16,
        fc2PixelFormat::FC2_PIXEL_FORMAT_RAW16
    ]
}
