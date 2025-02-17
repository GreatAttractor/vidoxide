//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Telescope focuser module.
//!

mod dream_focuser_mini;
mod focuscube3;
mod simulator;

use crate::{ProgramData, devices::DeviceConnection};
use std::{cell::RefCell, error::Error, rc::Rc};

pub type DFminiConnection = dream_focuser_mini::Connection;
pub type FC3Connection = focuscube3::Connection;

#[derive(Copy, Clone, Debug)]
pub struct Position(pub i32);

#[derive(Copy, Clone)]
pub struct RelativePos(pub Position);

/// For each focuser driver: value of 1.0 means "normal, reasonable speed"; not "so fast the attached mechanics will be
/// torn apart before the user can react".
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Speed(f64);

impl Speed {
    pub fn new(value: f64) -> Speed {
        if value >= 0.0 {
            Speed(value)
        } else {
            panic!("invalid focuser speed: {} (expected value >= 0.0)", value);
        }
    }

    pub fn get(&self) -> f64 { self.0 }

    pub fn one() -> Speed { Speed(1.0) }

    pub fn zero() -> Speed { Speed(0.0) }

    pub fn is_zero(&self) -> bool { self.0 == 0.0 }
}

impl std::ops::Mul<f64> for Speed {
    type Output = Self;

    fn mul(self, x: f64) -> Speed {
        Speed(self.0 * x)
    }
}

pub struct PositionRange {
    pub min: Position,
    pub max: Position
}

pub struct SpeedRange {
    pub min: Speed,
    pub max: Speed
}

pub struct DegC(pub f32);

pub struct State {
    pub pos: Position,
    pub moving: Option<bool>,
    pub temperature: Option<DegC>
}

pub trait Focuser {
    #[must_use]
    fn info(&self) -> String;

    fn pos_range(&mut self) -> Result<PositionRange, Box<dyn Error>>;

    fn speed_range(&mut self) -> Result<SpeedRange, Box<dyn Error>>;

    fn state(&mut self) -> Result<State, Box<dyn Error>>;

    /// Non-blocking; reaching `target` can be queried via `state`.
    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>>;

    fn sync(&mut self, current_pos: Position) -> Result<(), Box<dyn Error>>;

    fn stop(&mut self) -> Result<(), Box<dyn Error>>;
}

#[derive(Copy, Clone)]
pub enum FocuserDir { Negative, Positive }

pub struct FocuserWrapper {
    focuser: Box<dyn Focuser>
}

impl FocuserWrapper {
    fn new(focuser: Box<dyn Focuser>) -> FocuserWrapper {
        FocuserWrapper{ focuser }
    }

    pub fn get(&self) -> &Box<dyn Focuser> { &self.focuser }

    pub fn get_mut(&mut self) -> &mut Box<dyn Focuser> { &mut self.focuser }

    /// Non-blocking; reaching target position can be queried via `Focuser::state`.
    pub fn move_rel(&mut self, rel_pos: RelativePos, speed: Speed) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }

    pub fn move_in_dir(&mut self, speed: Speed, dir: FocuserDir) -> Result<(), Box<dyn Error>> {
        let PositionRange{ min, max } = self.focuser.pos_range().unwrap();
        self.focuser.move_(match dir { FocuserDir::Negative => min, FocuserDir::Positive => max }, speed)
    }
}

pub fn connect_to_focuser(
    connection: DeviceConnection,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> Result<FocuserWrapper, Box<dyn Error>> {
    match connection {
        DeviceConnection::FocusCube3{ connection } =>
            Ok(FocuserWrapper::new(Box::new(focuscube3::FocusCube3::new(connection)?))),

        DeviceConnection::FocuserSimulator => Ok(FocuserWrapper::new(Box::new(simulator::Simulator::new()?))),

        DeviceConnection::DreamFocuserMini{ connection } =>
            Ok(FocuserWrapper::new(Box::new(dream_focuser_mini::DreamFocuserMini::new(
                connection,
                #[cfg(feature = "bluetooth")]
                Rc::clone(&program_data_rc.borrow().tokio_rt)
            )?))),

        _ => unimplemented!()
    }
}
