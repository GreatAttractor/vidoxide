//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Guiding.
//!

use crate::ProgramData;
use crate::gui::show_message;
use crate::mount;
use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;

//TODO: set it from GUI
const GUIDE_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2000);

pub fn start_guiding(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let failed: bool = if program_data_rc.borrow().tracking.is_none() {
        show_message("Target tracking is not enabled.", "Error", gtk::MessageType::Error);
        true
    } else if program_data_rc.borrow().mount_data.calibration.is_none() {
        show_message("Calibration has not been performed.", "Error", gtk::MessageType::Error);
        true
    } else { false };

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
}

pub fn stop_guiding(program_data_rc: &Rc<RefCell<ProgramData>>) -> Result<(), mount::MountError> {
    let sd_on;

    {
        let mut pd = program_data_rc.borrow_mut();
        sd_on = pd.mount_data.sidereal_tracking_on;
        pd.mount_data.guiding_timer.stop();
        pd.mount_data.guide_slewing = false;
        pd.mount_data.guiding_pos = None;
    }

    let mut error: Option<mount::MountError> = None;

    match program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().set_motion(
        mount::Axis::Primary,
        if sd_on { 1.0 * mount::SIDEREAL_RATE } else { 0.0 }
    ) {
        Err(e) => error = Some(e),
        _ => ()
    }

    match program_data_rc.borrow_mut().mount_data.mount.as_mut().unwrap().stop_motion(mount::Axis::Secondary) {
        Err(e) => error = Some(e),
        _ => ()
    }

    if error.is_some() { Err(error.unwrap()) } else { Ok(()) }
}

pub fn guiding_step(program_data_rc: &Rc<RefCell<ProgramData>>) {
    /// Max acceptable X and Y difference between current and desired tracking position at the end of a guiding slew.
    const GUIDE_POS_MARGIN: i32 = 5;

    const GUIDE_DIR_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

    let mut error: Option<mount::MountError> = None;

    loop { // `loop` is only for an easy early exit from this block
        let mut pd = program_data_rc.borrow_mut();

        let dpos = *pd.mount_data.guiding_pos.as_ref().unwrap() - pd.tracking.as_ref().unwrap().pos;
        let st_on = pd.mount_data.sidereal_tracking_on;

        if dpos.x.abs() > GUIDE_POS_MARGIN || dpos.y.abs() > GUIDE_POS_MARGIN {
            let guide_dir_axis_space = guiding_direction(
                pd.mount_data.calibration.as_ref().unwrap().img_to_mount_axes.as_ref().unwrap(),
                dpos
            );

            let speed = pd.gui.as_ref().unwrap().mount_widgets().guide_speed() * mount::SIDEREAL_RATE;

            if let Err(e) = pd.mount_data.mount.as_mut().unwrap().set_motion(
                mount::Axis::Primary,
                speed * guide_dir_axis_space.0 + if st_on { 1.0 * mount::SIDEREAL_RATE } else { 0.0 }
            ) {
                error = Some(e);
                break;
            }

            if let Err(e) = pd.mount_data.mount.as_mut().unwrap().set_motion(
                mount::Axis::Secondary,
                speed * guide_dir_axis_space.1
            ) {
                error = Some(e);
                break;
            }

            pd.mount_data.guide_slewing = true;

            pd.mount_data.guiding_timer.run_once(
                GUIDE_DIR_UPDATE_INTERVAL,
                clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
            );
        } else {
            if let Err(e) = pd.mount_data.mount.as_mut().unwrap().set_motion(
                mount::Axis::Primary,
                if st_on { mount::SIDEREAL_RATE } else { 0.0 }
            ) {
                error = Some(e);
                break;
            }
            if let Err(e) = pd.mount_data.mount.as_mut().unwrap().stop_motion(mount::Axis::Secondary) {
                error = Some(e);
                break;
            }
            pd.mount_data.guide_slewing = false;

            pd.mount_data.guiding_timer.run_once(
                GUIDE_CHECK_INTERVAL,
                clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
            );
        }

        break;
    }

    if let Some(e) = error {
        // mount already failed, so ignore further mount errors from this call, if any
        let _ = stop_guiding(program_data_rc);
        program_data_rc.borrow().gui.as_ref().unwrap().mount_widgets().disable_guide();
        crate::gui::on_mount_error(&e);
    }
}

/// Returns guiding direction (unit vector in RA&Dec space) in order to move along `target_offset` in image space.
fn guiding_direction(img_to_mount_axes_matrix: &[[f64; 2]; 2], target_offset: ga_image::point::Point) -> (f64, f64) {
    #[allow(non_snake_case)]
    let M = img_to_mount_axes_matrix;

    let mut guide_dir_axis_space: (f64, f64) = (
        M[0][0] * target_offset.x as f64 + M[0][1] * target_offset.y as f64,
        M[1][0] * target_offset.x as f64 + M[1][1] * target_offset.y as f64
    );
    let len = (guide_dir_axis_space.0.powi(2) + guide_dir_axis_space.1.powi(2)).sqrt();
    guide_dir_axis_space.0 /= len;
    guide_dir_axis_space.1 /= len;

    guide_dir_axis_space
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
pub fn create_img_to_mount_axes_matrix(primary_dir: (f64, f64), secondary_dir: (f64, f64))
-> Result<[[f64; 2]; 2], ()> {
    // telescope-axes-space-to-image-space transformation matrix
    let axes_to_img: [[f64; 2]; 2] = [[primary_dir.0, secondary_dir.0], [primary_dir.1, secondary_dir.1]];

    let determinant = axes_to_img[0][0] * axes_to_img[1][1] - axes_to_img[0][1] * axes_to_img[1][0];
    if determinant == 0.0 {
        return Err(())
    } else {
        // axes_to_img⁻¹
        Ok([
            [ axes_to_img[1][1] / determinant, -axes_to_img[0][1] / determinant],
            [-axes_to_img[1][0] / determinant,  axes_to_img[0][0] / determinant]
        ])
    }
}

mod tests {
    use super::*;

    macro_rules! assert_almost_eq {
        ($expected:expr, $actual:expr) => {
            if ($expected.0 - $actual.0).abs() > 1.0e-9 {
                panic!("expected: {}, but was: {}", $expected.0, $actual.0);
            }

            if ($expected.1 - $actual.1).abs() > 1.0e-9 {
                panic!("expected: {}, but was: {}", $expected.1, $actual.1);
            }
        };
    }

    #[test]
    fn test_guiding_direction() {
        let point = |x, y| { ga_image::point::Point{ x, y } };
        let mat = |primary_dir, secondary_dir| { create_img_to_mount_axes_matrix(primary_dir, secondary_dir).unwrap() };
        let s2 = 1.0 / 2.0f64.sqrt();

        // All test cases ask for primary & secondary axis direction corresponding to image space vector [1, 0].

        assert_almost_eq!((1.0, 0.0), guiding_direction(&mat((1.0, 0.0), (0.0, 1.0)), point(1, 0)));
        assert_almost_eq!((1.0, 0.0), guiding_direction(&mat((1.0, 0.0), (0.0, -1.0)), point(1, 0)));

        assert_almost_eq!((s2, -s2), guiding_direction(&mat((1.0, 1.0), (-1.0, 1.0)), point(1, 0)));
        assert_almost_eq!((s2,  s2), guiding_direction(&mat((1.0, 1.0), (1.0, -1.0)), point(1, 0)));

        assert_almost_eq!((0.0, 1.0), guiding_direction(&mat((0.0, 1.0), (1.0, 0.0)), point(1, 0)));
        assert_almost_eq!((0.0, -1.0), guiding_direction(&mat((0.0, 1.0), (-1.0, 0.0)), point(1, 0)));

        assert_almost_eq!((-s2, s2), guiding_direction(&mat((-1.0, 1.0), (1.0, 1.0)), point(1, 0)));
        assert_almost_eq!((-s2, -s2), guiding_direction(&mat((-1.0, 1.0), (-1.0, -1.0)), point(1, 0)));

        assert_almost_eq!((-1.0, 0.0), guiding_direction(&mat((-1.0, 0.0), (0.0, 1.0)), point(1, 0)));
        assert_almost_eq!((-1.0, 0.0), guiding_direction(&mat((-1.0, 0.0), (0.0, -1.0)), point(1, 0)));

        assert_almost_eq!((-s2, -s2), guiding_direction(&mat((-1.0, -1.0), (-1.0, 1.0)), point(1, 0)));
        assert_almost_eq!((-s2, s2), guiding_direction(&mat((-1.0, -1.0), (1.0, -1.0)), point(1, 0)));

        assert_almost_eq!((0.0, 1.0), guiding_direction(&mat((0.0, -1.0), (1.0, 0.0)), point(1, 0)));
        assert_almost_eq!((0.0, -1.0), guiding_direction(&mat((0.0, -1.0), (-1.0, 0.0)), point(1, 0)));

        assert_almost_eq!((s2, s2), guiding_direction(&mat((1.0, -1.0), (1.0, 1.0)), point(1, 0)));
        assert_almost_eq!((s2, -s2), guiding_direction(&mat((1.0, -1.0), (-1.0, -1.0)), point(1, 0)));
    }
}
