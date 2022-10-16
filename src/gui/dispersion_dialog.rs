//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Atmospheric dispersion dialog.
//!

use cgmath::{EuclideanSpace, InnerSpace, Point2, Vector2, Zero};
use crate::ProgramData;
use gtk::cairo;
use gtk::prelude::*;
use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;

const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;

struct State {
    /// Averaged offset of red channel centroid relative to green channel centroid.
    avg_red_offset: Vector2<f64>,
    /// Averaged offset of blue channel centroid relative to green channel centroid.
    avg_blue_offset: Vector2<f64>,
    logical_display_size: f64
}

pub struct DispersionDialog {
    red_offset: Vector2<f64>,
    blue_offset: Vector2<f64>,
    num_averaged: usize,
    num_to_average: usize,
    dialog: gtk::Dialog,
    drawing_area: gtk::DrawingArea,
    red_offset_label: gtk::Label,
    blue_offset_label: gtk::Label,
    state: Rc<RefCell<State>>
}

impl DispersionDialog {
    pub fn new(
        parent: &gtk::ApplicationWindow,
        program_data_rc: &Rc<RefCell<ProgramData>>
    ) -> DispersionDialog {
        let dialog = gtk::Dialog::with_buttons(
            Some("Atmospheric dispersion"),
            Some(parent),
            gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[("Close", gtk::ResponseType::Close)]
        );

        dialog.set_default_response(gtk::ResponseType::Close);

        dialog.connect_response(|dialog, response| {
            if response == gtk::ResponseType::Close { dialog.hide(); }
        });

        dialog.connect_delete_event(|dialog, _| {
            dialog.hide();
            gtk::Inhibit(true)
        });

        let state = Rc::new(RefCell::new(State{
            logical_display_size: 5.0,
            avg_red_offset: Vector2::zero(),
            avg_blue_offset: Vector2::zero()
        }));

        let (drawing_area, red_offset_label, blue_offset_label) = init_controls(&dialog, program_data_rc, &state);
        dialog.show_all();
        dialog.hide();

        DispersionDialog{
            num_averaged: 0,
            num_to_average: 10,
            red_offset: Vector2::zero(),
            blue_offset: Vector2::zero(),
            dialog,
            drawing_area,
            red_offset_label,
            blue_offset_label,
            state
        }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn is_visible(&self) -> bool { self.dialog.is_visible() }

    pub fn update(&mut self, image: &ga_image::ImageView) {
        if !self.dialog.is_visible() { return; }

        let pix_fmt = image.pixel_format();
        if !(pix_fmt.num_channels() == 3 /*TODO: explicitly check for RGB instead*/ || pix_fmt.is_cfa()) {
            let mut state = self.state.borrow_mut();
            self.num_averaged = 0;
            self.red_offset = Vector2::zero();
            self.blue_offset = Vector2::zero();
            state.avg_red_offset = Vector2::zero();
            state.avg_blue_offset = Vector2::zero();
            return;
        }

        let rgb_centroids = get_rgb_centroids(image);
        let mut state = self.state.borrow_mut();
        self.red_offset += rgb_centroids[RED] - rgb_centroids[GREEN];
        self.blue_offset += rgb_centroids[BLUE] - rgb_centroids[GREEN];
        self.num_averaged += 1;
        if self.num_averaged == self.num_to_average {
            let n = self.num_averaged as f64;
            state.avg_red_offset = self.red_offset / n;
            state.avg_blue_offset = self.blue_offset / n;
            self.red_offset_label.set_label(&format!("R: {:.1} px", state.avg_red_offset.magnitude()));
            self.blue_offset_label.set_label(&format!("B: {:.1} px", state.avg_blue_offset.magnitude()));
            self.red_offset = Vector2::zero();
            self.blue_offset = Vector2::zero();
            self.drawing_area.queue_draw();
            self.num_averaged = 0;
        }
    }
}

fn init_controls(
    dialog: &gtk::Dialog,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    state: &Rc<RefCell<State>>
) -> (gtk::DrawingArea, gtk::Label, gtk::Label) {
    //TODO: force draw area background to black (?) via CSS provider

    // control padding in pixels
    const PADDING: u32 = 10;

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let drawing_area = gtk::DrawingAreaBuilder::new().app_paintable(true).build();
    drawing_area.connect_draw(clone!(@weak state => @default-panic, move |area, ctx| {
        draw(ctx, (area.allocated_width(), area.allocated_height()), &state);
        gtk::Inhibit(true)
    }));

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let label = gtk::Label::new(Some("scale:"));
    hbox.pack_start(&label, false, false, PADDING);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 2.0, 50.0, 1.0);
    scale.set_value(state.borrow().logical_display_size);
    scale.connect_value_changed(clone!(@weak state, @weak drawing_area => @default-panic, move |slider| {
        state.borrow_mut().logical_display_size = slider.value();
        drawing_area.queue_draw();
    }));
    hbox.pack_start(&scale, true, true, PADDING);
    vbox.pack_start(&hbox, false, true, PADDING);

    let red_offset = gtk::Label::new(Some("R: 0.0 px"));
    red_offset.set_halign(gtk::Align::Start);
    vbox.pack_start(&red_offset, false, false, PADDING);

    let blue_offset = gtk::Label::new(Some("B: 0.0 px"));
    blue_offset.set_halign(gtk::Align::Start);
    vbox.pack_start(&blue_offset, false, false, PADDING);

    vbox.pack_start(&drawing_area, true, true, PADDING);

    dialog.content_area().pack_start(&vbox, true, true, PADDING);

    (drawing_area, red_offset, blue_offset)
}

//TODO: draw arrow heads
fn draw(ctx: &cairo::Context, widget_size: (i32, i32), state: &Rc<RefCell<State>>) {
    if widget_size.1 == 0 { return; }

    let state = state.borrow();

    ctx.set_dash(&[], 0.0);
    ctx.set_antialias(cairo::Antialias::Default);

    ctx.translate(
        widget_size.0 as f64 / 2.0,
        widget_size.1 as f64 / 2.0
    );

    ctx.set_line_width(1.0);
    ctx.set_source_rgb(0.0, 1.0, 0.0);
    let radius = 10.0;
    ctx.arc(0.0, 0.0, radius, 0.0, 2.0 * std::f64::consts::PI);
    ctx.stroke().unwrap();

    let s = widget_size.0 as f64 / state.logical_display_size;
    ctx.scale(s, s);

    const LINE_WIDTH: f64 = 5.0;

    ctx.set_line_width(LINE_WIDTH / s);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.move_to(0.0, 0.0);
    ctx.line_to(state.avg_red_offset.x, state.avg_red_offset.y);
    ctx.stroke().unwrap();

    ctx.set_line_width(LINE_WIDTH / s);
    ctx.set_source_rgb(0.3, 0.3, 1.0);
    ctx.move_to(0.0, 0.0);
    ctx.line_to(state.avg_blue_offset.x, state.avg_blue_offset.y);
    ctx.stroke().unwrap();
}

// TODO: move it to ga_image?
fn get_rgb_centroids(image: &ga_image::ImageView) -> [Point2<f64>; 3] {
    let pix_fmt = image.pixel_format();
    if !(pix_fmt.is_cfa() && pix_fmt.bytes_per_channel() == 1) {
        return [Point2::origin(), Point2::origin(), Point2::origin()];

        //TODO: implement it
    }

    // values below are for (R, G, B) channels
    let mut m00 = [0.0; 3]; // image moment 00, i.e. sum of pixels' brightness
    let mut m10 = [0.0; 3]; // image moment 10
    let mut m01 = [0.0; 3]; // image moment 01

    let r_col_ofs = pix_fmt.cfa_pattern().red_col_ofs();
    let b_col_ofs = (r_col_ofs + 1) % 2;
    let r_row_ofs = pix_fmt.cfa_pattern().red_row_ofs();

    for y in 0..image.height() {
        let line = image.line::<u8>(y);

        // non-green channel being calculated for the current row
        let rb_calc_channel = if y % 2 == r_row_ofs as u32 { RED } else { BLUE };

        let rb_col_ofs = if rb_calc_channel == RED { r_col_ofs } else { b_col_ofs };

        for x in (0..image.width() - 1).step_by(2) {
            let rb_x = x as usize + rb_col_ofs;
            let g_x = x as usize + (rb_col_ofs + 1) % 2;

            let rb_value = line[rb_x] as f64;
            let g_value = line[g_x] as f64;

            m00[rb_calc_channel] += rb_value;
            m00[GREEN] += g_value;

            m10[rb_calc_channel] += rb_x as f64 * rb_value;
            m10[GREEN] += rb_x as f64 * g_value;

            m01[rb_calc_channel] += y as f64 * rb_value;
            m01[GREEN] += y as f64 * g_value;
        }
    }

    let mut result = [Point2::origin(); 3];

    for ch in 0..3 {
        if m00[ch] == 0.0 {
            result[ch] = Point2{ x: image.width() as f64 / 2.0, y: image.height() as f64 / 2.0 };
        } else {
            result[ch] = Point2{ x: m10[ch] / m00[ch], y: m01[ch] / m00[ch] };
        }
    }

    result
}
