//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Guiding.
//!

use cgmath::{InnerSpace, Point2, SquareMatrix, Matrix2, Vector2};
use crate::ProgramData;
use crate::gui::show_message;
use crate::mount;
use crate::mount::RadPerSec;
use glib::clone;
use std::{cell::RefCell, error::Error, rc::Rc};

//TODO: set it from GUI
const GUIDE_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2000);

pub fn start_guiding(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let failed: bool = if program_data_rc.borrow().tracking.is_none() {
        show_message("Target tracking is not enabled.", "Error", gtk::MessageType::Error);
        true
    } else if program_data_rc.borrow().mount_data.calibration.is_none() {
        show_message("Calibration has not been performed.", "Error", gtk::MessageType::Error);
        true
    } else if !program_data_rc.borrow().mount_data.sky_tracking_on {
        show_message("Sky tracking is not enabled.", "Error", gtk::MessageType::Error);
        true
    } else {
        false
    };

    if failed {
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets().disable_guide();
        return;
    }

    let mut pd = program_data_rc.borrow_mut();
    pd.mount_data.guiding_pos = Some(pd.tracking.as_ref().unwrap().pos);
    pd.mount_data.guiding_timer.run_once(
        GUIDE_CHECK_INTERVAL,
        clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
    );

    log::info!("guiding enabled");
}

pub fn stop_guiding(program_data_rc: &Rc<RefCell<ProgramData>>) -> Result<(), Box<dyn Error>> {
    {
        let mut pd = program_data_rc.borrow_mut();
        pd.mount_data.guiding_timer.stop();
        pd.mount_data.guide_slewing = false;
        pd.mount_data.guiding_pos = None;
    }

    log::info!("guiding disabled");

    program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().guide(RadPerSec(0.0), RadPerSec(0.0))
}

pub fn guiding_step(program_data_rc: &Rc<RefCell<ProgramData>>) {
    /// Max acceptable X and Y difference between current and desired tracking position at the end of a guiding slew.
    const GUIDE_POS_MARGIN: i32 = 5;

    const GUIDE_DIR_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

    let mut error = Ok(());

    loop { // `loop` is only for an easy early exit from this block
        let mut pd = program_data_rc.borrow_mut();

        let dpos = *pd.mount_data.guiding_pos.as_ref().unwrap() - pd.tracking.as_ref().unwrap().pos;
        let st_on = pd.mount_data.sky_tracking_on;

        if dpos.x.abs() > GUIDE_POS_MARGIN || dpos.y.abs() > GUIDE_POS_MARGIN {
            let guide_dir_axis_space = guiding_direction(
                pd.mount_data.calibration.as_ref().unwrap().img_to_mount_axes.as_ref().unwrap(),
                dpos.cast::<f64>().unwrap()
            );

            let speed = pd.gui.as_ref().unwrap().mount_widgets().guide_speed() * mount::SIDEREAL_RATE;

            let x_speed = speed * guide_dir_axis_space.x;
            let y_speed = speed * guide_dir_axis_space.y;

            log::info!(
                "off target by [{}, {}] pix; sending guide cmd [{:.2}, {:.2}] · sidereal",
                dpos.x, dpos.y, x_speed.0 / mount::SIDEREAL_RATE.0, y_speed.0 / mount::SIDEREAL_RATE.0
            );
            error = pd.mount_data.mount.as_mut().unwrap().guide(x_speed, y_speed);

            if error.is_err() { break; }

            pd.mount_data.guide_slewing = true;

            pd.mount_data.guiding_timer.run_once(
                GUIDE_DIR_UPDATE_INTERVAL,
                clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
            );
        } else {
            error = pd.mount_data.mount.as_mut().unwrap().guide(RadPerSec(0.0), RadPerSec(0.0));
            if error.is_err() { break; }

            pd.mount_data.guide_slewing = false;
            log::info!("back on target");

            pd.mount_data.guiding_timer.run_once(
                GUIDE_CHECK_INTERVAL,
                clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
            );
        }

        break;
    }

    if let Err(e) = error {
        // mount already failed, so ignore further mount errors from this call, if any
        let _ = stop_guiding(program_data_rc);
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets().disable_guide();
        crate::gui::on_mount_error(&e);
    }
}

/// Returns guiding direction (unit vector in RA&Dec space) in order to move along `target_offset` in image space.
fn guiding_direction(img_to_mount_axes_matrix: &Matrix2<f64>, target_offset: Vector2<f64>) -> Vector2<f64> {
    let guide_dir_axis_space = img_to_mount_axes_matrix * target_offset.cast::<f64>().unwrap();
    guide_dir_axis_space.normalize()
}

/// Creates a matrix transforming image-space vectors to mount-axes-space.
///
/// # Parameters
///
/// * `primary_dir` - Direction in image space corresponding to positive slew around primary axis.
/// * `secondary_dir` - Direction in image space corresponding to positive slew around secondary axis.
///
/// Fails if the provided directions are (anti-)parallel.
///
pub fn create_img_to_mount_axes_matrix(primary_dir: Vector2<f64>, secondary_dir: Vector2<f64>)
-> Result<Matrix2<f64>, ()> {
    // telescope-axes-space-to-image-space transformation matrix
    let axes_to_img = Matrix2::from_cols(primary_dir, secondary_dir);

    match axes_to_img.invert() {
        None => Err(()),
        Some(m) => Ok(m)
    }
}

mod tests {
    use super::*;

    macro_rules! assert_almost_eq {
        ($expected:expr, $actual:expr) => {
            if ($expected.x - $actual.x).abs() > 1.0e-9 {
                panic!("expected: {}, but was: {}", $expected.x, $actual.x);
            }

            if ($expected.y - $actual.y).abs() > 1.0e-9 {
                panic!("expected: {}, but was: {}", $expected.y, $actual.y);
            }
        };
    }

    #[test]
    fn test_guiding_direction() {
        let v2 = |x, y| { Vector2{ x, y } };
        let mat = |primary_dir, secondary_dir| { create_img_to_mount_axes_matrix(primary_dir, secondary_dir).unwrap() };
        let s2 = 1.0 / 2.0f64.sqrt();

        // All test cases ask for primary & secondary axis direction corresponding to image space vector [1, 0].

        assert_almost_eq!(v2(1.0, 0.0), guiding_direction(&mat(v2(1.0, 0.0), v2(0.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(1.0, 0.0), guiding_direction(&mat(v2(1.0, 0.0), v2(0.0, -1.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(s2, -s2), guiding_direction(&mat(v2(1.0, 1.0), v2(-1.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(s2,  s2), guiding_direction(&mat(v2(1.0, 1.0), v2(1.0, -1.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(0.0, 1.0), guiding_direction(&mat(v2(0.0, 1.0), v2(1.0, 0.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(0.0, -1.0), guiding_direction(&mat(v2(0.0, 1.0), v2(-1.0, 0.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(-s2, s2), guiding_direction(&mat(v2(-1.0, 1.0), v2(1.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(-s2, -s2), guiding_direction(&mat(v2(-1.0, 1.0), v2(-1.0, -1.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(-1.0, 0.0), guiding_direction(&mat(v2(-1.0, 0.0), v2(0.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(-1.0, 0.0), guiding_direction(&mat(v2(-1.0, 0.0), v2(0.0, -1.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(-s2, -s2), guiding_direction(&mat(v2(-1.0, -1.0), v2(-1.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(-s2, s2), guiding_direction(&mat(v2(-1.0, -1.0), v2(1.0, -1.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(0.0, 1.0), guiding_direction(&mat(v2(0.0, -1.0), v2(1.0, 0.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(0.0, -1.0), guiding_direction(&mat(v2(0.0, -1.0), v2(-1.0, 0.0)), v2(1.0, 0.0)));

        assert_almost_eq!(v2(s2, s2), guiding_direction(&mat(v2(1.0, -1.0), v2(1.0, 1.0)), v2(1.0, 0.0)));
        assert_almost_eq!(v2(s2, -s2), guiding_direction(&mat(v2(1.0, -1.0), v2(-1.0, -1.0)), v2(1.0, 0.0)));
    }
}
