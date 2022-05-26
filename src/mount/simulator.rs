use crate::mount::{Axis, Mount, MountError, SIDEREAL_RATE};
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct SimulatorError;

pub struct Simulator {
    data: crate::MountSimulatorData
}

impl std::convert::From<SimulatorError> for MountError {
    fn from(e: SimulatorError) -> MountError {
        MountError::SimulatorError(e)
    }
}

impl Simulator {
    pub fn new() -> Simulator {
        Simulator{
            data: Default::default()
        }
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        self.data.mount_connected.store(false, Ordering::Release);
        self.data.primary_axis_speed.store(0.0, Ordering::Relaxed);
        self.data.secondary_axis_speed.store(0.0, Ordering::Relaxed);
    }
}

impl Mount for Simulator {
    fn get_info(&self) -> Result<String, MountError> {
        Ok("Simulator".to_string())
    }

    fn set_motion(&mut self, axis: Axis, speed: f64) -> Result<(), MountError> {
        let speed_pix_per_sec = speed / SIDEREAL_RATE * self.data.sky_rotation_speed_pix_per_sec() as f64;

        match axis {
            Axis::Primary => self.data.primary_axis_speed.store(speed_pix_per_sec as f32, Ordering::Release),
            Axis::Secondary => self.data.secondary_axis_speed.store(speed_pix_per_sec as f32, Ordering::Release),
        }

        Ok(())
    }

    fn stop_motion(&mut self, axis: Axis) -> Result<(), MountError> {
        match axis {
            Axis::Primary => self.data.primary_axis_speed.store(0.0, Ordering::Release),
            Axis::Secondary => self.data.secondary_axis_speed.store(0.0, Ordering::Release),
        }

        Ok(())
    }

    fn get_motion_speed(&self, _axis: Axis) -> Result<f64, MountError> {
        unimplemented!()
    }

    fn set_mount_simulator_data(&mut self, mount_simulator_data: crate::MountSimulatorData) {
        self.data = mount_simulator_data;
        self.data.mount_connected.store(true, Ordering::Release);
    }
}
