use crate::ProgramData;
use crate::workers::controller::ControllerToMainThreadMsg;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct ControllerWidgets {
    device_list: gtk::ListBox
}

impl ControllerWidgets {
    pub fn add_device(&self, id: usize, name: &str) {
        //self.device_list.add(&gtk::Label::new(Some(&format!("{:08X} {}", id, name))));

        self.device_list.add(&gtk::ListBoxRow::builder()
            .child(&gtk::Label::new(Some(&format!("{:08X} {}", id, name))))
            .build());
    }
}


pub fn on_controller_event(msg: ControllerToMainThreadMsg, program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut pd = program_data_rc.borrow_mut();
    let gui = pd.gui.as_ref().unwrap();

    println!("received {:?}", msg);

    match msg {
        ControllerToMainThreadMsg::NewDevice(new_device) => {
            gui.controller_widgets.add_device(new_device.id, &new_device.name)
        },

        _ => ()
    }
}

pub fn create_controller_panel(program_data_rc: &Rc<RefCell<ProgramData>>) -> (gtk::Box, ControllerWidgets) {
    let box_all = gtk::Box::new(gtk::Orientation::Vertical, 0);

    box_all.pack_start(&gtk::Label::new(Some("Device list")), false, false, PADDING);

    let device_list = gtk::ListBox::builder().build();

    device_list.add(&gtk::Label::new(Some("aaa!")));
    device_list.add(&gtk::Label::new(Some("bbb!")));

    box_all.pack_start(&device_list, true, true, PADDING);

    (box_all, ControllerWidgets{ device_list })
}
