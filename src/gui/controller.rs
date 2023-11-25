use crate::ProgramData;
use crate::workers::controller::ControllerToMainThreadMsg;
use gtk::glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

struct Widgets {
    device_list: gtk::ListBox
}

pub struct ControllerDialog {
    dialog: gtk::Dialog,
    widgets: Widgets
}

impl ControllerDialog {
    pub fn new(
        parent: &gtk::ApplicationWindow,
        program_data_rc: &Rc<RefCell<ProgramData>>
    ) -> ControllerDialog {
        let dialog = gtk::Dialog::with_buttons(
            Some("Controller settings"),
            Some(parent),
            gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[("OK", gtk::ResponseType::Close)]
        );

        dialog.set_default_response(gtk::ResponseType::Close);

        dialog.connect_response(|dialog, response| {
            if response == gtk::ResponseType::Close { dialog.hide(); }
        });

        dialog.connect_delete_event(|dialog, _| {
            dialog.hide();
            gtk::Inhibit(true)
        });

        let (contents_box, widgets) = create_controls();
        dialog.content_area().pack_start(&contents_box, true, true, PADDING);

        dialog.show_all();
        dialog.hide();

        ControllerDialog { dialog, widgets }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn add_device(&mut self, id: u64, name: &str) {
        self.widgets.device_list.add(&gtk::ListBoxRow::builder()
            .child(&gtk::Label::builder()
                .label(&format!("[{:016X}] {}", id, name))
                .halign(gtk::Align::Start)
                .visible(true)
                .build()
            )
            .visible(true)
            .build());
    }

    pub fn remove_device(&mut self, index: usize) {
        self.widgets.device_list.remove(&self.widgets.device_list.row_at_index(index as i32).unwrap());
    }
}

pub fn on_controller_event(msg: ControllerToMainThreadMsg, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut pd = program_data_rc.borrow_mut();
    let gui = pd.gui.as_mut().unwrap();

    println!("received {:?}", msg);

    match msg {
        ControllerToMainThreadMsg::NewDevice(new_device) => {
            gui.controller_dialog.add_device(new_device.id, &new_device.name);
        },

        ControllerToMainThreadMsg::StickEvent(event) => {
            if let stick::Event::Disconnect = event.event {
                gui.controller_dialog.remove_device(event.index);
            }
        },
    }
}

fn create_controls() -> (gtk::Box, Widgets) {
    let box_all = gtk::Box::new(gtk::Orientation::Vertical, 0);

    box_all.pack_start(
        &gtk::Label::builder().label("Input devices").halign(gtk::Align::Start).build(),
        false, false, PADDING
    );

    let device_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    box_all.pack_start(&device_list, true, true, PADDING);

    //

    (box_all, Widgets{ device_list })
}

pub fn init_controller_menu(
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> gtk::Menu {
    let menu = gtk::Menu::new();

    let item_settings = gtk::MenuItem::with_label("Settings...");
    item_settings.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow().gui.as_ref().unwrap().controller_dialog.show();
    }));
    menu.append(&item_settings);

    menu
}
