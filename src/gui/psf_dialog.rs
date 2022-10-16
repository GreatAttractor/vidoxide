//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Point Spread Function (collimation helper) dialog.
//!

use cgmath::{EuclideanSpace, Point2};
use crate::ProgramData;
use ga_image::{ImageView, Image, PixelFormat};
use glib::clone;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

struct State {
    image: Option<cairo::ImageSurface>
}

pub struct PsfDialog {
    dialog: gtk::Dialog,
    drawing_area: gtk::DrawingArea,
    num_averaged: usize,
    num_to_average: usize,
    psf_size: u32,
    state: Rc<RefCell<State>>,
    averaged_img: Image
}

impl PsfDialog {
    pub fn new(
        parent: &gtk::ApplicationWindow,
        program_data_rc: &Rc<RefCell<ProgramData>>
    ) -> PsfDialog {
        let dialog = gtk::Dialog::with_buttons(
            Some("Point Spread Function"),
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
            image: None
        }));

        let drawing_area = init_controls(&dialog, &state);
        dialog.show_all();
        dialog.hide();

        const INITIAL_PSF_SIZE: u32 = 128;
        PsfDialog{
            dialog,
            drawing_area,
            state,
            num_averaged: 0,
            num_to_average: 10,
            psf_size: INITIAL_PSF_SIZE,
            averaged_img: Image::new(INITIAL_PSF_SIZE, INITIAL_PSF_SIZE, None, PixelFormat::Mono32f, None, true)
        }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn update(&mut self, image: &Image, area: Option<ga_image::Rect>) {
        if !self.dialog.is_visible() { return; }

        let area = match area { Some(r) => r, None => image.img_rect() };

        //TODO: use a subpixel centroid, use interpolation when adding images
        let centroid = Point2::from(image.centroid(Some(area))).cast::<i32>().unwrap();

        let mut converted = Image::new(self.psf_size, self.psf_size, None, PixelFormat::Mono32f, None, true);
        image.convert_pix_fmt_of_subimage_into(
            &mut converted,
            *(Point2::from(area.pos()) + (centroid - Point2{ x: self.psf_size as i32 / 2, y: self.psf_size as i32 / 2})).as_ref(),
            *Point2::origin().as_ref(),
            self.psf_size,
            self.psf_size,
            None
        );

        let height = self.averaged_img.height();

        for y in 0..height {
            let src_line = converted.line::<f32>(y);
            let dest_line = self.averaged_img.line_mut::<f32>(y);
            for (dest_val, src_val) in dest_line.iter_mut().zip(src_line.iter()) {
                *dest_val += src_val;
            }
        }

        self.num_averaged += 1;

        if self.num_averaged == self.num_to_average {
            let height = self.averaged_img.height();
            for y in 0..height {
                let line = self.averaged_img.line_mut::<f32>(y);
                for value in line.iter_mut() {
                    *value /= self.num_to_average as f32;
                }
            }

            let img_bgra24 = self.averaged_img.convert_pix_fmt(ga_image::PixelFormat::BGRA8, None);
            let stride = img_bgra24.bytes_per_line() as i32;
            let (width, height) = (img_bgra24.width() as i32, img_bgra24.height() as i32);
            self.state.borrow_mut().image = Some(cairo::ImageSurface::create_for_data(
                img_bgra24.take_pixel_data(),
                cairo::Format::Rgb24, // actually means: BGRA
                width,
                height,
                stride
            ).unwrap());

            self.drawing_area.queue_draw();

            self.averaged_img = Image::new(self.psf_size, self.psf_size, None, PixelFormat::Mono32f, None, true);
            self.num_averaged = 0;
        }

    }
}

fn init_controls(
    dialog: &gtk::Dialog,
    state: &Rc<RefCell<State>>
) -> gtk::DrawingArea {

    // control padding in pixels
    const PADDING: u32 = 10;

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let drawing_area = gtk::DrawingAreaBuilder::new().app_paintable(true).build();
    drawing_area.connect_draw(clone!(@weak state => @default-panic, move |area, ctx| {
        draw(ctx, area.allocated_width() as f64, area.allocated_height() as f64, &state);
        gtk::Inhibit(true)
    }));

    vbox.pack_start(&drawing_area, true, true, PADDING);

    dialog.content_area().pack_start(&vbox, true, true, PADDING);

    drawing_area
}

fn draw(ctx: &cairo::Context, width: f64, height: f64, state: &Rc<RefCell<State>>) {
    let state = state.borrow();
    if state.image.is_none() || width == 0.0 || height == 0.0 { return; }

    let image = state.image.as_ref().unwrap();

    //TODO: redraw only the invalidated areas (use `copy_clip_rectangle_list`)
    let scale_factor = image.width() as f64 / width;
    let source = cairo::SurfacePattern::create(image);

    source.set_matrix({
        let mut matrix = source.matrix();
        matrix.scale(scale_factor, scale_factor);
        matrix
    });
    source.set_filter(cairo::Filter::Bilinear);
    ctx.set_source(&source).unwrap();
    ctx.rectangle(0.0, 0.0, width / scale_factor, height / scale_factor);
    ctx.fill().unwrap();
}
