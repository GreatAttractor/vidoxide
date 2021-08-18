use crate::gui::{show_message, DialogDestroyer};
use crate::ProgramData;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub fn show_roi_dialog(parent: &gtk::ApplicationWindow)
-> Option<ga_image::point::Rect> {
    let dialog = gtk::Dialog::with_buttons(
        Some("Set ROI"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Accept), ("Cancel", gtk::ResponseType::Cancel)]
    );
    dialog.set_default_response(gtk::ResponseType::Accept);
    let _ddestr = DialogDestroyer::new(&dialog);

    dialog.content_area().pack_start(&gtk::Label::new(Some("Position is relative to the current ROI.")), false, true, PADDING);

    let add_entry = |label: &str| -> gtk::Entry {
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let entry = gtk::EntryBuilder::new()
            .input_purpose(gtk::InputPurpose::Digits)
            .text(&format!("{}", 0))
            .activates_default(true)
            .build();
        hbox.pack_start(&gtk::Label::new(Some(label)), false, false, PADDING);
        hbox.pack_start(&entry, true, true, PADDING);
        dialog.content_area().pack_start(&hbox, false, true, PADDING);

        entry
    };

    let entry_x_offset = add_entry("X offset:");
    let entry_y_offset = add_entry("Y offset:");
    let entry_width = add_entry("width:");
    let entry_height = add_entry("height:");

    dialog.show_all();

    loop {
        if dialog.run() == gtk::ResponseType::Accept {
            let x_offset = entry_x_offset.text().as_str().parse::<u32>();
            let y_offset = entry_y_offset.text().as_str().parse::<u32>();
            let width = entry_width.text().as_str().parse::<u32>();
            let height = entry_height.text().as_str().parse::<u32>();

            if x_offset.is_err() || y_offset.is_err() || width.is_err() || height.is_err() {
                show_message("Invalid value; expected non-negative integers.", "Error", gtk::MessageType::Error);
            } else {
                return Some(ga_image::point::Rect{
                    x: x_offset.unwrap() as i32,
                    y: y_offset.unwrap() as i32,
                    width: width.unwrap(),
                    height: height.unwrap()
                });
            }
        } else {
            return None;
        }
    }
}
