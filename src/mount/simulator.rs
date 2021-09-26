use crate::mount::{Axis, Mount, MountError};

#[derive(Debug)]
pub struct SimulatorError;

pub struct Simulator {
}

impl std::convert::From<SimulatorError> for MountError {
    fn from(e: SimulatorError) -> MountError {
        MountError::SimulatorError(e)
    }
}

impl Simulator {
    pub fn new() -> Simulator {
        Simulator{}
    }
}

impl Mount for Simulator {
    fn get_info(&self) -> Result<String, MountError> {
        Ok("Simulator".to_string())
    }

    fn set_motion(&mut self, _axis: Axis, _speed: f64) -> Result<(), MountError> {
        Ok(())
    }

    fn stop_motion(&mut self, _axis: Axis) -> Result<(), MountError> {
        Ok(())
    }

    fn get_motion_speed(&self, _axis: Axis) -> Result<f64, MountError> {
        Ok(1.0)
    }
}
