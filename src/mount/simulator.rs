//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2021-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Mount simulator.
//!

use crate::mount::{Axis, Mount, RadPerSec, SIDEREAL_RATE};
use std::error::Error;
use std::sync::atomic::Ordering;

pub struct Simulator {
    tracking: bool,
    data: crate::MountSimulatorData
}

impl Simulator {
    pub fn new() -> Simulator {
        Simulator{
            tracking: false,
            data: Default::default()
        }
    }

    fn motion(&mut self, axis: Axis, speed: RadPerSec) {
        let speed_pix_per_sec = speed.0 / SIDEREAL_RATE.0 * self.data.sky_rotation_speed_pix_per_sec() as f64;

        match axis {
            Axis::Primary => self.data.primary_axis_speed.store(speed_pix_per_sec as f32, Ordering::Release),
            Axis::Secondary => self.data.secondary_axis_speed.store(speed_pix_per_sec as f32, Ordering::Release),
        }
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        self.data.mount_connected.store(false, Ordering::Release);
        self.stop();
    }
}

impl Mount for Simulator {
    fn get_info(&self) -> String {
        "Simulator".to_string()
    }

    fn set_mount_simulator_data(&mut self, mount_simulator_data: crate::MountSimulatorData) {
        self.data = mount_simulator_data;
        self.data.mount_connected.store(true, Ordering::Release);
    }

    fn set_tracking(&mut self, enabled: bool) -> Result<(), Box<dyn Error>> {
        self.tracking = enabled;
        self.motion(Axis::Primary, if enabled { SIDEREAL_RATE } else { RadPerSec(0.0) });
        Ok(())
    }

    fn guide(&mut self, axis1_speed: RadPerSec, axis2_speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        if !self.tracking { return Err("cannot guide when tracking disabled".into()); }

        self.motion(Axis::Primary, SIDEREAL_RATE + axis1_speed);
        self.motion(Axis::Secondary, axis2_speed);

        Ok(())
    }

    fn slew(&mut self, axis: Axis, speed: RadPerSec) -> Result<(), Box<dyn Error>> {
        match axis {
            Axis::Primary => self.motion(axis, speed + if self.tracking { SIDEREAL_RATE } else { RadPerSec(0.0) }),
            Axis::Secondary => self.motion(axis, speed)
        }

        Ok(())
    }

    fn slewing_rate_supported(&self, _: RadPerSec) -> bool {
        true
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.tracking = false;
        self.data.primary_axis_speed.store(0.0, Ordering::Release);
        self.data.secondary_axis_speed.store(0.0, Ordering::Release);
        Ok(())
    }
}
