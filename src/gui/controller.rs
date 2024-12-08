//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Controller GUI.
//!

use crate::{
    controller::{SourceAction, TargetAction, choose_ctrl_action_based_on_events},
    gui::checked_listbox::CheckedListBox,
    workers,
    workers::controller::ControllerToMainThreadMsg, ProgramData
};
use gtk::glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use strum::IntoEnumIterator;

use super::checked_listbox::CheckedListBoxWeak;

/// Control padding in pixels.
const PADDING: u32 = 10;

struct Widgets {
    device_list: CheckedListBox
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
        self.widgets.device_list.add_item(id, true, &format!("{} [{:016X}]", name, id));
    }

    pub fn remove_device(&mut self, id: u64) {
        self.widgets.device_list.remove_item(id);
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

    let device_list = CheckedListBox::new();
    box_all.pack_start(&device_list.widget(), false, true, PADDING);

    let action_grid = gtk::GridBuilder::new()
        .build();

    action_grid.insert_column(0);
    action_grid.insert_column(1);
    action_grid.insert_column(2);

    let add_action_controls = |action_idx: usize, target_action: TargetAction| {
        action_grid.attach(
            &gtk::Label::builder()
                .label(&target_action.to_string())
                .halign(gtk::Align::Start)
                .margin_start(PADDING as i32)
                .margin_end(PADDING as i32)
                .expand(true)
                .build(),
            0, action_idx as i32,
            1, 1
        );

        let format_src_action = |a: &SourceAction| format!("[{}] {}", a.ctrl_name, a.event.as_str());

        let chosen_action_label = gtk::Label::builder()
            .label(&program_data_rc.borrow().ctrl_actions.get(target_action)
                .as_ref()
                .map_or("".to_string(), |a| format_src_action(&a))
            )
            .halign(gtk::Align::Start)
            .expand(true)
            .margin_start(PADDING as i32)
            .margin_end(PADDING as i32)
            .build();
        action_grid.attach(
            &chosen_action_label,
            1, action_idx as i32,
            1, 1
        );

        let btn = gtk::Button::builder()
            .label("configure")
            .halign(gtk::Align::End)
            .build();
        btn.connect_clicked(clone!(
            @weak parent,
            @weak chosen_action_label,
            @weak program_data_rc
            => @default-panic, move |_| {
                // TODO allow unsetting an action
                if let Some(src_action) = show_controller_action_selection_dialog(
                    &parent,
                    &program_data_rc,
                    target_action
                ) {
                    program_data_rc.borrow_mut().ctrl_actions.set(target_action, Some(src_action.clone()));
                    chosen_action_label.set_label(&format_src_action(&src_action));
                    log::info!("new action assignment: {:?} -> {}", src_action, target_action);
                }

                let pd = program_data_rc.borrow();
                pd.config.save_controller_actions(&pd.ctrl_actions);
            }
        ));
        action_grid.attach(
            &btn,
            2, action_idx as i32,
            1, 1
        );
    };

    for (idx, target_action) in TargetAction::iter().enumerate() {
        add_action_controls(idx, target_action);
    }

    let actions = gtk::Frame::builder()
        .label("Actions")
        .child(&action_grid)
        .build();
    box_all.pack_start(&actions, true, true, PADDING);

    (box_all, Widgets{ device_list })
}

#[cfg(feature = "controller")]
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
    program_data_rc: &Rc<RefCell<ProgramData>>,
    target_action: TargetAction
) -> Option<SourceAction> {
    let dialog = gtk::Dialog::with_buttons(
        Some("Choose controller action"),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[("OK", gtk::ResponseType::Ok), ("Cancel", gtk::ResponseType::Cancel)]
    );

    let msg = match (target_action.analog_ctrl_action_allowed(), target_action.discrete_ctrl_action_allowed()) {
        (true, false) => "Move controller analog axis through its full range.",
        (false, true) => "Press controller button.",
        (true, true) => "Press controller button or move its analog axis through its full range.",
        (false, false) => unreachable!()
    };

    dialog.content_area().pack_start(
        &gtk::Label::new(Some(msg)),
        true, true, PADDING
    );
    let action_label = gtk::Label::new(None);
    dialog.content_area().pack_start(&action_label, true, true, PADDING);
    dialog.show_all();

    program_data_rc.borrow_mut().sel_dialog_ctrl_events = Some(vec![]);

    let refresh_interval = std::time::Duration::from_secs(1);

    let timer = Rc::new(crate::timer::Timer::new());
    let selected_src_action: Rc<RefCell<Option<SourceAction>>> = Rc::new(RefCell::new(None));
    let handler = clone!(
        @weak selected_src_action,
        @weak timer,
        @weak action_label,
        @weak program_data_rc
        => @default-panic, move || {
            on_dialog_timer(program_data_rc, target_action, action_label, selected_src_action, 2 * refresh_interval);
        }
    );

    timer.run(refresh_interval, false, handler);

    let result = if let gtk::ResponseType::Ok = dialog.run() {
        selected_src_action.borrow_mut().take()
    } else {
        None
    };

    program_data_rc.borrow_mut().sel_dialog_ctrl_events = None;

    dialog.close();

    result
}

fn on_dialog_timer(
    program_data_rc: Rc<RefCell<ProgramData>>,
    target_action: TargetAction,
    action_label: gtk::Label,
    selected_src_action: Rc<RefCell<Option<SourceAction>>>,
    back_look_up_interval: std::time::Duration
) {
    let mut pd = program_data_rc.borrow_mut();

    remove_older_than(pd.sel_dialog_ctrl_events.as_mut().unwrap(), std::time::Instant::now() - back_look_up_interval);

    if let Some(mut action) = choose_ctrl_action_based_on_events(
        &pd.sel_dialog_ctrl_events.as_ref().unwrap(),
        &pd.ctrl_names,
        target_action.analog_ctrl_action_allowed(),
        target_action.discrete_ctrl_action_allowed()
    ) {
        let mut prev_action = selected_src_action.borrow_mut();
        if let Some(prev_action) = prev_action.as_mut() {
            if prev_action.ctrl_id == action.ctrl_id && prev_action.event == action.event {
                if let Some(ref mut range) = action.range {
                    range.extend_with(prev_action.range.as_ref().unwrap());
                }
            }
        }

        let text = &format!(
            "{}{}",
            action.event.as_str(),
            &if let Some(range) = &action.range {
                format!(" ({:.04}; {:.04})", range.min, range.max)
            } else {
                "".to_string()
            }
        );
        action_label.set_text(&text);
        *prev_action = Some(action);
    }
    pd.sel_dialog_ctrl_events.as_mut().unwrap().clear();
}

fn remove_older_than(
    events: &mut Vec<(std::time::Instant, workers::controller::StickEvent)>, when: std::time::Instant
) {
    if let Some((remove_until_idx, _)) = events.iter().enumerate().find(|(_, (t, _))| *t >= when) {
        events.drain(0..remove_until_idx);
    }
}
