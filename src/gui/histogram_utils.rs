//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Image histogram utilities.
//!

use ga_image::{Image, ImageView, point::Rect};
use num_traits::{bounds::Bounded, cast::AsPrimitive};
use std::ops::{Div, Mul, Sub};

pub fn stretch_histogram(image: &Image, area: &Option<Rect>) -> Image {
    let mut result = image.clone();

    let view = ImageView::new(&result, *area);

    let bpch = image.pixel_format().bytes_per_channel();
    if bpch == 1 {
        let (min, max) = find_min_max_value::<u8>(&view);
        scale_values::<u8, u16>(&mut result, area, min, max, 0xFF);
    } else if bpch == 2 {
        let (min, max) = find_min_max_value::<u16>(&view);
        scale_values::<u16, u32>(&mut result, area, min, max, 0xFFFF);
    } else if bpch == 4 {
        let (min, max) = find_min_max_value::<f32>(&view);
        scale_values::<f32, f32>(&mut result, area, min, max, 1.0);
    } else if bpch == 8 {
        let (min, max) = find_min_max_value::<f64>(&view);
        scale_values::<f64, f64>(&mut result, area, min, max, 1.0);
    }

    result
}

/// Find minimal and maximal pixel value in `image`.
///
/// `T`: type of pixel (channel) values.
///
fn find_min_max_value<T: 'static + Bounded + Copy + Default + PartialOrd>(image: &ImageView) -> (T, T) {
    let mut min_val = T::max_value();
    let mut max_val = T::min_value();

    for y in 0..image.height() {
        let line = image.line::<T>(y);
        for val in line.iter() {
            if val.partial_cmp(&min_val) == Some(core::cmp::Ordering::Less) {
                min_val = *val;
            }
            if val.partial_cmp(&max_val) == Some(core::cmp::Ordering::Greater) {
                max_val = *val;
            }
        }
    }

    (min_val, max_val)
}

/// Scales pixel values in `image` (or just in `area`, if set) so that the (min, max) range is stretched to (0, full_range).
///
/// `T`: type of pixel (channel) values.
/// `Product`: type which can fit the result of `T` * `T`.
///
fn scale_values<T, Product>(image: &mut Image, area: &Option<Rect>, min: T, max: T, full_range: T)
where
    T: 'static + Copy + Default + Sub<Output = T> + PartialOrd,
    Product: std::convert::From<T> + Div<Output = Product> + Mul<Output = Product> + AsPrimitive<T>
{
    if max.partial_cmp(&min) != Some(core::cmp::Ordering::Greater) { return; }

    let area = area.unwrap_or(image.img_rect());
    let num_ch = image.pixel_format().num_channels();

    for y in area.y..area.y + area.height as i32 {
        let line = image.line_mut::<T>(y as u32);
        for val in line.iter_mut().skip(num_ch * area.x as usize).take(num_ch * area.width as usize) {
            *val = AsPrimitive::<T>::as_(
                Into::<Product>::into(*val - min) * Into::<Product>::into(full_range) / Into::<Product>::into(max - min)
            );
        }
    }
}
