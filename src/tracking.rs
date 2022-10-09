//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Image feature tracking.
//!

use cgmath::{EuclideanSpace, Point2, Vector2};
use crate::{to_p, to_p2, to_v2};
use ga_image::{Image, ImageView, PixelFormat, point::Rect};

const ANCHOR_SEARCH_RADIUS: i32 = 20;
const REF_BLOCK_SIZE: u32 = 64;
const MIN_REL_BRIGHTNESS_FOR_CENTROID: f32 = 30.0 / 255.0;

struct Anchor {
    pos: Point2<i32>,
    ref_block: Image
}

struct Centroid {
    area: Rect,
    /// Desired position of the `area`s centroid relative to `area`'s origin.
    offset: Vector2<i32>
}

enum State {
    Disabled,
    Centroid(Centroid),
    Anchor(Anchor)
}

pub struct ImageTracker {
    state: State
}

impl ImageTracker {
    pub fn new_with_centroid(area: Rect, image: &Image) -> ImageTracker {
        ImageTracker{
            state: State::Centroid(Centroid{ area, offset: to_v2(image.centroid(Some(area))) })
        }
    }

    pub fn new_with_anchor(pos: Point2<i32>, image: &Image) -> ImageTracker {
        let ref_block = image.convert_pix_fmt_of_subimage(
            PixelFormat::Mono8,
            to_p(pos - Vector2{ x: REF_BLOCK_SIZE as i32 / 2, y: REF_BLOCK_SIZE as i32 / 2 }),
            REF_BLOCK_SIZE, REF_BLOCK_SIZE,
            None
        );

        ImageTracker{
            state: State::Anchor(Anchor{
                pos,
                ref_block
            })
        }
    }

    pub fn position(&self) -> Option<Point2<i32>> {
        match &self.state {
            State::Disabled => None,
            State::Centroid(centroid) => Some(to_p2(centroid.area.pos()) + centroid.offset),
            State::Anchor(anchor) => Some(anchor.pos)
        }
    }

    pub fn centroid_area(&self) -> Option<Rect> {
        match &self.state {
            State::Centroid(centroid) => Some(centroid.area),
            _ => None
        }
    }

    /// Updates tracker state.
    ///
    /// # Parameters
    ///
    /// * `image` - New image to be used for tracking.
    /// * `offset` - Offset of `image` relative to the image specified in the previous call
    ///   (may be non-zero, e.g., after a ROI change).
    ///
    #[must_use]
    pub fn update(&mut self, image: &Image, offset: Vector2<i32>) -> Result<(), ()> { //TODO: handle `offset`
        match &mut self.state {
            State::Centroid(centroid) => {
                if !image.img_rect().contains_rect(&centroid.area) {
                    return Err(());
                }

                let mut frag8 = image.convert_pix_fmt_of_subimage(
                    PixelFormat::Mono8,
                    centroid.area.pos(),
                    centroid.area.width,
                    centroid.area.height,
                    None
                );

                let w = frag8.width();
                let h = frag8.height();
                for y in 0..h {
                    let line = frag8.line_mut::<u8>(y);
                    for x in 0..w {
                        if line[x as usize] < (MIN_REL_BRIGHTNESS_FOR_CENTROID * 255.0) as u8 { line[x as usize] = 0; }
                    }
                }
                let new_c = frag8.centroid(None);

                // TODO: make it configurable
                // ignore a sudden jump which seems implausibly large; it's likely due to an image artifact
                // (e.g., a damaged/shredded frame)
                let old_c = centroid.offset + centroid.offset;
                if (old_c.x - new_c.x).pow(2) + (old_c.y - new_c.y).pow(2)
                    >= ((centroid.area.width as i32).pow(2) + (centroid.area.height as i32).pow(2)) * 3i32.pow(2) / 4i32.pow(2) {

                    return Ok(());
                }

                centroid.area.x += new_c.x - centroid.offset.x;
                centroid.area.y += new_c.y - centroid.offset.y;

                if image.img_rect().contains_rect(&centroid.area) {
                    Ok(())
                } else {
                    self.state = State::Disabled;
                    Err(())
                }
            },

            State::Anchor(anchor) => update_anchor(anchor, image, offset),

            State::Disabled => Err(())
        }
    }
}

/// Updates anchor.
///
/// # Parameters
///
/// * `anchor` - Anchor to update.
/// * `image` - New image to be used for tracking.
/// * `offset` - Offset of `image` relative to the image specified in the previous call
///   (may be non-zero, e.g., after a ROI change).
///
fn update_anchor(anchor: &mut Anchor, image: &Image, _offset: Vector2<i32>) -> Result<(), ()> {
    let mut search_xmin = anchor.pos.x - ANCHOR_SEARCH_RADIUS;
    let mut search_xmax = anchor.pos.x + ANCHOR_SEARCH_RADIUS;
    let mut search_ymin = anchor.pos.y - ANCHOR_SEARCH_RADIUS;
    let mut search_ymax = anchor.pos.y + ANCHOR_SEARCH_RADIUS;

    let search_rect = Rect{
        x: search_xmin - anchor.ref_block.width() as i32 / 2,
        y: search_ymin - anchor.ref_block.height() as i32 / 2,
        width: (search_xmax - search_xmin) as u32 + anchor.ref_block.width(),
        height: (search_ymax - search_ymin) as u32 + anchor.ref_block.height()
    };

    if !image.img_rect().contains_rect(&search_rect) { return Err(()); }

    // TODO: don't convert image fragment if already Mono8
    let search_area: Image = image.convert_pix_fmt_of_subimage(
        PixelFormat::Mono8,
        search_rect.pos(),
        search_rect.width,
        search_rect.height,
        None
    );

    let mut best_pos = anchor.pos;

    let mut search_step = 2;
    while search_step > 0 {
        let mut min_diff_sum = u64::max_value();

        let mut y = search_ymin;
        while y < search_ymax {
            let mut x = search_xmin;
            while x < search_xmax {
                let comparison_rect = Rect{
                    x: x - search_rect.x - anchor.ref_block.width() as i32 / 2,
                    y: y - search_rect.y - anchor.ref_block.height() as i32 / 2,
                    width: anchor.ref_block.width(),
                    height: anchor.ref_block.height(),
                };
                if search_area.img_rect().contains_rect(&comparison_rect) {
                    let sum_abs_diffs = calc_sum_of_abs_diffs(
                        &ImageView::new(&search_area, Some(comparison_rect)),
                        &anchor.ref_block.view()
                    );

                    if sum_abs_diffs < min_diff_sum {
                        min_diff_sum = sum_abs_diffs;
                        best_pos = Point2{ x, y };
                    }
                }

                x += search_step;
            }
            y += search_step;
        }

        search_xmin = best_pos.x - search_step;
        search_ymin = best_pos.y - search_step;
        search_xmax = best_pos.x + search_step;
        search_ymax = best_pos.y + search_step;

        search_step /= 2;
    }

    // exponential damping
    anchor.pos += (best_pos - anchor.pos) / 2;

    Ok(())
}

/// Calculates sum of absolute pixel differences between images.
fn calc_sum_of_abs_diffs(img1: &ImageView, img2: &ImageView) -> u64 {
    assert!(img1.pixel_format() == PixelFormat::Mono8);
    assert!(img2.pixel_format() == PixelFormat::Mono8);
    assert!(img1.width() == img2.width());
    assert!(img1.height() == img2.height());

    let mut sum_diffs = 0u64;
    for y in 0..img1.height() {
        let line1 = img1.line_raw(y);
        let line2 = img2.line_raw(y);

        //TODO: use unsafe access
        sum_diffs += (0..img1.width()).fold(0, |sum, i| {
            sum + (line1[i as usize] as i16 - line2[i as usize] as i16).pow(2) as u64
        });
    }

    sum_diffs
}
