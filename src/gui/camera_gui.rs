//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Camera GUI.
//!

use crate::{CameraControlChange, OnCapturePauseAction, ProgramData};
use crate::camera;
use crate::camera::{BaseProperties, CameraControl, CameraControlId, CameraInfo, ControlAccessMode, Driver};
use crate::gui::{disconnect_camera, on_capture_thread_message, show_message};
use crate::workers::capture;
use crate::workers::capture::MainToCaptureThreadMsg;
use enum_dispatch::enum_dispatch;
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct ListControlWidgets {
    pub combo: gtk::ComboBoxText,
    pub combo_changed_signal: glib::SignalHandlerId
}

pub struct NumberControlWidgets {
    pub slider: gtk::Scale,
    pub spin_btn: gtk::SpinButton,
    pub slider_changed_signal: Rc<RefCell<Option<glib::SignalHandlerId>>>,
    pub spin_btn_changed_signal: Rc<RefCell<Option<glib::SignalHandlerId>>>
}

pub struct CommonControlWidgets {
    pub name: String,
    pub h_box: gtk::Box,
    pub auto: Option<gtk::CheckButton>,
    pub on_off: Option<gtk::CheckButton>,
    pub access_mode: camera::ControlAccessMode
}

#[enum_dispatch]
pub enum ControlWidgetBundle {
    ListControl(ListControlWidgets),
    NumberControl(NumberControlWidgets)
}

#[enum_dispatch(ControlWidgetBundle)]
pub trait Editability {
    fn set_editable(&self, state: bool);
}

impl Editability for ListControlWidgets {
    fn set_editable(&self, state: bool) {
        self.combo.set_sensitive(state);
    }
}

impl Editability for NumberControlWidgets {
    fn set_editable(&self, state: bool) {
        self.slider.set_sensitive(state);
    }
}

/// Returns (camera menu, camera menu items, camera disconnect menu item).
pub fn init_camera_menu(
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> (gtk::Menu, Vec<(gtk::CheckMenuItem, glib::SignalHandlerId)>, gtk::MenuItem) {
    let menu = gtk::Menu::new();
    let camera_menu_items = create_camera_menu_items(&menu, program_data_rc);

    menu.append(&gtk::SeparatorMenuItem::new());

    let rescan = gtk::MenuItem::new_with_label("Rescan");
    rescan.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        disconnect_camera(&mut program_data_rc.borrow_mut(), true);

        for cam_item in &program_data_rc.borrow().gui.as_ref().unwrap().camera_menu_items {
            program_data_rc.borrow().gui.as_ref().unwrap().camera_menu.remove(&cam_item.0);
        }

        let camera_menu = program_data_rc.borrow().gui.as_ref().unwrap().camera_menu.clone();
        program_data_rc.borrow_mut().gui.as_mut().unwrap().camera_menu_items =
            create_camera_menu_items(
                &camera_menu,
                &program_data_rc
            );
    }));

    let disconnect_item = gtk::MenuItem::new_with_label("Disconnect");
    disconnect_item.connect_activate(clone!(@weak program_data_rc => @default-panic, move |_| {
        disconnect_camera(&mut program_data_rc.borrow_mut(), true);
    }));
    disconnect_item.set_sensitive(false);

    menu.append(&rescan);
    menu.append(&disconnect_item);

    (menu, camera_menu_items, disconnect_item)
}

/// Adds camera items at the beginning of `camera_menu`.
fn create_camera_menu_items(
    camera_menu: &gtk::Menu,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> Vec<(gtk::CheckMenuItem, glib::SignalHandlerId)> {
    let mut camera_menu_items = vec![];

    let mut item_pos = 0;
    for driver in program_data_rc.borrow().drivers.iter() {
        let drv_name = driver.borrow().name();
        for camera_info in driver.borrow_mut().enumerate_cameras().unwrap() {
            let cam_menu_item = gtk::CheckMenuItem::new_with_label(&format!("[{}] {}", drv_name, camera_info.name()));
            cam_menu_item.show();

            let signal = cam_menu_item.connect_activate(clone!(
                @weak driver, @weak program_data_rc
                => @default-panic, move |menu_item| {
                    on_select_camera(menu_item, &driver, &camera_info, &program_data_rc);
                    program_data_rc.borrow().gui.as_ref().unwrap().camera_disconnect_menu_item.set_sensitive(true);
                }
            ));
            camera_menu.insert(&cam_menu_item, item_pos);
            camera_menu_items.push((cam_menu_item, signal));
            item_pos += 1;
        }
    }

    camera_menu_items
}

fn on_select_camera(
    menu_item: &gtk::CheckMenuItem,
    driver: &Rc<RefCell<std::boxed::Box<(dyn Driver)>>>,
    camera_info: &CameraInfo,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    {
        disconnect_camera(&mut program_data_rc.borrow_mut(), true);

        // drop the camera first, to avoid constructing a second one with the same id
        program_data_rc.borrow_mut().camera = None;

        program_data_rc.borrow_mut().camera = match driver.borrow_mut().open_camera(camera_info.id()) {
            Ok(camera) => Some(camera),
            Err(e) => {
                show_message(&format!("Failed to open {}:\n{:?}", camera_info.name(), e), "Error", gtk::MessageType::Error);
                return;
            }
        };

        let fc_result = program_data_rc.borrow_mut().camera.as_mut().unwrap().create_capturer();
        let frame_capturer = match fc_result {
            Ok(capturer) => capturer,
            Err(e) => {
                show_message(&format!("Failed to open {}:\n{:?}", camera_info.name(), e), "Error", gtk::MessageType::Error);
                disconnect_camera(&mut program_data_rc.borrow_mut(), false);
                return;
            }
        };

        let mut program_data = program_data_rc.borrow_mut();

        let (sender_main, receiver_worker) = std::sync::mpsc::channel();
        let (sender_worker, receiver_main) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        receiver_main.attach(None, clone!(@weak program_data_rc
            => @default-panic, move |msg| {
                on_capture_thread_message(msg, &program_data_rc);
                glib::Continue(true)
            }
        ));

        let buffered_kib_clone = program_data.recording_thread_data.buffered_kib.clone();

        let new_preview_wanted = std::sync::Arc::new(AtomicBool::new(true));

        program_data.capture_thread_data = Some(crate::CaptureThreadData {
            join_handle: Some(std::thread::spawn(clone!(@weak new_preview_wanted =>
                move || capture::capture_thread(frame_capturer, sender_worker, receiver_worker, buffered_kib_clone, new_preview_wanted)
            ))),
            sender: sender_main,
            new_preview_wanted
        });

        program_data.gui.as_ref().unwrap().rec_widgets.on_connect();

        for (cam_item, activate_signal) in &program_data.gui.as_ref().unwrap().camera_menu_items {
            if cam_item == menu_item {
                cam_item.set_sensitive(true);
                cam_item.block_signal(&activate_signal);
                cam_item.set_active(true);
                cam_item.unblock_signal(&activate_signal);
            }
        }
        menu_item.set_sensitive(false);

    } // end borrow of `program_data`

    init_camera_control_widgets(program_data_rc);
}

pub fn remove_camera_controls(program_data: &mut ProgramData) {
    if let Some(gui) = program_data.gui.as_mut() {
        gui.control_widgets.clear();
        gui.controls_box.foreach(|child| gui.controls_box.remove(child));
    }
}

fn init_camera_control_widgets(
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    remove_camera_controls(&mut program_data_rc.borrow_mut());

    let controls_box = program_data_rc.borrow().gui.as_ref().unwrap().controls_box.clone();

    let controls = program_data_rc.borrow_mut().camera.as_mut().unwrap().enumerate_controls().unwrap();
    for control in controls  {
        let h_box = create_control_widgets(
            &control,
            program_data_rc,
            &mut program_data_rc.borrow_mut().gui.as_mut().unwrap().control_widgets
        );
        controls_box.pack_start(&h_box, false, false, PADDING);
    }

    controls_box.show_all();
}

/// Reacts to camera notification(s) which may be returned after modifying a camera control.
pub fn on_camera_notification(
    notifications: Vec<camera::Notification>,
    program_data_rc: &Rc<RefCell<ProgramData>>
) {
    for notification in notifications {
        match notification {
            camera::Notification::ControlRemoved(id) => {
                match program_data_rc.borrow().gui.as_ref().unwrap().control_widgets.get(&id) {
                    Some((common_widgets, _)) => common_widgets.h_box.set_visible(false),
                    None => ()
                }
            },
            camera::Notification::ControlChanged(control) => {
                let controls_box = program_data_rc.borrow().gui.as_ref().unwrap().controls_box.clone();

                if let Some(gui) = program_data_rc.borrow_mut().gui.as_mut() {
                    match gui.control_widgets.get_mut(&control.base().id) {
                        // The control had been added previously, re-enable its widget(s).
                        Some(widgets) => {
                            widgets.0.h_box.set_visible(true);
                            match &mut widgets.1 {
                                ControlWidgetBundle::ListControl(list_widgets) => {
                                    if let camera::CameraControl::List(list_control) = control {
                                        fill_combo_for_list_control(
                                            &list_control,
                                            &list_widgets.combo,
                                            Some(&list_widgets.combo_changed_signal)
                                        );
                                    }
                                },

                                ControlWidgetBundle::NumberControl(number_widgets) => {
                                    if let camera::CameraControl::Number(number_control) = control {
                                        let adjustment = gtk::Adjustment::new(
                                            number_control.value(),
                                            number_control.min(),
                                            number_control.max(),
                                            number_control.step(),
                                            number_control.step() * 5.0,
                                            0.0
                                        );

                                        let slider = &number_widgets.slider;
                                        slider.block_signal(number_widgets.slider_changed_signal.borrow().as_ref().unwrap());
                                        slider.set_adjustment(&adjustment);
                                        slider.unblock_signal(number_widgets.slider_changed_signal.borrow().as_ref().unwrap());

                                        let spin_btn = &number_widgets.spin_btn;
                                        spin_btn.block_signal(number_widgets.spin_btn_changed_signal.borrow().as_ref().unwrap());
                                        spin_btn.configure(Some(&adjustment), spin_btn.get_property_climb_rate(), spin_btn.get_digits());
                                        spin_btn.unblock_signal(number_widgets.spin_btn_changed_signal.borrow().as_ref().unwrap());
                                    }
                                }
                            }
                        },
                        // This is the first time the control is returned; create its widgets from scratch.
                        None => {
                            let h_box = create_control_widgets(&control, program_data_rc, &mut gui.control_widgets);
                            h_box.show_all();
                            controls_box.pack_start(&h_box, false, false, PADDING);
                        }
                    }
                }
            }
        }
    }
}

fn create_control_widgets(
    control: &camera::CameraControl,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    control_widgets: &mut std::collections::HashMap<
        camera::CameraControlId,
        (CommonControlWidgets, ControlWidgetBundle)
    >
) -> gtk::Box {
    let h_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    h_box.pack_start(&gtk::Label::new(Some(&control.base().label)), false, false, PADDING);

    let ctrl_id = control.base().id;
    let access = control.base().access_mode;

    let cb_auto = match control.base().auto_state {
        Some(value) => {
            let cb = gtk::CheckButtonBuilder::new().label("auto").active(value).build();
            cb.connect_toggled(clone!(@weak program_data_rc => @default-panic, move|cb| {
                let notifications = program_data_rc.borrow().camera.as_ref().unwrap().set_auto(
                    ctrl_id,
                    cb.get_active()
                ).unwrap();
                on_camera_notification(notifications, &program_data_rc);

                let program_data = program_data_rc.borrow();
                let control_widgets = &program_data.gui.as_ref().unwrap().control_widgets[&ctrl_id];
                let is_on = match &control_widgets.0.on_off {
                    Some(cb_on_off) => cb_on_off.get_active(),
                    _ => true
                };
                control_widgets.1.set_editable(
                    !cb.get_active() && is_on && access != ControlAccessMode::ReadOnly
                );
            }));
            Some(cb)
        },
        _ => None
    };

    let cb_on_off = match control.base().on_off_state {
        Some(value) => {
            let cb = gtk::CheckButtonBuilder::new().label("on").active(value).build();
            cb.connect_toggled(clone!(@weak program_data_rc => @default-panic, move|cb| {
                let notifications = program_data_rc.borrow().camera.as_ref().unwrap().set_on_off(
                    ctrl_id,
                    cb.get_active()
                ).unwrap();
                on_camera_notification(notifications, &program_data_rc);
                let program_data = program_data_rc.borrow();
                let control_widgets = &program_data.gui.as_ref().unwrap().control_widgets[&ctrl_id];
                let is_auto = match &control_widgets.0.auto {
                    Some(cb_auto) => cb_auto.get_active(),
                    _ => false
                };

                control_widgets.1.set_editable(
                    cb.get_active() && !is_auto && access != ControlAccessMode::ReadOnly
                );
            }));
            Some(cb)
        },
        _ => None
    };

    match control {
        CameraControl::List(list_ctrl) => {
            let widget_bundle = create_list_control_widgets(
                &list_ctrl,
                &h_box,
                program_data_rc
            );
            control_widgets.insert(
                list_ctrl.base().id,
                (CommonControlWidgets{
                    name: list_ctrl.base().label.clone(),
                    h_box: h_box.clone(),
                    auto: cb_auto.clone(),
                    on_off: cb_on_off.clone(),
                    access_mode: list_ctrl.base().access_mode
                }, widget_bundle)
            );
        },
        CameraControl::Number(number_ctrl) => {
            let widget_bundle = create_number_control_widgets(
                &number_ctrl,
                &h_box,
                program_data_rc
            );
            control_widgets.insert(
                number_ctrl.base().id,
                (CommonControlWidgets{
                    name: number_ctrl.base().label.clone(),
                    h_box: h_box.clone(),
                    auto: cb_auto.clone(),
                    on_off: cb_on_off.clone(),
                    access_mode: number_ctrl.base().access_mode
                }, widget_bundle)
            );
        }
    }

    if cb_auto.is_some() { h_box.pack_start(cb_auto.as_ref().unwrap(), false, false, PADDING); }
    if cb_on_off.is_some() { h_box.pack_start(cb_on_off.as_ref().unwrap(), false, false, PADDING); }

    h_box
}

fn create_list_control_widgets(
    list_ctrl: &camera::ListControl,
    h_box: &gtk::Box,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> ControlWidgetBundle {
    let items_combo = gtk::ComboBoxText::new();
    fill_combo_for_list_control(list_ctrl, &items_combo, None);

    if !is_control_editable(list_ctrl.base()) { items_combo.set_sensitive(false); }

    let ctrl_id = list_ctrl.base().id;
    let requires_capture_pause = list_ctrl.base().requires_capture_pause;
    let combo_changed_signal = items_combo.connect_changed(clone!(@weak program_data_rc => @default-panic, move |combo| {
        on_camera_list_control_change(combo, &program_data_rc, ctrl_id, requires_capture_pause)
    }));

    h_box.pack_start(&items_combo, false, false, PADDING);

    ControlWidgetBundle::ListControl(ListControlWidgets{
        combo: items_combo,
        combo_changed_signal
    })
}

fn create_number_control_widgets(
    number_ctrl: &camera::NumberControl,
    h_box: &gtk::Box,
    program_data_rc: &Rc<RefCell<ProgramData>>
) -> ControlWidgetBundle {
    // As of `gtk-sys` 0.9.2, cannot set min = max (contrary to what GTK 3 documentation allows).
    // Create a disabled slider instead with range = 1.

    let must_disable: bool;
    let min = number_ctrl.min();
    let max = if number_ctrl.min() == number_ctrl.max() {
        must_disable = true;
        number_ctrl.min() + 1.0
    } else {
        must_disable = false;
        number_ctrl.max()
    };
    let ctrl_id = number_ctrl.base().id;
    let requires_capture_pause = number_ctrl.base().requires_capture_pause;

    // create the slider -----------------------------------------

    let slider = gtk::Scale::new_with_range(
        gtk::Orientation::Horizontal,
        min,
        max,
        if must_disable { 1.0 } else { number_ctrl.step() }
    );
    slider.set_digits(number_ctrl.num_decimals() as i32);
    slider.set_value(if must_disable { min } else { number_ctrl.value() });

    if must_disable || !is_control_editable(number_ctrl.base()) { slider.set_sensitive(false); }

    h_box.pack_start(&slider, true, true, PADDING);

    // create the spin button -----------------------------------------

    let spin_btn = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(
            number_ctrl.value(), min, max, number_ctrl.step(), 10.0, 0.0
        )),
        0.0,
        number_ctrl.num_decimals() as u32
    );

    if must_disable || !is_control_editable(number_ctrl.base()) { spin_btn.set_sensitive(false); }

    h_box.pack_start(&spin_btn, false, true, PADDING);

    // set up event handlers -----------------------------------------

    // It is a hassle with storing those signals, but in GTK one cannot just say "freeze event handling temporarily and
    // let me change a slider/spin button value" - the specific event handler's signal ID must be explicitly blocked.
    // And we want to update both the slider and the spin button if either is changed.

    let slider_changed_signal: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let spin_btn_changed_signal: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));

    slider_changed_signal.borrow_mut().replace(slider.connect_value_changed(clone!(
        @weak program_data_rc, @weak spin_btn_changed_signal, @weak spin_btn => @default-panic,
        move |slider| {
            spin_btn.block_signal(spin_btn_changed_signal.borrow().as_ref().unwrap());
            spin_btn.set_value(slider.get_value());
            spin_btn.unblock_signal(spin_btn_changed_signal.borrow().as_ref().unwrap());
            on_camera_number_control_change(slider.get_value(), &program_data_rc, ctrl_id, requires_capture_pause);
        }
    )));

    spin_btn_changed_signal.borrow_mut().replace(spin_btn.connect_value_changed(clone!(
        @weak program_data_rc, @weak slider_changed_signal, @weak slider => @default-panic,
        move |spin_btn| {
            slider.block_signal(slider_changed_signal.borrow().as_ref().unwrap());
            slider.set_value(spin_btn.get_value());
            slider.unblock_signal(slider_changed_signal.borrow().as_ref().unwrap());
            on_camera_number_control_change(spin_btn.get_value(), &program_data_rc, ctrl_id, requires_capture_pause);
        }
    )));

    ControlWidgetBundle::NumberControl(
        NumberControlWidgets{ slider, spin_btn, slider_changed_signal, spin_btn_changed_signal }
    )
}

fn fill_combo_for_list_control(
    ctrl: &camera::ListControl,
    combo: &gtk::ComboBoxText,
    change_signal: Option<&glib::SignalHandlerId>
) {
    if change_signal.is_some() { combo.block_signal(change_signal.unwrap()); }

    match ctrl.base().access_mode {
        ControlAccessMode::ReadOnly | ControlAccessMode::None => combo.set_sensitive(false),
        _ => combo.set_sensitive(true)
    }

    combo.remove_all();
    for item in ctrl.items() {
        combo.append_text(item);
    }
    combo.set_active(Some(ctrl.current_idx() as u32));

    if change_signal.is_some() { combo.unblock_signal(change_signal.unwrap()); }
}

fn is_control_editable(control: &camera::CameraControlBase) -> bool {
    let is_on = match control.on_off_state {
        Some(false) => false,
        _ => true
    };

    let is_auto = match control.auto_state {
        Some(true) => true,
        _ => false
    };

    match control.access_mode {
        ControlAccessMode::ReadOnly | ControlAccessMode::None => false,
        _ => is_on && !is_auto
    }
}

fn on_camera_list_control_change(
    combo: &gtk::ComboBoxText,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    ctrl_id: CameraControlId,
    requires_capture_pause: bool
) {
    if requires_capture_pause {
        if program_data_rc.borrow_mut().capture_thread_data.as_mut().unwrap().sender.send(
            MainToCaptureThreadMsg::Pause
        ).is_err() {
            crate::on_capture_thread_failure(program_data_rc);
            return;
        }

        program_data_rc.borrow_mut().on_capture_pause_action = Some(OnCapturePauseAction::ControlChange(CameraControlChange{
            id: ctrl_id,
            option_idx: combo.get_active().unwrap() as usize
        }));
    } else {
        let notifications = program_data_rc.borrow_mut().camera.as_mut().unwrap().set_list_control(
            ctrl_id,
            combo.get_active().unwrap() as usize
        ).unwrap();
        on_camera_notification(
            notifications,
            &program_data_rc
        );
    }
}

fn on_camera_number_control_change(
    value: f64,
    program_data_rc: &Rc<RefCell<ProgramData>>,
    ctrl_id: CameraControlId,
    requires_capture_pause: bool
) {
    if requires_capture_pause {
        panic!("Not implemented yet.");
    } else {
        let notifications = program_data_rc.borrow_mut().camera.as_mut().unwrap().set_number_control(
            ctrl_id,
            value
        ).unwrap();
        on_camera_notification(
            notifications,
            &program_data_rc
        );
    }
}
