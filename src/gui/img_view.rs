//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Image view widget.
//!

use crate::gui::{MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT};
use cgmath::{Point2, Vector2, Zero};
use gtk::{cairo, gdk};
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

struct State {
    image: Option<cairo::ImageSurface>,
    zoom: f64,
    drag_start_pos: Option<(f64, f64)>,
    stabilization_offset: Vector2<i32>
}

/// Displays an image (stored in `state`) and handles scrolling with right mouse button and zooming with mouse wheel.
pub struct ImgView {
    top_widget: gtk::ScrolledWindow,
    drawing_area: gtk::DrawingArea,
    state: Rc<RefCell<State>>
}

impl ImgView {
    ///
    /// Constructor.
    ///
    /// # Parameters
    ///
    /// * `on_button_down` - Called on left mouse button press; receives image coordinates
    ///   (scrolling and zoom are applied).
    /// * `on_button_up` - Called on left mouse button release; receives image coordinates
    ///   (scrolling and zoom are applied).
    /// * `on_mouse_move` - Called on mouse move; receives image coordinates (scrolling and zoom are applied).
    /// * `draw_info_overlay` - Called after the image is drawn. Receives drawing context and zoom value.
    /// * `draw_reticle` - Called after the image is drawn. Receives drawing context having its origin in the middle
    ///    of the captured image.
    ///
    pub fn new(
        on_button_down: Box<dyn Fn(Point2<i32>)>,
        on_button_up: Box<dyn Fn(Point2<i32>)>,
        on_mouse_move: Box<dyn Fn(Point2<i32>)>,
        draw_info_overlay: Box<dyn Fn(&cairo::Context, f64)>,
        draw_reticle: Box<dyn Fn(&cairo::Context)>
    ) -> ImgView {
        let top_widget = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
        let drawing_area = gtk::DrawingAreaBuilder::new().app_paintable(true).build();
        let evt_box = gtk::EventBox::new();
        evt_box.add(&drawing_area);
        top_widget.add(&evt_box);

        let state = Rc::new(RefCell::new(State{
            image: None,
            zoom: 1.0,
            drag_start_pos: None,
            stabilization_offset: Vector2::zero()
        }));

        evt_box.set_events(
            gdk::EventMask::POINTER_MOTION_MASK |
            gdk::EventMask::BUTTON_PRESS_MASK |
            gdk::EventMask::BUTTON_RELEASE_MASK |
            gdk::EventMask::SCROLL_MASK
        );

        evt_box.connect_button_press_event(clone!(@weak state => @default-panic, move |_, evt| {
            if evt.button() == MOUSE_BUTTON_LEFT {
                let image_pos = {
                    let (pos_x, pos_y) = evt.position();
                    let zoom = state.borrow().zoom;
                    Point2{ x: (pos_x / zoom) as i32, y: (pos_y / zoom) as i32 }
                };
                on_button_down(image_pos);
            }

            if evt.button() == MOUSE_BUTTON_RIGHT {
                state.borrow_mut().drag_start_pos = Some(evt.position());
            }

            gtk::Inhibit(true)
        }));

        evt_box.connect_button_release_event(clone!(@weak state => @default-panic, move |_, evt| {
            if evt.button() == MOUSE_BUTTON_LEFT {
                let image_pos = {
                    let (pos_x, pos_y) = evt.position();
                    let zoom = state.borrow().zoom;
                    Point2{ x: (pos_x / zoom) as i32, y: (pos_y / zoom) as i32 }
                };
                on_button_up(image_pos);
            }

            if evt.button() == MOUSE_BUTTON_RIGHT {
                state.borrow_mut().drag_start_pos = None;
            }

            gtk::Inhibit(true)
        }));

        evt_box.connect_motion_notify_event(clone!(@weak state, @weak top_widget => @default-panic, move |_, evt| {
            let image_pos = {
                let (pos_x, pos_y) = evt.position();
                let zoom = state.borrow().zoom;
                Point2{ x: (pos_x / zoom) as i32, y: (pos_y / zoom) as i32 }
            };

            on_mouse_move(image_pos);

            let state = state.borrow();
            if let Some(pos) = &state.drag_start_pos {
                let new_horz = top_widget.hadjustment();
                let new_vert = top_widget.vadjustment();

                new_horz.set_value(top_widget.hadjustment().value() + pos.0 - evt.position().0);
                new_vert.set_value(top_widget.vadjustment().value() + pos.1 - evt.position().1);

                top_widget.set_hadjustment(Some(&new_horz));
                top_widget.set_vadjustment(Some(&new_vert));
            }

            gtk::Inhibit(true)
        }));

        evt_box.connect_scroll_event(clone!(
            @weak state,
            @weak top_widget,
            @weak drawing_area
            => @default-panic,
            move |_, evt| {
                if state.borrow().image.is_none() { return gtk::Inhibit(true); }

                // ensures zooming happens around the current mouse position
                ImgView::on_mouse_wheel(evt, &state, &top_widget, &drawing_area);

                gtk::Inhibit(true)
            }
        ));

        drawing_area.connect_draw(clone!(@weak state, @weak top_widget => @default-panic, move |_, ctx| {
            let state = state.borrow();

            ctx.transform({
                let mut m = cairo::Matrix::identity();
                m.translate(
                    -state.stabilization_offset.x as f64 * state.zoom,
                    -state.stabilization_offset.y as f64 * state.zoom
                );
                m
            });

            match &state.image {
                Some(surface) => {
                    let source = cairo::SurfacePattern::create(&surface);
                    source.set_matrix({
                        let mut matrix = cairo::Matrix::identity();
                        matrix.scale(1.0 / state.zoom, 1.0 / state.zoom);
                        matrix
                    });
                    source.set_filter(cairo::Filter::Bilinear);
                    ctx.set_source(&source).unwrap();
                    //TODO: redraw only the invalidated areas (use `copy_clip_rectangle_list`)
                    ctx.rectangle(
                        0.0, 0.0,
                        surface.width() as f64 * state.zoom,
                        surface.height() as f64 * state.zoom
                    );
                    ctx.fill().unwrap();

                    draw_info_overlay(ctx, state.zoom);

                    ctx.translate(
                        state.zoom * surface.width() as f64 / 2.0,
                        state.zoom * surface.height() as f64 / 2.0
                   );
                   draw_reticle(ctx);
                },
                None => ()
            }

            gtk::Inhibit(true)
        }));

        ImgView{
            top_widget,
            drawing_area,
            state
        }
    }

    pub fn get_zoom(&self) -> f64 {
        self.state.borrow().zoom
    }

    fn set_zoom_priv(state: &mut State, drawing_area: &gtk::DrawingArea, zoom: f64) {
        state.zoom = zoom;
        if let Some(image) = &state.image {
            drawing_area.set_size_request(
                (image.width() as f64 * zoom) as i32,
                (image.height() as f64 * zoom) as i32
            );
        }
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        ImgView::set_zoom_priv(&mut self.state.borrow_mut(), &self.drawing_area, zoom);
        if self.state.borrow().image.is_some() {
            self.refresh();
        }
    }

    pub fn change_zoom(&mut self, factor: f64) {
        let zoom = self.state.borrow().zoom;
        self.set_zoom(zoom * factor);
    }

    pub fn set_image(&self, image: cairo::ImageSurface, stabilization_offset: Vector2<i32>) {
        let mut state = self.state.borrow_mut();
        self.drawing_area.set_size_request(
            (image.width() as f64 * state.zoom) as i32,
            (image.height() as f64 * state.zoom) as i32
        );
        state.image = Some(image);
        state.stabilization_offset = stabilization_offset;
        self.drawing_area.queue_draw();
    }

    pub fn top_widget(&self) -> &gtk::ScrolledWindow { &self.top_widget }

    pub fn refresh(&self) {
        self.drawing_area.queue_draw();
    }

    pub fn image_size(&self) -> Option<(i32, i32)> {
        match &self.state.borrow().image {
            Some(image) => Some((image.width(), image.height())),
            None => None
        }
    }

    pub fn scroll_pos(&self) -> Point2<i32> {
        Point2{
            x: self.top_widget.hadjustment().value() as i32,
            y: self.top_widget.vadjustment().value() as i32
        }
     }

     fn on_mouse_wheel(
         evt: &gdk::EventScroll,
         state: &Rc<RefCell<State>>,
         scroll_wnd: &gtk::ScrolledWindow,
         drawing_area: &gtk::DrawingArea
     ) {
        let prev_zoom = state.borrow().zoom;
        let new_zoom = match evt.direction() {
            gdk::ScrollDirection::Up => (state.borrow().zoom * super::ZOOM_CHANGE_FACTOR).min(super::MAX_ZOOM),
            gdk::ScrollDirection::Down => (state.borrow().zoom / super::ZOOM_CHANGE_FACTOR).max(super::MIN_ZOOM),
            _ => state.borrow().zoom
        };

        let prev_scroll_pos_x = scroll_wnd.hadjustment().value();
        let prev_scroll_pos_y = scroll_wnd.vadjustment().value();

        let delta_x = evt.position().0 - prev_scroll_pos_x;
        let delta_y = evt.position().1 - prev_scroll_pos_y;

        ImgView::set_zoom_priv(&mut state.borrow_mut(), &drawing_area, new_zoom);

        // The above call (which performs `set_size_request` on `drawing_area`) does not resize
        // the `scroll_wnd`'s `Adjustment`s yet; since we already need to set their post-resize scroll
        // positions, we reinitialize them ourselves.
        let adj = scroll_wnd.hadjustment();
        adj.set_upper(
            state.borrow().image.as_ref().unwrap().width() as f64 * new_zoom
        );
        adj.set_value(
            new_zoom / prev_zoom * (prev_scroll_pos_x + delta_x) - delta_x
        );
        scroll_wnd.set_hadjustment(Some(&adj));

        let adj = scroll_wnd.vadjustment();
        adj.set_upper(
            state.borrow().image.as_ref().unwrap().height() as f64 * new_zoom
        );
        adj.set_value(
            new_zoom / prev_zoom * (prev_scroll_pos_y + delta_y) - delta_y
        );
        scroll_wnd.set_vadjustment(Some(&adj));

        drawing_area.queue_draw();
     }
}
