//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Video4Linux2 camera driver.
//!

extern crate ioctl_rs as ioctl;

use crate::camera::*;
use ga_image::{Image, PixelFormat};
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;
use v4l2_sys;

const V4L2_DEVICE_LIST_PATH: &str = "/sys/class/video4linux";
const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
const V4L2_FRMIVAL_TYPE_DISCRETE: u32 = 1;
const V4L2_MEMORY_MMAP: u32 = 1;
const MAP_SHARED: std::os::raw::c_int = 1;

const NUM_CAPTURE_BUFFERS: usize = 20;

/// Produces a range of specified length.
macro_rules! range { ($start:expr, $len:expr) => { $start .. $start + $len } }

fn clamp(x: f32, min: f32, max: f32) -> f32 { if x < min { min } else if x > max { max } else { x } }

#[derive(Debug)]
pub enum V4L2Error {
    IO(std::io::Error),
    NoVideoModesFound,
    FailedToSetVideMode,
    Internal
}

impl From<std::io::Error> for CameraError {
    fn from(error: std::io::Error) -> CameraError {
        CameraError::V4L2Error(V4L2Error::IO(error))
    }
}

fn is_fourcc(value: u32, chars: &[u8; 4]) -> bool {
    value as u8 == chars[0] &&
    (value >> 8) as u8 == chars[1] &&
    (value >> 16) as u8 == chars[2] &&
    (value >> 24) as u8 == chars[3]
}

pub struct V4L2Driver {
    devices: Vec<String>,
    /// Elements correspond to `devices`.
    names: Vec<String>
}

impl V4L2Driver {
    pub fn new() -> Option<V4L2Driver> {
        Some(V4L2Driver{ devices: vec![], names: vec![] })
    }
}

impl Driver for V4L2Driver {
    fn name(&self) -> &'static str { "V4L2" }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        let dir = std::fs::read_dir(V4L2_DEVICE_LIST_PATH);
        if dir.is_err() { return Ok(vec![]); }
        let dir = dir.unwrap();

        let devices: Vec<String> = dir
            .filter(|e| e.is_ok())
            .map(|e| e.unwrap().path().file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        let mut cameras: Vec<CameraInfo> = vec![];

        self.names = vec![];

        for (i, device) in devices.iter().map(|d| "/dev/".to_string() + d).enumerate() {
            let mut caps = MaybeUninit::<v4l2_sys::v4l2_capability>::uninit();
            let file = std::fs::File::open(&device)?;
            let fd = file.as_raw_fd();
            if 0 == unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_QUERYCAP, caps.as_mut_ptr() as *mut _) } {
                let caps = unsafe { caps.assume_init() };
                self.devices.push(device.to_string());
                let name = format!(
                    "{} ({})",
                    std::str::from_utf8(&caps.card).unwrap().trim_end_matches(char::from(0)),
                    std::str::from_utf8(&caps.driver).unwrap().trim_end_matches(char::from(0))
                );
                self.names.push(name.clone());
                cameras.push(CameraInfo{ id: CameraId{ id1: i as u64, id2: 0 }, name });
            } else {
                println!("WARNING: Failed to read capabilities of V4L2 device {}.", device);
            }
        }

        Ok(cameras)
    }

    fn open_camera(&mut self, id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        let device_file = std::fs::OpenOptions::new().read(true).write(true).open(&self.devices[id.id1 as usize])?;
        let fd = device_file.as_raw_fd();

        let mut vid_modes: Vec<VideoMode> = vec![];
        let mut first_video_mode_set = false;

        let mut format_idx = 0;
        loop {
            let mut format_desc = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_fmtdesc>() };
            format_desc.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
            format_desc.index = format_idx;

            if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_ENUM_FMT, &mut format_desc) } {
                break;
            }

            // TODO: handle other formats
            if is_fourcc(format_desc.pixelformat, b"YUYV") {
                let mut frame_size = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_frmsizeenum>() };
                frame_size.type_ = V4L2_FRMIVAL_TYPE_DISCRETE;
                frame_size.pixel_format = format_desc.pixelformat;
                let mut fsize_idx = 0;
                loop {
                    frame_size.index = fsize_idx;
                    if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_ENUM_FRAMESIZES, &mut frame_size) } {
                        break;
                    }

                    let video_mode = VideoMode{
                        width: unsafe { frame_size.__bindgen_anon_1.discrete.width },
                        height: unsafe { frame_size.__bindgen_anon_1.discrete.height },
                        pixel_format: format_desc.pixelformat
                    };

                    if !first_video_mode_set {
                        let mut format = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_format>() };
                        format.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
                        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_G_FMT, &mut format) } {
                            return Err(CameraError::V4L2Error(V4L2Error::Internal));
                        }

                        format.fmt.pix.pixelformat = video_mode.pixel_format;
                        format.fmt.pix.width = video_mode.width;
                        format.fmt.pix.height = video_mode.height;

                        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_S_FMT, &format) } {
                            return Err(CameraError::V4L2Error(V4L2Error::FailedToSetVideMode));
                        }

                        first_video_mode_set = true;
                    }

                    vid_modes.push(video_mode);

                    fsize_idx += 1;
                }
            } else {
                println!(
                    "V4L2: Ignoring unsupported pixel format {}{}{}{}.",
                    (format_desc.pixelformat         & 0xFF) as u8 as char,
                    ((format_desc.pixelformat >>  8) & 0xFF) as u8 as char,
                    ((format_desc.pixelformat >> 16) & 0xFF) as u8 as char,
                    ((format_desc.pixelformat >> 24) & 0xFF) as u8 as char
                );
            }

            format_idx += 1;
        }

        if vid_modes.is_empty() {
            Err(CameraError::V4L2Error(V4L2Error::NoVideoModesFound))
        } else {
            Ok(Box::new(V4L2Camera{ id, device_file, fd, vid_modes, name: self.names[id.id1 as usize].clone() }))
        }
    }
}

struct VideoMode {
    width: u32,
    height: u32,
    pixel_format: u32
}

pub struct V4L2Camera {
    id: CameraId,
    device_file: std::fs::File,
    /// Raw file descriptor of `device_file`.
    fd: std::os::unix::io::RawFd,
    vid_modes: Vec<VideoMode>,
    name: String
}

impl Camera for V4L2Camera {
    fn id(&self) -> CameraId { self.id }

    fn name(&self) -> &str { &self.name }

    fn temperature(&self) -> Option<f64> { None }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        Ok(vec![])
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        match V4L2FrameCapturer::new(self.fd) {
            Some(capturer) => Ok(Box::new(capturer)),
            None => Err(CameraError::V4L2Error(V4L2Error::Internal))
        }
    }

    fn set_number_control(&self, _id: CameraControlId, _value: f64) -> Result<(), CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_list_control(&mut self, _id: CameraControlId, _option_idx: usize) -> Result<(), CameraError> {
        panic!("Not implemented yet.");
    }

    fn get_number_control(&self, _id: CameraControlId) -> Result<f64, CameraError> {
        panic!("Not implemented yet.");
    }

    fn get_list_control(&self, _id: CameraControlId) -> Result<usize, CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_auto(&self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_on_off(&self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_roi(&mut self, _x0: u32, _y0: u32, _width: u32, _height: u32) -> Result<(), CameraError> {
        Err(CameraError::UnableToSetROI("setting ROI is not supported".to_string()))
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        panic!("Not implemented yet.");
    }

    fn set_boolean_control(&mut self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_boolean_control(&self, _id: CameraControlId) -> Result<bool, CameraError> {
        unimplemented!()
    }
}

pub struct V4L2FrameCapturer {
    /// Raw file descriptor of `device_file` of the associated camera.
    fd: std::os::unix::io::RawFd,
    buffers: Vec<mmap::MemoryMap>,
    img_width: u32,
    img_height: u32,
    bytes_per_line: u32
}

unsafe impl Send for V4L2FrameCapturer {}

impl FrameCapturer for V4L2FrameCapturer {
    fn pause(&mut self)
    {
        panic!("Not implemented yet.");
    }

    fn resume(&mut self)
    {
        panic!("Not implemented yet.");
    }

    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        let mut capbuf = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_buffer>() };
        capbuf.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        capbuf.memory = V4L2_MEMORY_MMAP;
        if 0 != unsafe { ioctl::ioctl(self.fd, v4l2_sys::VIDIOC_DQBUF, &mut capbuf) } {
            return Err(CameraError::V4L2Error(V4L2Error::Internal));
        }

        if dest_image.width() != self.img_width ||
           dest_image.height() != self.img_height ||
           dest_image.pixel_format() != PixelFormat::RGB8 {

            *dest_image = Image::new(self.img_width, self.img_height, None, PixelFormat::RGB8, None, true);
        }

        let membuf = &self.buffers[capbuf.index as usize];
        let membuf_data: &[u8] = unsafe { std::slice::from_raw_parts(membuf.data(), membuf.len()) };

        for y in 0..self.img_height {
            let src_line = &membuf_data[range!((y * self.bytes_per_line) as usize, 2 * self.img_width as usize)];
            let dest_line = dest_image.line_mut::<u8>(y);

            //TODO: fill borders; use unchecked access
            for x in 1..self.img_width - 1 {
                let y = src_line[2 * x as usize] as f32;
                let u = if x & 1 == 1 { (src_line[2 * x as usize + 1 - 2] as u16 + src_line[2 * x as usize + 1 + 2] as u16) as f32 / 2.0 }
                    else { src_line[2 * x as usize + 1] as f32 };
                let v = if x & 1 == 1 { src_line[2 * x as usize + 1] as f32 }
                    else { (src_line[2 * x as usize + 1 - 2] as u16 + src_line[2 * x as usize + 1 + 2] as u16) as f32 / 2.0 };

                let b = clamp(1.164 * (y - 16.0) + 2.018 * (u - 128.0), 0.0, 255.0);
                let g = clamp(1.164 * (y - 16.0) - 0.813 * (v - 128.0) - 0.391 * (u - 128.0), 0.0, 255.0);
                let r = clamp(1.164 * (y - 16.0) + 1.596 * (v - 128.0), 0.0, 255.0);

                dest_line[3 * x as usize    ] = r as u8;
                dest_line[3 * x as usize + 1] = g as u8;
                dest_line[3 * x as usize + 2] = b as u8;
            }
        }

        if 0 != unsafe { ioctl::ioctl(self.fd, v4l2_sys::VIDIOC_QBUF, &capbuf) } {
            return Err(CameraError::V4L2Error(V4L2Error::Internal));
        }

        Ok(())
    }
}

impl V4L2FrameCapturer {
    fn new(fd: std::os::unix::io::RawFd) -> Option<V4L2FrameCapturer> {
        let buffers = match prepare_buffers(NUM_CAPTURE_BUFFERS, fd) {
            Some(buffers) => buffers,
            None => return None
        };

        let mut format = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_format>() };
        format.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_G_FMT, &mut format) } {
            return None;
        }

        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_STREAMON, &V4L2_BUF_TYPE_VIDEO_CAPTURE) } {
            return None;
        }

        let mut bytes_per_line = unsafe { format.fmt.pix.bytesperline };
        if bytes_per_line == 0 {
            bytes_per_line = unsafe { format.fmt.pix.width * 2 }; // valid only for YUYV
        }

        Some(V4L2FrameCapturer{
            fd,
            buffers,
            img_width: unsafe { format.fmt.pix.width },
            img_height: unsafe { format.fmt.pix.height },
            bytes_per_line
        })
    }
}

fn prepare_buffers(count: usize, fd: std::os::unix::io::RawFd) -> Option<Vec<mmap::MemoryMap>> {
    let mut req_buf = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_requestbuffers>() };
    req_buf.type_ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    req_buf.memory = V4L2_MEMORY_MMAP;
    req_buf.count = count as u32;

    if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_REQBUFS, &mut req_buf) } {
        return None;
    }

    let mut buffers = vec![];

    for i in 0..count {
        let mut buf = unsafe { std::mem::zeroed::<v4l2_sys::v4l2_buffer>() };
        buf.type_ = req_buf.type_;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i as u32;

        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_QUERYBUF, &mut buf) } {
            continue;
        }

        let buffer = mmap::MemoryMap::new(buf.length as usize, &[
            mmap::MapOption::MapReadable,
            mmap::MapOption::MapWritable,
            mmap::MapOption::MapNonStandardFlags(MAP_SHARED),
            mmap::MapOption::MapFd(fd),
            mmap::MapOption::MapOffset(unsafe { buf.m.offset } as usize)
        ]);
        if buffer.is_err() { return None; }

        if 0 != unsafe { ioctl::ioctl(fd, v4l2_sys::VIDIOC_QBUF, &buf) } {
            return None;
        }

        buffers.push(buffer.unwrap());
    }

    if buffers.is_empty() {
        None
    } else {
        Some(buffers)
    }
}
