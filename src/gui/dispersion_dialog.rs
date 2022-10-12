use cgmath::{EuclideanSpace, Point2, Vector2, Zero};
use crate::ProgramData;
use ga_image::{Image, ImageView};
use gtk::cairo;
use gtk::prelude::*;
use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;

const RED: usize = 0;
const GREEN: usize = 1;
const BLUE: usize = 2;

struct State {
    /// Offset of red channel centroid relative to green channel centroid.
    red_offset: Vector2<f64>,

    /// Offset of blue channel centroid relative to green channel centroid.
    blue_offset: Vector2<f64>,

    last_red_offset: Vector2<f64>,

    last_blue_offset: Vector2<f64>,

    num_averaged: usize,

    num_to_average: usize,

    logical_display_size: f64
}

pub struct DispersionDialog {
    dialog: gtk::Dialog,
    drawing_area: gtk::DrawingArea,
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
            logical_display_size: 12.0,
            red_offset: Vector2::zero(),
            blue_offset: Vector2::zero(),
            last_red_offset: Vector2::zero(),
            last_blue_offset: Vector2::zero(),
            num_averaged: 0,
            num_to_average: 10
        }));

        let drawing_area = init_controls(&dialog, program_data_rc, &state);
        dialog.show_all();
        dialog.hide();

        DispersionDialog{ dialog, drawing_area, state }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn is_visible(&self) -> bool { self.dialog.is_visible() }

    pub fn update(&self, image: &ImageView) {
        if !self.dialog.is_visible() { return; }

        let pix_fmt = image.pixel_format();
        if !(pix_fmt.num_channels() == 3 /*TODO: explicitly check for RGB instead*/ || pix_fmt.is_cfa()) {
            let mut state = self.state.borrow_mut();
            state.num_averaged = 0;
            state.red_offset = Vector2::zero();
            state.blue_offset = Vector2::zero();
            return;
        }

        let rgb_centroids = get_rgb_centroids(image);
        let mut state = self.state.borrow_mut();
        state.red_offset += rgb_centroids[RED] - rgb_centroids[GREEN];
        state.blue_offset += rgb_centroids[BLUE] - rgb_centroids[GREEN];
        state.num_averaged += 1;
        if state.num_averaged == state.num_to_average {
            let n = state.num_to_average as f64;
            state.last_red_offset = state.red_offset / n;
            state.last_blue_offset = state.blue_offset / n;
            state.red_offset = Vector2::zero();
            state.blue_offset = Vector2::zero();
            self.drawing_area.queue_draw();
            state.num_averaged = 0;
        }
    }
}

fn init_controls(
    dialog: &gtk::Dialog,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    state: &Rc<RefCell<State>>
) -> gtk::DrawingArea {
    //TODO: force draw area background to black (?) via CSS provider

    /// Control padding in pixels.
    const PADDING: u32 = 10;

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let drawing_area = gtk::DrawingAreaBuilder::new().app_paintable(true).build();
    drawing_area.connect_draw(clone!(@weak state => @default-panic, move |area, ctx| {
        draw(ctx, (area.allocated_width(), area.allocated_height()), &state);
        gtk::Inhibit(true)
    }));

    vbox.pack_start(&drawing_area, true, true, PADDING);

    dialog.content_area().pack_start(&vbox, true, true, PADDING);

    drawing_area
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

    ctx.set_line_width(2.5 / s);
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.move_to(0.0, 0.0);
    ctx.line_to(state.last_red_offset.x, state.last_red_offset.y);
    ctx.stroke().unwrap();

    ctx.set_line_width(2.5 / s);
    ctx.set_source_rgb(0.3, 0.3, 1.0);
    ctx.move_to(0.0, 0.0);
    ctx.line_to(state.last_blue_offset.x, state.last_blue_offset.y);
    ctx.stroke().unwrap();
}

// TODO: move it to ga_image?
fn get_rgb_centroids(image: &ImageView) -> [Point2<f64>; 3] {
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
