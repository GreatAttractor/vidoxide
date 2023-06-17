use crate::ProgramData;
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub fn create_preview_processing_dialog(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> gtk::Dialog {
    let dialog = gtk::Dialog::with_buttons(
        Some("Processing (preview only)"),
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

    init_controls(&dialog, program_data_rc);
    dialog.show_all();
    dialog.hide();

    dialog
}

fn init_controls(
    dialog: &gtk::Dialog,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    //dialog.content_area().pack_start(&create__controls(program_data_rc), false, false, PADDING);

    let stretch_checkbox = gtk::CheckButton::with_label("Stretch histogram");
    stretch_checkbox.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().preview_processing.stretch_histogram ^= true;
    }));
    dialog.content_area().pack_start(&stretch_checkbox, false, false, PADDING);

    dialog.content_area().pack_start(&create_gain_controls(program_data_rc), false, false, PADDING);
    dialog.content_area().pack_start(&create_gamma_controls(program_data_rc), false, false, PADDING);
}

fn create_gamma_controls(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Box {
    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    hbox.pack_start(&gtk::Label::new(Some("Gamma")), false, false, PADDING);

    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.25, 4.0, 0.05);
    slider.set_value(1.0);
    slider.set_value_pos(gtk::PositionType::Right);
    slider.add_mark(1.0, gtk::PositionType::Bottom, Some("1.0"));
    slider.connect_value_changed(clone!(@weak program_data_rc => @default-panic, move |slider| {
        let mut pd = program_data_rc.borrow_mut();
        pd.gui.as_mut().unwrap().preview_processing.gamma = slider.value() as f32;
        pd.gui.as_ref().unwrap().preview_area.refresh();
    }));
    hbox.pack_start(&slider, true, true, PADDING);

    let btn_reset = gtk::Button::with_label("reset");
    btn_reset.connect_clicked(clone!(@weak slider => @default-panic, move |_| {
        slider.set_value(1.0);
    }));
    hbox.pack_start(&btn_reset, false, false, PADDING);

    hbox
}

fn create_gain_controls(program_data_rc: &Rc<RefCell<ProgramData>>) -> gtk::Box {
    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    hbox.pack_start(&gtk::Label::new(Some("Gain (dB)")), false, false, PADDING);

    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, -4.0, 20.0, 0.05);
    slider.set_value(0.0);
    slider.set_value_pos(gtk::PositionType::Right);
    slider.add_mark(0.0, gtk::PositionType::Bottom, Some("0.0"));
    slider.connect_value_changed(clone!(@weak program_data_rc => @default-panic, move |slider| {
        let mut pd = program_data_rc.borrow_mut();
        pd.gui.as_mut().unwrap().preview_processing.gain = crate::gui::Decibel(slider.value() as f32);
        pd.gui.as_ref().unwrap().preview_area.refresh();
    }));
    hbox.pack_start(&slider, true, true, PADDING);

    let btn_reset = gtk::Button::with_label("reset");
    btn_reset.connect_clicked(clone!(@weak slider => @default-panic, move |_| {
        slider.set_value(0.0);
    }));
    hbox.pack_start(&btn_reset, false, false, PADDING);

    hbox
}
