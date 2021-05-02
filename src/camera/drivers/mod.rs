//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Camera drivers.
//!

#[cfg(feature = "camera_flycap2")]
pub mod flycapture2;
#[cfg(feature = "camera_iidc")]
pub mod iidc;
pub mod simulator;
#[cfg(feature = "camera_v4l2")]
pub mod v4l2;

use crate::camera::Driver;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init_drivers<'a>() -> Vec<Rc<RefCell<Box<dyn Driver>>>> {
    vec![
        #[cfg(feature = "camera_iidc")]
        Rc::new(RefCell::new(Box::new(iidc::IIDCDriver::new().unwrap()))),
        Rc::new(RefCell::new(Box::new(simulator::SimDriver::new().unwrap()))),
        #[cfg(feature = "camera_v4l2")]
        Rc::new(RefCell::new(Box::new(v4l2::V4L2Driver::new().unwrap()))),
        #[cfg(feature = "camera_flycap2")]
        Rc::new(RefCell::new(Box::new(flycapture2::FlyCapture2Driver::new().unwrap()))),
        // add more drivers here
    ]
}
