//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
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
#[cfg(feature = "camera_spinnaker")]
pub mod spinnaker;
#[cfg(feature = "camera_asi")]
pub mod asi;

use crate::camera::Driver;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init_drivers<'a>(disabled_drivers: &[&str]) -> Vec<Rc<RefCell<Box<dyn Driver>>>> {
    let mut drivers: Vec<Rc<RefCell<Box<dyn Driver>>>> = vec![];

    #[cfg(feature = "camera_iidc")]
    if !disabled_drivers.contains(&"camera_iidc") {
        drivers.push(Rc::new(RefCell::new(Box::new(iidc::IIDCDriver::new().unwrap()))));
    }

    #[cfg(feature = "camera_v4l2")]
    if !disabled_drivers.contains(&"camera_v4l2") {
        drivers.push(Rc::new(RefCell::new(Box::new(v4l2::V4L2Driver::new().unwrap()))));
    }

    #[cfg(feature = "camera_flycap2")]
    if !disabled_drivers.contains(&"camera_flycap2") {
        drivers.push(Rc::new(RefCell::new(Box::new(flycapture2::FlyCapture2Driver::new().unwrap()))));
    }

    #[cfg(feature = "camera_spinnaker")]
    if !disabled_drivers.contains(&"camera_spinnaker") {
        drivers.push(Rc::new(RefCell::new(Box::new(spinnaker::SpinnakerDriver::new().unwrap()))));
    }

    #[cfg(feature = "camera_asi")]
    if !disabled_drivers.contains(&"camera_asi") {
        drivers.push(Rc::new(RefCell::new(Box::new(asi::ASIDriver::new().unwrap()))));
    }

    // add more drivers here

    if !disabled_drivers.contains(&"simulator") {
        drivers.push(Rc::new(RefCell::new(Box::new(simulator::SimDriver::new().unwrap()))));
    }

    drivers
}
