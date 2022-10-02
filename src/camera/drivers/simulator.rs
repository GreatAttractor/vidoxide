//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Camera simulator.
//!

use cgmath::{InnerSpace, Vector2};
use crate::camera::*;
use crate::input;
use crate::resources;
use ga_image;
use std::cell::RefCell;
use std::sync::{Arc, RwLock, atomic::Ordering};
use strum::IntoEnumIterator;

mod control_ids {
    pub const IMAGE_SHOWN: u64 = 1;
    pub const DUMMY_1: u64 = 2;
    pub const DUMMY_2: u64 = 3;
    pub const FRAME_RATE: u64 = 4;
    pub const EXPOSURE_TIME: u64 = 5;
}

#[derive(Debug)]
pub enum SimulatorError {
    Internal
}

impl From<SimulatorError> for CameraError {
    fn from(sim_error: SimulatorError) -> CameraError {
        CameraError::SimulatorError(sim_error)
    }
}

pub struct SimDriver {
}

impl SimDriver {
    pub fn new() -> Option<SimDriver> {
        Some(SimDriver{})
    }
}

impl Driver for SimDriver {
    fn name(&self) -> &'static str { "Sim" }

    fn enumerate_cameras(&mut self) -> Result<Vec<CameraInfo>, CameraError> {
        Ok(vec![CameraInfo{ id: CameraId{ id1: 1, id2: 1 }, name: "Simulator".to_string() }])
    }

    fn open_camera(&mut self, _id: CameraId) -> Result<Box<dyn Camera>, CameraError> {
        Ok(Box::new(SimCamera{
            image_shown: ImageShown::LandscapeRGB8,
            new_img_seq: RefCell::new(None),
            dummy1: RefCell::new(5.0),
            frame_rate: Arc::new(RwLock::new(30.0)),
            exposure_time: RefCell::new(5.0),
            mount_simulator_data: crate::MountSimulatorData::default()
        }))
    }
}

pub struct SimCamera {
    image_shown: ImageShown,
    dummy1: RefCell<f64>,
    new_img_seq: RefCell<Option<crossbeam::channel::Sender<Box<dyn input::ImageSequence>>>>,
    frame_rate: Arc<RwLock<f64>>,
    exposure_time: RefCell<f64>,
    mount_simulator_data: crate::MountSimulatorData
}

#[derive(strum_macros::EnumIter)]
enum ImageShown {
    LandscapeRGB8,
    LandscapeMono8,
    LandscapeCFA8,
    Star1
}

impl Camera for SimCamera {
    fn id(&self) -> CameraId {
        CameraId{
            id1: 1,
            id2: 1
        }
    }

    fn temperature(&self) -> Option<f64> { Some(35.0) }

    fn name(&self) -> &str { "Simulator" }

    fn enumerate_controls(&mut self) -> Result<Vec<CameraControl>, CameraError> {
        let image_shown = CameraControl::List(ListControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::IMAGE_SHOWN),
                label: "Image shown".to_string(),
                refreshable: false,
                access_mode: ControlAccessMode::WriteOnly,
                on_off_state: None,
                auto_state: None,
                requires_capture_pause: false
            },
            items: vec![
                "Landscape (RGB 8-bit)".to_string(),
                "Landscape (mono 8-bit)".to_string(),
                "Landscape (raw color 8-bit)".to_string(),
                "Defocused star".to_string(),
            ],
            current_idx: 0
        });

        let dummy_exposure_time = CameraControl::Number(NumberControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::EXPOSURE_TIME),
                label: "Exposure time".to_string(),
                refreshable: true,
                access_mode: ControlAccessMode::ReadWrite,
                on_off_state: None,
                auto_state: Some(false),
                requires_capture_pause: false
            },
            value: *self.exposure_time.borrow(),
            min: 40.0e-6,
            max: 30.0,
            step: 1.0e-6,
            num_decimals: 6,
            is_exposure_time: true
        });

        let dummy_control_1 = CameraControl::Number(NumberControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::DUMMY_1),
                label: "Dummy Control 1".to_string(),
                refreshable: false,
                access_mode: ControlAccessMode::ReadWrite,
                on_off_state: Some(true),
                auto_state: Some(true),
                requires_capture_pause: false
            },
            value: *self.dummy1.borrow(),
            min: 0.0,
            max: 10.0,
            step: 0.1,
            num_decimals: 1,
            is_exposure_time: false
        });

        let dummy_control_2 = CameraControl::List(ListControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::DUMMY_2),
                label: "Dummy Control 2".to_string(),
                refreshable: false,
                access_mode: ControlAccessMode::WriteOnly,
                on_off_state: None,
                auto_state: None,
                requires_capture_pause: true
            },
            items: vec!["Value 1".to_string(), "Value 2".to_string()],
            current_idx: 0
        });

        let frame_rate = CameraControl::Number(NumberControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::FRAME_RATE),
                label: "Frame Rate".to_string(),
                refreshable: false,
                access_mode: ControlAccessMode::WriteOnly,
                on_off_state: None,
                auto_state: None,
                requires_capture_pause: false
            },
            value: 30.0,
            min: 1.0,
            max: 1000.0,
            step: 10.0,
            num_decimals: 0,
            is_exposure_time: false
        });

        Ok(vec![
            image_shown,
            frame_rate,
            dummy_exposure_time,
            dummy_control_1,
            dummy_control_2
        ])
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        let img_sequence = create_capturer_input(&self.image_shown);
        let (sender, receiver) = crossbeam::channel::unbounded();
        *self.new_img_seq.borrow_mut() = Some(sender);


        Ok(Box::new(SimFrameCapturer{
            t_last_capture: std::time::Instant::now(),
            img_index: 0,
            img_sequence,
            new_img_seq: receiver,
            frame_rate: Arc::clone(&self.frame_rate),
            mount_simulator_data: self.mount_simulator_data.clone(),
            img_offset: cgmath::Vector2::new(0.0, 0.0)
        }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<(), CameraError> {
        match id.0 {
            control_ids::DUMMY_1 => {
                *self.dummy1.borrow_mut() = value;
                Ok(())
            },

            control_ids::FRAME_RATE => {
                *self.frame_rate.write().unwrap() = value;
                Ok(())
            },

            control_ids::EXPOSURE_TIME => {
                *self.exposure_time.borrow_mut() = value;
                Ok(())
            }

            _ => Err(SimulatorError::Internal).map_err(CameraError::SimulatorError)
        }
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<(), CameraError> {
        match id.0 {
            control_ids::IMAGE_SHOWN => {
                self.image_shown = ImageShown::iter().skip(option_idx).next().unwrap();
                self.new_img_seq.borrow().as_ref().unwrap().send(create_capturer_input(&self.image_shown)).unwrap();
            },

            _ => ()
        }

        Ok(())
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        match id.0 {
            control_ids::DUMMY_1 => Ok(*self.dummy1.borrow()),
            control_ids::EXPOSURE_TIME => Ok(*self.exposure_time.borrow()),
            _ => Err(SimulatorError::Internal).map_err(CameraError::SimulatorError)
        }
    }

    fn get_list_control(&self, _id: CameraControlId) -> Result<usize, CameraError> {
        unimplemented!();
    }

    fn set_auto(&mut self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        Ok(())
    }

    fn set_on_off(&self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        Ok(())
    }

    fn set_roi(&mut self, _x0: u32, _y0: u32, _width: u32, _height: u32) -> Result<(), CameraError> {
        println!("Simulator: ROI not implemented yet.");
        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        println!("Simulator: ROI not implemented yet.");
        Ok(())
    }

    fn set_boolean_control(&mut self, _id: CameraControlId, _state: bool) -> Result<(), CameraError> {
        unimplemented!()
    }

    fn get_boolean_control(&self, _id: CameraControlId) -> Result<bool, CameraError> {
        unimplemented!()
    }

    fn set_mount_simulator_data(&mut self, data: crate::MountSimulatorData) {
        self.mount_simulator_data = data;
    }
}

pub struct SimFrameCapturer {
    t_last_capture: std::time::Instant,
    img_index: usize,
    img_sequence: Box<dyn input::ImageSequence>,
    frame_rate: Arc<RwLock<f64>>,
    mount_simulator_data: crate::MountSimulatorData,
    img_offset: cgmath::Vector2<f64>,
    new_img_seq: crossbeam::channel::Receiver<Box<dyn input::ImageSequence>>
}

impl FrameCapturer for SimFrameCapturer {
    fn pause(&mut self) -> Result<(), CameraError> { Ok(()) }

    fn resume(&mut self) -> Result<(), CameraError> { Ok(()) }

    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        match self.new_img_seq.try_recv() {
            Err(e) => if e != crossbeam::TryRecvError::Empty { panic!("unexpected receiver error {:?}.", e) },

            Ok(img_seq) => {
                self.img_index = 0;
                self.img_sequence = img_seq;
            }
        }


        let t_between_frames = std::time::Duration::from_secs_f64(1.0 / self.frame_rate.read().unwrap().clone());
        let t_elapsed = self.t_last_capture.elapsed();
        if t_elapsed < t_between_frames {
            std::thread::sleep(t_between_frames - t_elapsed);
        }
        let t_elapsed = self.t_last_capture.elapsed();

        let image = self.img_sequence.get_image(self.img_index).unwrap();
        self.img_index = (self.img_index + 1) % self.img_sequence.num_images();
        if dest_image.bytes_per_line() != image.bytes_per_line()
            || dest_image.width() != image.width()
            || dest_image.height() != image.height()
            || dest_image.pixel_format() != image.pixel_format() {

            *dest_image = ga_image::Image::new(
                image.width(),
                image.height(),
                Some(image.bytes_per_line()),
                image.pixel_format(),
                None,
                true
            );
        }

        let msd = &self.mount_simulator_data;

        let sky_rotation: Vector2<f64> = msd.sky_rotation_dir_in_img_space().cast::<f64>().unwrap().normalize() *
            msd.sky_rotation_speed_pix_per_sec() as f64;

        let primary_axis_slew_dir_in_img_space: Vector2<f64> =
            msd.primary_axis_slew_dir_in_img_space().cast::<f64>().unwrap().normalize();

        let secondary_axis_slew_dir_in_img_space = Vector2{
            x: -primary_axis_slew_dir_in_img_space.y,
            y: primary_axis_slew_dir_in_img_space.x
        };

        let mount_slew: Vector2<f64> =
            primary_axis_slew_dir_in_img_space * msd.primary_axis_speed.load(Ordering::Acquire) as f64 +
            secondary_axis_slew_dir_in_img_space * msd.secondary_axis_speed.load(Ordering::Acquire) as f64;

        self.img_offset = self.img_offset +
            t_elapsed.as_secs_f64() * (sky_rotation + mount_slew);

        image.resize_and_translate_into(
            dest_image,
            ga_image::point::Point{ x: 0, y: 0 },
            image.width(),
            image.height(),
            ga_image::point::Point{ x: self.img_offset.x as i32, y: self.img_offset.y as i32 },
            true
        );

        self.t_last_capture = std::time::Instant::now();

        Ok(())
    }
}

fn rgb8_to_cfa8(image: &ga_image::Image) -> ga_image::Image {
    assert!(image.pixel_format() == ga_image::PixelFormat::RGB8);
    let mut result = ga_image::Image::new(image.width(), image.height(), None, ga_image::PixelFormat::CfaGBRG8, None, true);

    const RED: usize = 0;
    const GREEN: usize = 1;
    const BLUE: usize = 2;

    for y in 0..image.height() {
        let src_line = image.line::<u8>(y);
        let dest_line = result.line_mut::<u8>(y);

        for x in 0..image.width() {
            if (x & 1 == 0) && (y & 1 == 0) || (x & 1 == 1) && (y & 1 == 1) {
                dest_line[x as usize] = src_line[(3*x) as usize + GREEN];
            } else if (x & 1 == 1) && (y & 1 == 0) {
                dest_line[x as usize] = src_line[(3*x) as usize + BLUE];
            } else if (x & 1 == 0) && (y & 1 == 1) {
                dest_line[x as usize] = src_line[(3*x) as usize + RED];
            }
        }
    }

    result
}

fn create_capturer_input(image_shown: &ImageShown) -> Box<dyn input::ImageSequence> {
    match image_shown {
        ImageShown::LandscapeRGB8 => {
            input::create_image_list(vec![resources::load_sim_image(resources::SimulatorImage::Landscape).unwrap()])
        },

        ImageShown::LandscapeMono8 => {
            input::create_image_list(vec![resources::load_sim_image(resources::SimulatorImage::Landscape).unwrap()
                .convert_pix_fmt(ga_image::PixelFormat::Mono8, None)])
        },

        ImageShown::LandscapeCFA8 => {
            let img = resources::load_sim_image(resources::SimulatorImage::Landscape).unwrap();
            input::create_image_list(vec![rgb8_to_cfa8(&img)])
        },

        ImageShown::Star1 => {
            input::create_image_list(vec![resources::load_sim_image(resources::SimulatorImage::Star1).unwrap()])
        }
    }
}
