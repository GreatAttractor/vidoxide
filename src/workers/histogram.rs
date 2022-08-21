//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Histogram thread.
//!

use ga_image::{Image, ImageView, PixelFormat};
use ga_image::point::Rect;

#[derive(Debug)]
pub struct HistogramRequest {
    pub image: Image,
    pub fragment: Option<Rect>
}

#[derive(Debug)]
pub enum MainToHistogramThreadMsg {
    CalculateHistogram(HistogramRequest)
}

const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;

pub struct Histogram {
    /// True if the histogram was calculated from color or raw color (CFA) image.
    is_rgb: bool,
    /// Each element contains 3 values for each of RGB channels; for mono images, R=G=B.
    values: [[usize; 3]; 256]
}

impl Histogram {
    pub fn new() -> Histogram { Histogram{ is_rgb: false, values: [[0usize; 3]; 256] } }
    pub fn values(&self) -> &[[usize; 3]] { &self.values }
    pub fn is_rgb(&self) -> bool { self.is_rgb }
}

pub fn histogram_thread(
    sender: glib::Sender<Histogram>,
    receiver: crossbeam::channel::Receiver<MainToHistogramThreadMsg>,
) {
    loop {
        match receiver.recv() {
            Ok(msg) => match msg {
                MainToHistogramThreadMsg::CalculateHistogram(hist_request) => {
                    let histogram = calculate_histogram(hist_request);
                    sender.send(histogram).unwrap();
                }
            },

            _ => break
        }
    }
}

// TODO: ignore hotpixels
fn calculate_histogram(hist_request: HistogramRequest) -> Histogram {
    let img_view = ImageView::new(&hist_request.image, hist_request.fragment);

    let mut values = [[0usize; 3]; 256];
    let is_rgb = !img_view.pixel_format().is_mono();

    match img_view.pixel_format() {
        PixelFormat::Mono8 => {
            for y in 0..img_view.height() {
                let line = img_view.line::<u8>(y);
                for p in line {
                    for i in 0..3 {
                        unsafe { values.get_unchecked_mut(*p as usize)[i] += 1; }
                    }
                }
            }
        },

        PixelFormat::Mono16 => {
            for y in 0..img_view.height() {
                let line = img_view.line::<u16>(y);
                for p in line {
                    for i in 0..3 {
                        unsafe { values.get_unchecked_mut((*p >> 8) as usize)[i] += 1; }
                    }
                }
            }
        },

        PixelFormat::RGB8 => {
            for y in 0..img_view.height() {
                let line = img_view.line::<u8>(y);
                for i in (0..line.len()).step_by(3) {
                    unsafe {
                        values.get_unchecked_mut(*line.get_unchecked(i    ) as usize)[RED] += 1;
                        values.get_unchecked_mut(*line.get_unchecked(i + 1) as usize)[GREEN] += 1;
                        values.get_unchecked_mut(*line.get_unchecked(i + 2) as usize)[BLUE] += 1;
                    }
                }
            }
        },

        _ => if img_view.pixel_format().is_cfa() {
            if img_view.pixel_format().bytes_per_channel() == 1 {
                count_cfa_values::<u8>(&mut values, &img_view);
            } else if img_view.pixel_format().bytes_per_channel() == 2 {
                //FIXME: need to scale down values to 0-255 before increasing the counters
                //count_cfa_values::<u16>(&mut values, &img_view);
            }
        }

        //TODO: implement for other formats
    }

    Histogram{ is_rgb, values }
}

fn count_cfa_values<T: 'static + Copy + Default + Into<usize>>(values: &mut [[usize; 3]; 256], img_view: &ImageView) {
    let cfa = img_view.pixel_format().cfa_pattern();
    let dx_r = cfa.red_col_ofs() as usize;
    let dy_r = cfa.red_row_ofs() as usize;
    let dx_b = dx_r ^ 1;
    let dy_b = dy_r ^ 1;

    for y in (0..img_view.height()).step_by(2) {
        if (y + dy_r as u32) < img_view.height() {
            let line_red = img_view.line::<T>(y + dy_r as u32);
            for red in line_red.iter().skip(dx_r).step_by(2) {
                unsafe { values.get_unchecked_mut(Into::<usize>::into(*red))[RED] += 1; }
            }
            for green in line_red.iter().skip(dx_r ^ 1).step_by(2) {
                unsafe { values.get_unchecked_mut(Into::<usize>::into(*green))[GREEN] += 1; }
            }
        }

        if (y + dy_b as u32) < img_view.height() {
            let line_blue = img_view.line::<T>(y + dy_b as u32);
            for blue in line_blue.iter().skip(dx_b).step_by(2) {
                unsafe { values.get_unchecked_mut(Into::<usize>::into(*blue))[BLUE] += 1; }
            }
            for green in line_blue.iter().skip(dx_b ^ 1).step_by(2) {
                unsafe { values.get_unchecked_mut(Into::<usize>::into(*green))[GREEN] += 1; }
            }
        }
    }
}
