use crate::ProgramData;
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn create_reticle_dialog(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    opacity: f64,
    diameter: f64,
    step: f64,
    line_width: f64
) -> gtk::Dialog {
    let dialog = gtk::Dialog::with_buttons(
        Some("Reticle settings"),
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

    init_controls(&dialog, program_data_rc, opacity, diameter, step, line_width);
    dialog.show_all();
    dialog.hide();

    dialog
}

fn init_controls(
    dialog: &gtk::Dialog,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    opacity: f64,
    diameter: f64,
    step: f64,
    line_width: f64
) {
    /// Control padding in pixels.
    const PADDING: u32 = 10;

    //TODO: finish implementing and sync with the menu item (use actions?)
    // let btn_toggle = gtk::CheckButtonBuilder::new().label("Reticle enabled").active(false).build();
    // dialog.content_area().pack_start(&btn_toggle, false, false, PADDING);

    let add_slider = |label, min_val, max_val, step, current_val, action: fn(&mut crate::gui::Reticle, f64)| {
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hbox.pack_start(&gtk::Label::new(Some(label)), false, false, PADDING);
        let slider = gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            min_val,
            max_val,
            step
        );
        slider.set_value(current_val);
        slider.connect_value_changed(clone!(@weak program_data_rc => @default-panic, move |slider| {
            let mut pd = program_data_rc.borrow_mut();
            action(&mut pd.gui.as_mut().unwrap().reticle, slider.value());
            pd.gui.as_ref().unwrap().preview_area.refresh();
        }));
        hbox.pack_start(&slider, true, true, PADDING);
        dialog.content_area().pack_start(&hbox, false, false, PADDING);
    };

    add_slider("Diameter:", 10.0, 500.0, 1.0, diameter, |reticle, new_value| { reticle.diameter = new_value; });

    add_slider("Opacity:", 0.1, 1.0, 0.01, opacity, |reticle, new_value| { reticle.opacity = new_value; });

    add_slider("Step:", 10.0, 100.0, 0.5, step, |reticle, new_value| { reticle.step = new_value; });

    add_slider("Line width:", 1.0, 8.0, 0.2, line_width, |reticle, new_value| { reticle.line_width = new_value; });
}
