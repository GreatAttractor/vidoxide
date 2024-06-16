//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Focuser simulator driver.
//!

use crate::devices::focuser::{DegC, Focuser, Position, PositionRange, Speed, SpeedRange, State};
use std::error::Error;

const UNIT_SPEED: f64 = 100.0; // change in position per second for speed = 1.0

#[derive(Debug)]
struct MoveRequest {
    t: std::time::Instant,
    origin: Position,
    target: Position,
    speed: Speed
}

pub struct Simulator {
    position: Position,
    move_request: Option<MoveRequest>
}

impl Simulator {
    pub fn new() -> Result<Simulator, Box<dyn Error>> {
        Ok(Simulator{
            position: Position(0),
            move_request: None
        })
    }

    fn update_state(&mut self, new_request: Option<MoveRequest>) {
        if let Some(prev_req) = &self.move_request {
            let raw_speed = UNIT_SPEED * prev_req.speed.0;
            let time_to_target =
                std::time::Duration::from_secs_f64((prev_req.target.0 - prev_req.origin.0).abs() as f64 / raw_speed);
            let dt = prev_req.t.elapsed();
            if dt > time_to_target {
                self.position = prev_req.target;
            } else {
                let sign = (prev_req.target.0 - prev_req.origin.0).signum() as f64;
                self.position = Position((prev_req.origin.0 as f64 + dt.as_secs_f64() * sign * raw_speed) as i32);
            }
        }

        if new_request.is_some() { self.move_request = new_request }
    }
}

impl Focuser for Simulator {
    fn info(&self) -> String {
        "Simulator".into()
    }

    fn move_(&mut self, target: Position, speed: Speed) -> Result<(), Box<dyn Error>> {
        if speed.is_zero() {
            self.stop().unwrap();
        } else {
            self.update_state(Some(MoveRequest{ t: std::time::Instant::now(), origin: self.position, target, speed }));
        }

        Ok(())
    }

    fn pos_range(&mut self) -> Result<PositionRange, Box<dyn Error>> {
        Ok(PositionRange{ min: Position(-10_000), max: Position(10_000) })
    }

    fn speed_range(&mut self) -> Result<SpeedRange, Box<dyn Error>> {
        Ok(SpeedRange{ min: Speed::new(1.0 / 100.0), max: Speed::new(100.0) })
    }

    fn state(&mut self) -> Result<State, Box<dyn Error>> {
        self.update_state(None);
        Ok(State{
            pos: self.position,
            moving: Some(self.move_request.is_some()),
            temperature: Some(DegC(20.0))
        })
    }

    fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.update_state(None);
        self.move_request = None;
        Ok(())
    }

    fn sync(&mut self, current_pos: Position) -> Result<(), Box<dyn Error>> {
        unimplemented!()
    }
}
