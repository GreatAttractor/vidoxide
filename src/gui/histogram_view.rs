//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Histogram view widget.
//!

use crate::workers::histogram::Histogram;
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct HistogramView {
    top_box: gtk::Box,
    info: gtk::Label,
    drawing_area: gtk::DrawingArea,
    histogram: Rc<RefCell<Histogram>>
}

impl HistogramView {
    pub fn new() -> HistogramView {
        let top_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let info = gtk::Label::new(None);
        info.set_xalign(0.1);
        top_box.pack_start(&info, false, false, PADDING);

        let histogram = Rc::new(RefCell::new(Histogram::new()));
        let drawing_area = gtk::DrawingAreaBuilder::new().app_paintable(true).build();
        drawing_area.connect_draw(clone!(@weak histogram => @default-panic, move |d_area, ctx| {
            draw_histogram(&histogram.borrow(), true /*make it a parameter*/, d_area, ctx);
            gtk::Inhibit(true)
        }));
        top_box.pack_start(&drawing_area, true, true, PADDING);

        HistogramView{
            top_box: top_box,
            info,
            drawing_area,
            histogram
        }
    }

    pub fn top_widget(&self) -> &gtk::Box { &self.top_box }

    pub fn set_histogram(&mut self, histogram: Histogram) {

        let first_nonzero_idx: Option<usize> = histogram.values()
            .iter()
            .enumerate()
            .find(|&(_, x)| (*x)[0] != 0 || (*x)[1] != 0 || (*x)[2] != 0)
            .map(|(i, _)| i);

        let last_nonzero_idx: Option<usize> = histogram.values()
            .iter()
            .rev()
            .enumerate()
            .find(|&(_, x)| (*x)[0] != 0 || (*x)[1] != 0 || (*x)[2] != 0)
            .map(|(i, _)| histogram.values().len() - 1 - i);

        if let Some(idx) = first_nonzero_idx {
            let min_p: f64 = idx as f64 / (histogram.values().len() - 1) as f64 * 100.0;
            let max_p: f64 = last_nonzero_idx.unwrap() as f64 / (histogram.values().len() - 1) as f64 * 100.0;
            self.info.set_text(&format!("min = {:.1}%  max = {:.1}%  Δ = {:.1}%", min_p, max_p, max_p - min_p));
        } else {
            self.info.set_text("");
        }

        *self.histogram.borrow_mut() = histogram;
        self.refresh();
    }

    pub fn refresh(&self) {
        self.drawing_area.queue_draw();
    }
}

fn draw_histogram(histogram: &Histogram, log_scale: bool, d_area: &gtk::DrawingArea, ctx: &cairo::Context) {
    ctx.set_antialias(cairo::Antialias::None);

    let gtk::Allocation{x: _, y: _, width, height} = d_area.get_allocation();

    let max_value = *histogram.values().iter().map(|x| x.iter().max().unwrap()).max().unwrap().max(&1);

    let mut log_values = [[0.0f32; 3]; 256];
    if max_value > 1 {
        for i in 0..256 {
            for j in 0..3 {
                log_values[i][j] = (histogram.values()[i][j].max(1) as f32).log2() / (max_value as f32).log2();
            }
        }
    }

    // draw histogram bars

    let brightness = 0.33;

    let num_buckets = histogram.values().len();

    if histogram.is_rgb() {

        // first, draw the black baground

        ctx.set_operator(cairo::Operator::Over);
        ctx.set_source_rgb(0.0, 0.0, 0.0);

        for (i, rgb) in histogram.values().iter().enumerate() {
            let max_bar_height = if log_scale {
                *log_values[i].iter().max_by(|a, b| a.partial_cmp(&b).unwrap()).unwrap() as f64 * height as f64
            } else {
                (*rgb.iter().max().unwrap() as i32 * height) as f64 / max_value as f64
            };

            ctx.rectangle(
                (i as i32 * width) as f64 / num_buckets as f64,
                height as f64 - max_bar_height,
                width as f64 / num_buckets as f64,
                max_bar_height
            );
        }

        ctx.fill();

        // second, draw the color bars additively

        ctx.set_operator(cairo::Operator::Add);

        let bar_colors = [
            [brightness, 0.0, 0.0],
            [0.0, brightness, 0.0],
            [0.0, 0.0, brightness]
        ];

        for channel in 0..3 {
            let color = &bar_colors[channel];
            ctx.set_source_rgba(color[0], color[1], color[2], 1.0);

            for (i, rgb) in histogram.values().iter().enumerate() {
                let bar_height = if log_scale {
                    log_values[i][channel] as f64 * height as f64
                } else {
                    (rgb[channel] as i32 * height) as f64 / max_value as f64
                };

                ctx.rectangle(
                    (i as i32 * width) as f64 / num_buckets as f64,
                    height as f64 - bar_height,
                    width as f64 / num_buckets as f64,
                    bar_height
                );
            }

            ctx.fill();
        }
    } else {
        ctx.set_source_rgb(brightness, brightness, brightness);
        for (i, rgb) in histogram.values().iter().enumerate() {
            let bar_height = if log_scale {
                log_values[i][0] as f64 * height as f64
            } else {
                (rgb[0] as i32 * height) as f64 / max_value as f64
            };

            ctx.rectangle(
                (i as i32 * width) as f64 / num_buckets as f64,
                height as f64 - bar_height,
                width as f64 / num_buckets as f64,
                bar_height
            );
        }
        ctx.fill();
    }

    // draw grid and info

    ctx.set_operator(cairo::Operator::Over);
    ctx.set_antialias(cairo::Antialias::Default);

    ctx.set_source_rgba(0.5, 0.5, 0.5, 0.2);
    for i in 1..=3 {
        ctx.move_to(i as f64 * width as f64 / 4.0, 0.0);
        ctx.line_to(i as f64 * width as f64 / 4.0, (height - 1) as f64);
    }
    ctx.stroke();

    ctx.set_source_rgba(1.0, 0.0, 0.0, 0.5);

    let first_nonzero_idx: Option<usize> = histogram.values()
        .iter()
        .enumerate()
        .find(|&(_, x)| x[0] != 0 || x[1] != 0 || x[2] != 0)
        .map(|(i, _)| i);

    let last_nonzero_idx: Option<usize> = histogram.values()
        .iter()
        .rev()
        .enumerate()
        .find(|&(_, x)| x[0] != 0 || x[1] != 0 || x[2] != 0)
        .map(|(i, _)| histogram.values().len() - 1 - i);

    if first_nonzero_idx.is_some() {
        let first_x = first_nonzero_idx.unwrap() as f64 / histogram.values().len() as f64 * width as f64;

        ctx.set_dash(&[5.0, 2.0], 0.0);
        ctx.set_line_width(2.0);
        ctx.move_to(first_x, 0.0);
        ctx.line_to(first_x, (height - 1) as f64);
        ctx.stroke();

        // ctx.move_to(first_x + 5.0, height as f64 / 5.0);
        // ctx.set_font_size(30.0/*TODO: configurable*/);
        // ctx.show_text(&format!("← {:.1}%", first_nonzero_idx.unwrap() as f64 / (histogram.values().len() - 1) as f64 * 100.0));
        // ctx.fill();

        let last_x = last_nonzero_idx.unwrap() as f64 / histogram.values().len() as f64 * width as f64;
        ctx.move_to(last_x, 0.0);
        ctx.line_to(last_x, (height - 1) as f64);
        ctx.stroke();

        // let right_text = format!("{:.1}% →", last_nonzero_idx.unwrap() as f64 / (histogram.values().len() - 1) as f64 * 100.0);
        // ctx.move_to(last_x - 5.0 - ctx.text_extents(&right_text).width, height as f64 / 5.0 + 35.0);
        // ctx.set_font_size(30.0/*TODO: configurable*/);
        // ctx.show_text(&right_text);
        // ctx.fill();
    } else {
        //
    }
}
