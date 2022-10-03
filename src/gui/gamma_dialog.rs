use crate::ProgramData;
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn create_gamma_dialog(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> gtk::Dialog {
    let dialog = gtk::Dialog::with_buttons(
        Some("Gamma correction"),
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
    /// Control padding in pixels.
    const PADDING: u32 = 10;

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    hbox.pack_start(&gtk::Label::new(Some("gamma")), false, false, PADDING);

    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.25, 4.0, 0.05);
    slider.set_value(1.0);
    slider.set_value_pos(gtk::PositionType::Right);
    slider.add_mark(1.0, gtk::PositionType::Bottom, Some("1.0"));
    slider.connect_value_changed(clone!(@weak program_data_rc => @default-panic, move |slider| {
        let mut pd = program_data_rc.borrow_mut();
        pd.gui.as_mut().unwrap().gamma_correction.gamma = slider.value() as f32;
        pd.gui.as_ref().unwrap().preview_area.refresh();
    }));
    hbox.pack_start(&slider, true, true, PADDING);

    let btn_reset = gtk::Button::with_label("reset");
    btn_reset.connect_clicked(clone!(@weak slider => @default-panic, move |_| {
        slider.set_value(1.0);
    }));
    hbox.pack_start(&btn_reset, false, false, PADDING);

    dialog.content_area().pack_start(&hbox, false, false, PADDING);


}
