//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Camera simulator.
//!

use crate::camera::*;
use crate::resources;
use ga_image;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

mod control_ids {
    pub const IMAGE_SHOWN: u64 = 1;
    pub const DUMMY_1: u64 = 2;
    pub const DUMMY_2: u64 = 3;
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
        let image_rgb8 = resources::load_sim_image(resources::SimulatorImage::Landscape).unwrap();
        let image_mono8 = image_rgb8.convert_pix_fmt(ga_image::PixelFormat::Mono8, None);
        let image_cfa8 = rgb8_to_cfa8(&image_rgb8);

        let image_shown = Arc::new(RwLock::new(image_rgb8.clone()));

        Ok(Box::new(SimCamera{
            image_rgb8,
            image_mono8,
            image_cfa8,
            image_shown,
            dummy1: RefCell::new(5.0)
        }))
    }
}

pub struct SimCamera {
    dummy1: RefCell<f64>,
    image_rgb8: ga_image::Image,
    image_mono8: ga_image::Image,
    image_cfa8: ga_image::Image,
    image_shown: Arc<RwLock<ga_image::Image>>
}

#[derive(strum_macros::EnumIter)]
enum ImageShown {
    LandscapeRGB8,
    LandscapeMono8,
    LandscapeCFA8
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
                access_mode: ControlAccessMode::WriteOnly,
                on_off_state: None,
                auto_state: None,
                requires_capture_pause: false
            },
            items: vec![
                "Landscape (RGB 8-bit)".to_string(),
                "Landscape (mono 8-bit)".to_string(),
                "Landscape (raw color 8-bit)".to_string(),
            ],
            current_idx: 0
        });

        let dummy_control_1 = CameraControl::Number(NumberControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::DUMMY_1),
                label: "Dummy Control 1".to_string(),
                access_mode: ControlAccessMode::ReadWrite,
                on_off_state: Some(true),
                auto_state: Some(true),
                requires_capture_pause: false
            },
            value: *self.dummy1.borrow(),
            min: 0.0,
            max: 10.0,
            step: 0.1,
            num_decimals: 1
        });

        let dummy_control_2 = CameraControl::List(ListControl{
            base: CameraControlBase{
                id: CameraControlId(control_ids::DUMMY_2),
                label: "Dummy Control 2".to_string(),
                access_mode: ControlAccessMode::WriteOnly,
                on_off_state: None,
                auto_state: None,
                requires_capture_pause: true
            },
            items: vec!["Value 1".to_string(), "Value 2".to_string()],
            current_idx: 0
        });

        Ok(vec![
            image_shown,
            dummy_control_1,
            dummy_control_2
        ])
    }

    fn create_capturer(&self) -> Result<Box<dyn FrameCapturer + Send>, CameraError> {
        Ok(Box::new(SimFrameCapturer{
            t_last_capture: std::time::Instant::now(),
            image: Arc::clone(&self.image_shown)
        }))
    }

    fn set_number_control(&self, id: CameraControlId, value: f64) -> Result<Vec<Notification>, CameraError> {
        match id.0 {
            control_ids::DUMMY_1 => {
                *self.dummy1.borrow_mut() = value;
                Ok(vec![])
            },

            _ => Err(SimulatorError::Internal).map_err(CameraError::SimulatorError)
        }
    }

    fn set_list_control(&mut self, id: CameraControlId, option_idx: usize) -> Result<Vec<Notification>, CameraError> {
        match id.0 {
            control_ids::IMAGE_SHOWN => {
                let new_value: ImageShown = ImageShown::iter().skip(option_idx).next().unwrap();
                match new_value {
                    ImageShown::LandscapeRGB8 => *self.image_shown.write().unwrap() = self.image_rgb8.clone(),
                    ImageShown::LandscapeMono8 => *self.image_shown.write().unwrap() = self.image_mono8.clone(),
                    ImageShown::LandscapeCFA8 => *self.image_shown.write().unwrap() = self.image_cfa8.clone(),
                }
            },

            _ => panic!("Invalid control ID.")
        }

        Ok(vec![])
    }

    fn get_number_control(&self, id: CameraControlId) -> Result<f64, CameraError> {
        match id.0 {
            control_ids::DUMMY_1 => Ok(*self.dummy1.borrow()),
            _ => Err(SimulatorError::Internal).map_err(CameraError::SimulatorError)
        }
    }

    fn get_list_control(&self, _id: CameraControlId) -> Result<usize, CameraError> {
        unimplemented!();
    }

    fn set_auto(&self, _id: CameraControlId, _state: bool) -> Result<Vec<Notification>, CameraError> {
        Ok(vec![])
    }

    fn set_on_off(&self, _id: CameraControlId, _state: bool) -> Result<Vec<Notification>, CameraError> {
        Ok(vec![])
    }

    fn set_roi(&mut self, _x0: u32, _y0: u32, _width: u32, _height: u32) -> Result<(), CameraError> {
        println!("Simulator: ROI not implemented yet.");
        Ok(())
    }

    fn unset_roi(&mut self) -> Result<(), CameraError> {
        println!("Simulator: ROI not implemented yet.");
        Ok(())
    }
}

pub struct SimFrameCapturer {
    t_last_capture: std::time::Instant,
    image: Arc<RwLock<ga_image::Image>>
}

impl FrameCapturer for SimFrameCapturer {
    fn pause(&mut self) {}

    fn resume(&mut self) {}

    fn capture_frame(&mut self, dest_image: &mut Image) -> Result<(), CameraError> {
        let t_between_frames = std::time::Duration::from_secs_f32(1.0/30.0);
        let t_elapsed = self.t_last_capture.elapsed();
        if t_elapsed < t_between_frames {
            std::thread::sleep(t_between_frames - t_elapsed);
        }

        *dest_image = self.image.read().unwrap().clone();

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
