use crate::ProgramData;
use crate::{workers, workers::controller::ControllerToMainThreadMsg};
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

        let (contents_box, widgets) = create_controls(parent, program_data_rc);
        dialog.content_area().pack_start(&contents_box, true, true, PADDING);

        dialog.show_all();
        dialog.hide();

        ControllerDialog { dialog, widgets }
    }

    pub fn show(&self) { self.dialog.show(); }

    pub fn add_device(&mut self, id: u64, name: &str) {
        self.widgets.device_list.add(&gtk::ListBoxRow::builder()
            .child(&gtk::Label::builder()
                .label(&format!("{} [{:016X}]", name, id))
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
            } else if let Some(sel_events) = &mut pd.sel_dialog_ctrl_events {
                sel_events.push(event);
            }
        },
    }
}

fn create_controls(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> (gtk::Box, Widgets) {
    let box_all = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin(PADDING as i32)
        .build();

    box_all.pack_start(
        &gtk::Label::builder().label("Input devices:").halign(gtk::Align::Start).build(),
        false, false, PADDING
    );

    let device_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    box_all.pack_start(&device_list, false, true, PADDING);

    let action_box = gtk::Box::new(gtk::Orientation::Vertical, PADDING as i32);

    let make_action_box = |text| {
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        hbox.pack_start(&gtk::Label::builder()
            .label(text)
            .halign(gtk::Align::Start)
            .margin_start(PADDING as i32)
            .margin_end(PADDING as i32)
            .build(),
            true, true, 0
        );

        let btn = gtk::Button::builder()
            .label("configure")
            .halign(gtk::Align::End)
            .build();
        btn.connect_clicked(clone!(@weak parent, @weak program_data_rc => @default-panic, move |_| {
            show_controller_action_selection_dialog(&parent, &program_data_rc);
        }));
        hbox.pack_start(&btn, false, false, 0);

        hbox
    };

    action_box.pack_start(&make_action_box("Mount axis 1 / positive"), false, true, 0);
    action_box.pack_start(&make_action_box("Mount axis 1 / negative"), false, true, 0);
    action_box.pack_start(&make_action_box("Mount axis 2 / positive"), false, true, 0);
    action_box.pack_start(&make_action_box("Mount axis 2 / negative"), false, true, 0);
    action_box.pack_start(&make_action_box("Focuser / in"), false, true, 0);
    action_box.pack_start(&make_action_box("Focuser / out"), false, true, 0);
    action_box.pack_start(&make_action_box("Recording start/stop"), false, true, 0);

    let actions = gtk::Frame::builder()
        .label("Actions")
        .child(&action_box)
        .build();
    box_all.pack_start(&actions, true, true, PADDING);

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

fn show_controller_action_selection_dialog(
    parent: &gtk::ApplicationWindow,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    let dialog = gtk::Dialog::with_buttons(
        Some("Choose controller action"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Ok), ("Cancel", gtk::ResponseType::Cancel)]
    );

    dialog.content_area().pack_start(
        &gtk::Label::new(Some("Press a controller button or perform an axis movement:")),
        true, true, PADDING
    );
    let action_label = gtk::Label::new(None);
    dialog.content_area().pack_start(&action_label, true, true, PADDING);
    dialog.show_all();

    program_data_rc.borrow_mut().sel_dialog_ctrl_events = Some(vec![]);

    let timer = Rc::new(crate::timer::Timer::new());
    let handler = clone!(@weak timer, @weak action_label, @weak program_data_rc => @default-panic, move || {
        if let Some(s) = choose_ctrl_action_based_on_events(&program_data_rc.borrow().sel_dialog_ctrl_events.as_ref().unwrap()) {
            action_label.set_text(&s);
        }
        program_data_rc.borrow_mut().sel_dialog_ctrl_events.as_mut().unwrap().clear();
    });
    timer.run(std::time::Duration::from_millis(500), false, handler);

    if let gtk::ResponseType::Ok = dialog.run() {
        //
    }

    dialog.close();
}

// TODO: use an action enum
fn choose_ctrl_action_based_on_events(events: &[workers::controller::StickEvent]) -> Option<String> {
    if events.is_empty() { return None; }

// analog controls
//
// TriggerL(f64),
// TriggerR(f64),
// JoyX(f64),
// JoyY(f64),
// JoyZ(f64),
// CamX(f64),
// CamY(f64),
// CamZ(f64),
// Slew(f64),
// Throttle(f64),
// ThrottleL(f64),
// ThrottleR(f64),
// Volume(f64),
// Wheel(f64),
// Rudder(f64),
// Gas(f64),
// Brake(f64),
// MouseX(f64),
// MouseY(f64),
// ScrollX(f64),
// ScrollY(f64),

    for event in events {
        match event.event {
            stick::Event::Exit(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionA(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionB(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionC(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionH(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionV(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionD(_) => return Some(format!("{:?}", event)),
            stick::Event::MenuL(_) => return Some(format!("{:?}", event)),
            stick::Event::MenuR(_) => return Some(format!("{:?}", event)),
            stick::Event::Joy(_) => return Some(format!("{:?}", event)),
            stick::Event::Cam(_) => return Some(format!("{:?}", event)),
            stick::Event::BumperL(_) => return Some(format!("{:?}", event)),
            stick::Event::BumperR(_) => return Some(format!("{:?}", event)),
            stick::Event::Up(_) => return Some(format!("{:?}", event)),
            stick::Event::Down(_) => return Some(format!("{:?}", event)),
            stick::Event::Left(_) => return Some(format!("{:?}", event)),
            stick::Event::Right(_) => return Some(format!("{:?}", event)),
            stick::Event::PovUp(_) => return Some(format!("{:?}", event)),
            stick::Event::PovDown(_) => return Some(format!("{:?}", event)),
            stick::Event::PovLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::PovRight(_) => return Some(format!("{:?}", event)),
            stick::Event::HatUp(_) => return Some(format!("{:?}", event)),
            stick::Event::HatDown(_) => return Some(format!("{:?}", event)),
            stick::Event::HatLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::HatRight(_) => return Some(format!("{:?}", event)),
            stick::Event::TrimUp(_) => return Some(format!("{:?}", event)),
            stick::Event::TrimDown(_) => return Some(format!("{:?}", event)),
            stick::Event::TrimLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::TrimRight(_) => return Some(format!("{:?}", event)),
            stick::Event::MicUp(_) => return Some(format!("{:?}", event)),
            stick::Event::MicDown(_) => return Some(format!("{:?}", event)),
            stick::Event::MicLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::MicRight(_) => return Some(format!("{:?}", event)),
            stick::Event::MicPush(_) => return Some(format!("{:?}", event)),
            stick::Event::Trigger(_) => return Some(format!("{:?}", event)),
            stick::Event::Bumper(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionM(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionL(_) => return Some(format!("{:?}", event)),
            stick::Event::ActionR(_) => return Some(format!("{:?}", event)),
            stick::Event::Pinky(_) => return Some(format!("{:?}", event)),
            stick::Event::PinkyForward(_) => return Some(format!("{:?}", event)),
            stick::Event::PinkyBackward(_) => return Some(format!("{:?}", event)),
            stick::Event::FlapsUp(_) => return Some(format!("{:?}", event)),
            stick::Event::FlapsDown(_) => return Some(format!("{:?}", event)),
            stick::Event::BoatForward(_) => return Some(format!("{:?}", event)),
            stick::Event::BoatBackward(_) => return Some(format!("{:?}", event)),
            stick::Event::AutopilotPath(_) => return Some(format!("{:?}", event)),
            stick::Event::AutopilotAlt(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineMotorL(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineMotorR(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineFuelFlowL(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineFuelFlowR(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineIgnitionL(_) => return Some(format!("{:?}", event)),
            stick::Event::EngineIgnitionR(_) => return Some(format!("{:?}", event)),
            stick::Event::SpeedbrakeBackward(_) => return Some(format!("{:?}", event)),
            stick::Event::SpeedbrakeForward(_) => return Some(format!("{:?}", event)),
            stick::Event::ChinaBackward(_) => return Some(format!("{:?}", event)),
            stick::Event::ChinaForward(_) => return Some(format!("{:?}", event)),
            stick::Event::Apu(_) => return Some(format!("{:?}", event)),
            stick::Event::RadarAltimeter(_) => return Some(format!("{:?}", event)),
            stick::Event::LandingGearSilence(_) => return Some(format!("{:?}", event)),
            stick::Event::Eac(_) => return Some(format!("{:?}", event)),
            stick::Event::AutopilotToggle(_) => return Some(format!("{:?}", event)),
            stick::Event::ThrottleButton(_) => return Some(format!("{:?}", event)),
            stick::Event::Mouse(_) => return Some(format!("{:?}", event)),
            stick::Event::Number(i8, bool) => return Some(format!("{:?}", event)),
            stick::Event::PaddleLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::PaddleRight(_) => return Some(format!("{:?}", event)),
            stick::Event::PinkyLeft(_) => return Some(format!("{:?}", event)),
            stick::Event::PinkyRight(_) => return Some(format!("{:?}", event)),
            stick::Event::Context(_) => return Some(format!("{:?}", event)),
            stick::Event::Dpi(_) => return Some(format!("{:?}", event)),
            stick::Event::Scroll(_) => return Some(format!("{:?}", event)),

            _ => continue
        }
    }

    None
}
