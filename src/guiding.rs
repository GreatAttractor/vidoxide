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

pub fn stop_guiding(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut pd = program_data_rc.borrow_mut();
    let sd_on = pd.mount_data.sidereal_tracking_on;
    pd.mount_data.mount.as_mut().unwrap().set_motion(
        mount::Axis::RA,
        if sd_on { 1.0 * mount::SIDEREAL_RATE } else { 0.0 }
    ).unwrap();
    pd.mount_data.mount.as_mut().unwrap().stop_motion(mount::Axis::Dec).unwrap();
    pd.mount_data.guiding_timer.stop();
    pd.mount_data.guide_slewing = false;
    pd.mount_data.guiding_pos = None;
}

pub fn guiding_step(program_data_rc: &Rc<RefCell<ProgramData>>) {
    /// Max acceptable X and Y difference between current and desired tracking position at the end of a guiding slew.
    const GUIDE_POS_MARGIN: i32 = 5;

    const GUIDE_DIR_UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

    let mut pd = program_data_rc.borrow_mut();

    let dpos = *pd.mount_data.guiding_pos.as_ref().unwrap() - pd.tracking.as_ref().unwrap().pos;
    if dpos.x.abs() > GUIDE_POS_MARGIN || dpos.y.abs() > GUIDE_POS_MARGIN {
        let guide_dir_radec_space =
            guiding_direction(pd.mount_data.calibration.as_ref().unwrap().img_to_radec.as_ref().unwrap(), dpos);

        let speed = pd.gui.as_ref().unwrap().mount_widgets().guide_speed() * mount::SIDEREAL_RATE;

        let sd_on = pd.mount_data.sidereal_tracking_on;
        pd.mount_data.mount.as_mut().unwrap().set_motion(
            mount::Axis::RA,
            speed * guide_dir_radec_space.0 + if sd_on { 1.0 * mount::SIDEREAL_RATE } else { 0.0 }
        ).unwrap();
        pd.mount_data.mount.as_mut().unwrap().set_motion(mount::Axis::Dec, speed * guide_dir_radec_space.1).unwrap();

        pd.mount_data.guide_slewing = true;

        pd.mount_data.guiding_timer.run_once(
            GUIDE_DIR_UPDATE_INTERVAL,
            clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
        );
    } else {
        let st_on = pd.mount_data.sidereal_tracking_on;
        pd.mount_data.mount.as_mut().unwrap().set_motion(
            mount::Axis::RA,
            if st_on { mount::SIDEREAL_RATE } else { 0.0 }
        ).unwrap();
        pd.mount_data.mount.as_mut().unwrap().stop_motion(mount::Axis::Dec).unwrap();
        pd.mount_data.guide_slewing = false;

        pd.mount_data.guiding_timer.run_once(
            GUIDE_CHECK_INTERVAL,
            clone!(@weak program_data_rc => @default-panic, move || guiding_step(&program_data_rc))
        );
    }
}

/// Returns guiding direction (unit vector in RA&Dec space) in order to move along `target_offset` in image space.
fn guiding_direction(img_to_radec_matrix: &[[f64; 2]; 2], target_offset: ga_image::point::Point) -> (f64, f64) {
    #[allow(non_snake_case)]
    let M = img_to_radec_matrix;

    let mut guide_dir_radec_space: (f64, f64) = (
        M[0][0] * target_offset.x as f64 + M[0][1] * target_offset.y as f64,
        M[1][0] * target_offset.x as f64 + M[1][1] * target_offset.y as f64
    );
    let len = (guide_dir_radec_space.0.powi(2) + guide_dir_radec_space.1.powi(2)).sqrt();
    guide_dir_radec_space.0 /= len;
    guide_dir_radec_space.1 /= len;

    guide_dir_radec_space
}

/// Creates a matrix transforming image-space vectors to ra&dec-space.
///
/// # Parameters
///
/// * `ra_dir` - Direction in image space corresponding to a positive slew in rightascension.
/// * `dec_dir` - Direction in image space corresponding to a positive slew in declination.
///
pub fn create_img_to_radec_matrix(ra_dir: (f64, f64), dec_dir: (f64, f64)) -> [[f64; 2]; 2] {
    // ra&dec-space-to-image-space transformation matrix
    let radec_to_img: [[f64; 2]; 2] = [[ra_dir.0, dec_dir.0], [ra_dir.1, dec_dir.1]];

    let det_rtoi = radec_to_img[0][0] * radec_to_img[1][1] - radec_to_img[0][1] * radec_to_img[1][0];
    // radec_to_img⁻¹
    [
        [ radec_to_img[1][1] / det_rtoi, -radec_to_img[0][1] / det_rtoi],
        [-radec_to_img[1][0] / det_rtoi,  radec_to_img[0][0] / det_rtoi]
    ]
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
        let mat = |ra_dir, dec_dir| { create_img_to_radec_matrix(ra_dir, dec_dir) };
        let s2 = 1.0 / 2.0f64.sqrt();

        // All test cases ask for RA & Dec direction corresponding to image space vector [1, 0].

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
