use cgmath::{Vector2, Zero};
use crate::ProgramData;
use ga_image::{Image, ImageView};
use gtk::cairo;
use gtk::prelude::*;
use glib::clone;
use std::cell::RefCell;
use std::rc::Rc;

struct State {
    /// Offset of red channel centroid relative to green channel centroid.
    red_offset: Vector2<f64>,

    /// Offset of blue channel centroid relative to green channel centroid.
    blue_offset: Vector2<f64>,

    num_averaged: usize,

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
            // red_offset: Vector2::zero(),
            // blue_offset: Vector2::zero(),

            //TESTING #########
            red_offset: Vector2{ x: 2.0, y: 3.0 },
            blue_offset: Vector2{ x: -3.0, y: -5.0 },
            //END TESTING #####

            num_averaged: 0
        }));

        let drawing_area = init_controls(&dialog, program_data_rc, &state);
        dialog.show_all();
        dialog.hide();

        DispersionDialog{ dialog, drawing_area, state }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn update(&mut self, image: &ImageView) {
        //
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
    ctx.line_to(state.red_offset.x, state.red_offset.y);
    ctx.stroke().unwrap();

    ctx.set_line_width(2.5 / s);
    ctx.set_source_rgb(0.3, 0.3, 1.0);
    ctx.move_to(0.0, 0.0);
    ctx.line_to(state.blue_offset.x, state.blue_offset.y);
    ctx.stroke().unwrap();
}
