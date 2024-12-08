//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2024 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Informational overlay.
//!

use cgmath::{InnerSpace, Point2};
use crate::{gui::MouseMode, MountCalibration, ProgramData, TrackingMode};
use ga_image::Rect;
use gtk::cairo;

/// Offset (in pixels) of labels from the associated rectangle in the informational overlay.
const INFO_OVERLAY_LABEL_OFFSET: i32 = 5;

const GUIDING_POS_CIRCLE_R: f64 = 12.0;

const GUIDING_BLINK_ON: std::time::Duration = std::time::Duration::from_millis(200);
const GUIDING_BLINK_OFF: std::time::Duration = std::time::Duration::from_millis(500);

/// Size (in pixels) of the font used in the informational overlay.
const DEFAULT_INFO_OVERLAY_FONT_SIZE: f64 = 10.0;

enum SelectionType {
    Line,
    Rectangle,
}

impl SelectionType {
    fn from_mouse_mode(mouse_mode: MouseMode) -> SelectionType {
        match mouse_mode {
            MouseMode::MeasureDistance => SelectionType::Line,
            _ => SelectionType::Rectangle
        }
    }
}

/// Selection made by mouse in preview area; image coordinates.
pub struct ScreenSelection {
    pub start: Point2<i32>,
    pub end: Point2<i32>
}

pub struct InfoOverlay {
    pub enabled: bool,
    pub screen_sel: Option<ScreenSelection>,
    last_guiding_blink_change: Option<std::time::Instant>,
    guiding_blink_state: Option<bool>,
}

impl InfoOverlay {
    pub fn new() -> InfoOverlay {
        InfoOverlay{
            enabled: true,
            screen_sel: None,
            last_guiding_blink_change: None,
            guiding_blink_state: None
        }
    }
}

pub fn draw_info_overlay(
    ctx: &cairo::Context,
    zoom: f64,
    program_data: &mut ProgramData
) {
    if !program_data.gui.as_ref().unwrap().info_overlay.enabled { return; }

    ctx.set_antialias(cairo::Antialias::None);

    let font_size = program_data.config.info_overlay_font_size().unwrap_or(DEFAULT_INFO_OVERLAY_FONT_SIZE);

    if let Some(sel) = &program_data.gui.as_ref().unwrap().info_overlay.screen_sel {
        draw_screen_selection(
            ctx, zoom, sel, SelectionType::from_mouse_mode(program_data.gui.as_ref().unwrap().mouse_mode), font_size
        );
    }

    if let Some(tracking) = &program_data.tracking {
        match tracking.mode {
            TrackingMode::Centroid(rect) => draw_centroid_rect(ctx, rect, zoom, font_size),
            TrackingMode::Anchor(pos) => draw_anchor(ctx, pos, zoom)
        }

        draw_tracking_target_pos(ctx, zoom, tracking.pos);

        if let Some(guiding_pos) = &program_data.mount_data.guiding_pos {
            let info_overlay = &mut program_data.gui.as_mut().unwrap().info_overlay;

            if program_data.mount_data.guide_slewing {
                match info_overlay.guiding_blink_state {
                    None => {
                        info_overlay.guiding_blink_state = Some(true);
                        info_overlay.last_guiding_blink_change = Some(std::time::Instant::now());
                    },
                    _ => ()
                }

                if *info_overlay.guiding_blink_state.as_ref().unwrap() == true &&
                info_overlay.last_guiding_blink_change.as_ref().unwrap().elapsed() >= GUIDING_BLINK_ON
                ||
                *info_overlay.guiding_blink_state.as_ref().unwrap() == false &&
                info_overlay.last_guiding_blink_change.as_ref().unwrap().elapsed() >= GUIDING_BLINK_OFF {

                    info_overlay.guiding_blink_state = Some(!info_overlay.guiding_blink_state.as_ref().unwrap());
                    info_overlay.last_guiding_blink_change = Some(std::time::Instant::now());
                }
            } else {
                info_overlay.guiding_blink_state = None;
                info_overlay.last_guiding_blink_change = None;
            }

            draw_guiding_info(ctx, zoom, *guiding_pos, tracking.pos, info_overlay.guiding_blink_state.unwrap_or(false));
        }

        if let Some(calibration) = &program_data.mount_data.calibration {
            draw_calibration(ctx, zoom, calibration, tracking.pos)
        }
    }

    if let Some(rect) = &program_data.crop_area {
        draw_crop_area(ctx, zoom, font_size, *rect);
    }

    if let Some(rect) = &program_data.histogram_area {
        draw_histogram_area(ctx, zoom, font_size, *rect);
    }
}

fn draw_guiding_info(ctx: &cairo::Context, zoom: f64, guiding_pos: Point2<i32>, tracking_pos: Point2<i32>, blink_on: bool) {
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.set_line_width(1.0);
    ctx.set_dash(&[6.0, 4.0], 0.0);
    ctx.arc(guiding_pos.x as f64 * zoom, guiding_pos.y as f64 * zoom, GUIDING_POS_CIRCLE_R, 0.0, 2.0 * std::f64::consts::PI);
    ctx.stroke().unwrap();

    if blink_on {
        ctx.set_dash(&[], 0.0);
        ctx.move_to(tracking_pos.x as f64 * zoom, tracking_pos.y as f64 * zoom);
        ctx.line_to(guiding_pos.x as f64 * zoom, guiding_pos.y as f64 * zoom);
        ctx.stroke().unwrap();
    }
}

fn draw_screen_selection(
    ctx: &cairo::Context,
    zoom: f64,
    sel: &ScreenSelection,
    sel_type: SelectionType,
    font_size: f64
) {
    let x1 = sel.start.x as f64 * zoom;
    let y1 = sel.start.y as f64 * zoom;
    let x2 = sel.end.x as f64 * zoom;
    let y2 = sel.end.y as f64 * zoom;

    match sel_type {
        SelectionType::Rectangle => {
            let pos_x = x1.min(x2);
            let pos_y = y1.min(y2);
            let width =  (x1 - x2).abs();
            let height = (y1 - y2).abs();
            ctx.set_source_rgba(1.0, 0.0, 0.0, 0.5);
            ctx.rectangle(pos_x, pos_y, width, height);
            ctx.fill().unwrap();

            ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            ctx.move_to(pos_x + font_size / 2.0, pos_y + 1.5 * font_size);
            ctx.set_font_size(font_size);
            let pix_width = (sel.start.x - sel.end.x).abs();
            let pix_height = (sel.start.y - sel.end.y).abs();
            let _ = ctx.show_text(&format!("{}x{}", pix_width, pix_height));
        },

        SelectionType::Line => {
           ctx.move_to(x1, y1);
           ctx.line_to(x2, y2);
           ctx.set_source_rgba(1.0, 0.0, 0.0, 0.5);
           let _ = ctx.stroke();
           ctx.set_source_rgba(1.0, 0.0, 0.0, 1.0);
           ctx.move_to(x1 + font_size / 2.0, y1 - font_size / 2.0);
           ctx.set_font_size(font_size);
           let dist = (sel.start.cast::<f64>().unwrap() - sel.end.cast::<f64>().unwrap()).magnitude();
           let _ = ctx.show_text(&format!("{}", dist as i32));
        }
    }
}

fn draw_calibration(ctx: &cairo::Context, zoom: f64, calibration: &MountCalibration, target_pos: Point2<i32>) {
    if calibration.primary_dir.is_some() && calibration.secondary_dir.is_some() { return; }

    ctx.set_line_width(1.0);
    ctx.set_source_rgb(1.0, 0.0, 0.0);

    let origin = (calibration.origin.x as f64 * zoom, calibration.origin.y as f64 * zoom);
    ctx.arc(origin.0, origin.1, 2.5, 0.0, 2.0 * std::f64::consts::PI);
    ctx.fill().unwrap();

    ctx.set_line_width(1.0);
    ctx.move_to(origin.0, origin.1);
    ctx.line_to(target_pos.x as f64 * zoom, target_pos.y as f64 * zoom);
    ctx.stroke().unwrap();
}

fn draw_crop_area(ctx: &cairo::Context, zoom: f64, font_size: f64, area: Rect) {
    ctx.set_line_width(1.0);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.rectangle(
        area.x as f64 * zoom,
        area.y as f64 * zoom,
        area.width as f64 * zoom,
        area.height as f64 * zoom
    );
    ctx.set_dash(&[6.0, 4.0], 0.0);
    ctx.stroke().unwrap();

    ctx.move_to(area.x as f64 * zoom, area.y as f64 * zoom - INFO_OVERLAY_LABEL_OFFSET as f64);
    ctx.set_font_size(font_size);
    ctx.show_text("CROP").unwrap();
    ctx.fill().unwrap();
}

fn draw_histogram_area(ctx: &cairo::Context, zoom: f64, font_size: f64, area: Rect) {
    ctx.set_line_width(1.0);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.rectangle(
        area.x as f64 * zoom,
        area.y as f64 * zoom,
        area.width as f64 * zoom,
        area.height as f64 * zoom
    );
    ctx.set_dash(&[6.0, 8.0], 0.0);
    ctx.stroke().unwrap();

    ctx.move_to(area.x as f64 * zoom, area.y as f64 * zoom - INFO_OVERLAY_LABEL_OFFSET as f64);
    ctx.set_font_size(font_size);
    ctx.show_text("HIST").unwrap();
    ctx.fill().unwrap();
}

fn draw_tracking_target_pos(ctx: &cairo::Context, zoom: f64, pos: Point2<i32>) {
    ctx.set_line_width(1.0);
    let pos_x = pos.x as f64 * zoom;
    let pos_y = pos.y as f64 * zoom;
    const CROSS_SIZE: f64 = 20.0;
    ctx.move_to(pos_x, pos_y);
    ctx.rel_line_to(-CROSS_SIZE / 2.0, 0.0);
    ctx.move_to(pos_x, pos_y);
    ctx.rel_line_to(CROSS_SIZE / 2.0, 0.0);
    ctx.move_to(pos_x, pos_y);
    ctx.rel_line_to(0.0, -CROSS_SIZE / 2.0);
    ctx.move_to(pos_x, pos_y);
    ctx.rel_line_to(0.0, CROSS_SIZE / 2.0);

    ctx.set_dash(&[], 0.0);
    ctx.stroke().unwrap();
}

fn draw_centroid_rect(ctx: &cairo::Context, rect: Rect, zoom: f64, font_size: f64) {
    ctx.set_line_width(1.0);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.rectangle(
        rect.x as f64 * zoom,
        rect.y as f64 * zoom,
        rect.width as f64 * zoom,
        rect.height as f64 * zoom
    );
    ctx.set_dash(&[1.0, 6.0], 0.0);
    ctx.stroke().unwrap();

    ctx.move_to(rect.x as f64 * zoom, rect.y as f64 * zoom - INFO_OVERLAY_LABEL_OFFSET as f64);
    ctx.set_font_size(font_size);
    ctx.show_text("CENTROID").unwrap();
    ctx.fill().unwrap();
}

fn draw_anchor(ctx: &cairo::Context, pos: Point2<i32>, zoom: f64) {
    ctx.set_line_width(1.0);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.arc(pos.x as f64 * zoom, pos.y as f64 * zoom, 32.0, 0.0, 6.0);
    ctx.stroke().unwrap();
}