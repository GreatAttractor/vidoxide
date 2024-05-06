//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Controller handling.
//!

use crate::{focuser, gui, mount, workers, workers::controller::{ControllerToMainThreadMsg, StickEvent}, ProgramData};
use std::{cell::RefCell, collections::HashMap, error::Error, rc::Rc};
use strum::IntoEnumIterator;

const FOCUSER_ACTION_REL_DEADZONE: f64 = 0.1;

mod serialized_event {
    use std::error::Error;

    #[derive(Clone, Debug, Hash, Eq, PartialEq)]
    pub struct SerializedEvent(String);

    impl SerializedEvent {
        pub fn as_str(&self) -> &str { &self.0 }

        pub fn from_str(s: &str) -> Result<SerializedEvent, Box<dyn Error>> {
            Ok(SerializedEvent(s.to_string()))
        }

        pub fn from_event(event: &stick::Event) -> SerializedEvent {
            SerializedEvent(format!("{}", event).split(' ').next().unwrap().to_string())
        }

        pub fn is_discrete(&self) -> bool {
            match self.as_str() {
                "Exit"
                | "ActionA"
                | "ActionB"
                | "ActionC"
                | "ActionH"
                | "ActionV"
                | "ActionD"
                | "MenuL"
                | "MenuR"
                | "Joy"
                | "Cam"
                | "BumperL"
                | "BumperR"
                | "Up"
                | "Down"
                | "Left"
                | "Right"
                | "PovUp"
                | "PovDown"
                | "PovLeft"
                | "PovRight"
                | "HatUp"
                | "HatDown"
                | "HatLeft"
                | "HatRight"
                | "TrimUp"
                | "TrimDown"
                | "TrimLeft"
                | "TrimRight"
                | "MicUp"
                | "MicDown"
                | "MicLeft"
                | "MicRight"
                | "MicPush"
                | "Trigger"
                | "Bumper"
                | "ActionM"
                | "ActionL"
                | "ActionR"
                | "Pinky"
                | "PinkyForward"
                | "PinkyBackward"
                | "FlapsUp"
                | "FlapsDown"
                | "BoatForward"
                | "BoatBackward"
                | "AutopilotPath"
                | "AutopilotAlt"
                | "EngineMotorL"
                | "EngineMotorR"
                | "EngineFuelFlowL"
                | "EngineFuelFlowR"
                | "EngineIgnitionL"
                | "EngineIgnitionR"
                | "SpeedbrakeBackward"
                | "SpeedbrakeForward"
                | "ChinaBackward"
                | "ChinaForward"
                | "Apu"
                | "RadarAltimeter"
                | "LandingGearSilence"
                | "Eac"
                | "AutopilotToggle"
                | "ThrottleButton"
                | "Mouse"
                | "Number"
                | "PaddleLeft"
                | "PaddleRight"
                | "PinkyLeft"
                | "PinkyRight"
                | "Context"
                | "Dpi"
                | "Scroll" => true,

                _ => false
            }
        }
    }
}

pub use serialized_event::SerializedEvent;

enum EventValue {
    Discrete(bool),
    Analog(f64)
}

#[derive(Clone, Debug)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64
}

impl ValueRange {
    pub fn extend_with(&mut self, other: &ValueRange) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }
}

#[derive(Clone, Debug)]
pub struct SourceAction {
    pub ctrl_id: u64,
    pub ctrl_name: String, // only for user information, not used to filter controller events
    pub event: SerializedEvent,
    pub range: Option<ValueRange> // analog actions only
}

impl SourceAction {
    pub fn serialize(&self) -> String {
        let range_s: String = if let Some(range) = &self.range {
            format!(";{:.05};{:.05}", range.min, range.max)
        } else {
            "".into()
        };
        format!("{:016X};{};{}{}", self.ctrl_id, self.ctrl_name, self.event.as_str(), range_s)
    }

    pub fn matches(&self, event: &StickEvent) -> bool {
        self.ctrl_id == event.id && SerializedEvent::from_event(&event.event).as_str() == self.event.as_str()
    }
}

impl std::str::FromStr for SourceAction {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() { return Err("unassigned".into()); }

        let fields: Vec<&str> = s.split(';').collect();
        if fields.len() < 3 { return Err(format!("invalid entry: {}", s).into()); }

        let id_bytes = hex::decode(fields[0])?;
        if id_bytes.len() != 8 { return Err(format!("invalid 64-bit hex number: {}", fields[0]).into()); }
        let id_bytes: [u8; 8] = [
            id_bytes[0], id_bytes[1], id_bytes[2], id_bytes[3], id_bytes[4], id_bytes[5], id_bytes[6], id_bytes[7]
        ];
        let ctrl_id = u64::from_be_bytes(id_bytes);
        let ctrl_name = fields[1].to_string();
        let event_str = fields[2];

        let event = SerializedEvent::from_str(&event_str)?;
        let range = if event.is_discrete() {
            None
        } else {
            if fields.len() < 5 { return Err(format!("missing analog event range: {}", s).into()); }
            let min = fields[3].parse::<f64>()?;
            let max = fields[4].parse::<f64>()?;
            if !min.is_finite() || !max.is_finite() || min >= max {
                return Err(format!("invalid analog event range: {}; {}", min, max).into());
            }
            Some(ValueRange{ min, max })
        };

        Ok(SourceAction{ ctrl_id, ctrl_name, event, range })
    }
}

#[derive(Copy, Clone, Eq, Hash, PartialEq, strum_macros::EnumIter)]
pub enum TargetAction {
    MountAxis1Pos,
    MountAxis1Neg,
    MountAxis2Pos,
    MountAxis2Neg,
    FocuserIn,
    FocuserOut,
    ToggleRecording,
}

impl TargetAction {
    pub fn config_key(&self) -> &'static str {
        match self {
            TargetAction::MountAxis1Pos => "MountAxis1Pos",
            TargetAction::MountAxis1Neg => "MountAxis1Neg",
            TargetAction::MountAxis2Pos => "MountAxis2Pos",
            TargetAction::MountAxis2Neg => "MountAxis2Neg",
            TargetAction::FocuserIn => "FocuserIn",
            TargetAction::FocuserOut => "FocuserOut",
            TargetAction::ToggleRecording => "ToggleRecording"
        }
    }

    pub fn discrete_ctrl_action_allowed(&self) -> bool {
        match self {
            TargetAction::MountAxis1Pos => true,
            TargetAction::MountAxis1Neg => true,
            TargetAction::MountAxis2Pos => true,
            TargetAction::MountAxis2Neg => true,
            TargetAction::FocuserIn => true,
            TargetAction::FocuserOut => true,
            TargetAction::ToggleRecording => true
        }
    }

    pub fn analog_ctrl_action_allowed(&self) -> bool {
        match self {
            TargetAction::MountAxis1Pos => false,
            TargetAction::MountAxis1Neg => false,
            TargetAction::MountAxis2Pos => false,
            TargetAction::MountAxis2Neg => false,
            TargetAction::FocuserIn => true,
            TargetAction::FocuserOut => true,
            TargetAction::ToggleRecording => false
        }
    }
}

impl std::fmt::Display for TargetAction  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", match self {
            TargetAction::MountAxis1Pos => "Mount axis 1 / positive",
            TargetAction::MountAxis1Neg => "Mount axis 1 / negative",
            TargetAction::MountAxis2Pos => "Mount axis 2 / positive",
            TargetAction::MountAxis2Neg => "Mount axis 2 / negative",
            TargetAction::FocuserIn => "Focuser / in",
            TargetAction::FocuserOut => "Focuser / out",
            TargetAction::ToggleRecording => "Toggle recording"
        })
    }
}

#[derive(Default)]
pub struct ActionAssignments {
    pub mount_axis_1_pos: Option<SourceAction>,
    pub mount_axis_1_neg: Option<SourceAction>,
    pub mount_axis_2_pos: Option<SourceAction>,
    pub mount_axis_2_neg: Option<SourceAction>,
    pub focuser_in: Option<SourceAction>,
    pub focuser_out: Option<SourceAction>,
    pub toggle_recording: Option<SourceAction>,
}

impl ActionAssignments {
    pub fn get(&self, target_action: TargetAction) -> &Option<SourceAction> {
        match target_action {
            TargetAction::MountAxis1Pos => &self.mount_axis_1_pos,
            TargetAction::MountAxis1Neg => &self.mount_axis_1_neg,
            TargetAction::MountAxis2Pos => &self.mount_axis_2_pos,
            TargetAction::MountAxis2Neg => &self.mount_axis_2_neg,
            TargetAction::FocuserIn => &self.focuser_in,
            TargetAction::FocuserOut => &self.focuser_out,
            TargetAction::ToggleRecording => &self.toggle_recording,
        }
    }

    pub fn set(&mut self, target_action: TargetAction, src_action: Option<SourceAction>) {
        match target_action {
            TargetAction::MountAxis1Pos => self.mount_axis_1_pos = src_action,
            TargetAction::MountAxis1Neg => self.mount_axis_1_neg = src_action,
            TargetAction::MountAxis2Pos => self.mount_axis_2_pos = src_action,
            TargetAction::MountAxis2Neg => self.mount_axis_2_neg = src_action,
            TargetAction::FocuserIn => self.focuser_in = src_action,
            TargetAction::FocuserOut => self.focuser_out = src_action,
            TargetAction::ToggleRecording => self.toggle_recording = src_action,
        }
    }
}

pub fn on_controller_event(msg: ControllerToMainThreadMsg, program_data_rc: &Rc<RefCell<ProgramData>>) {
    match msg {
        ControllerToMainThreadMsg::NewDevice(new_device) => {
            let mut pd = program_data_rc.borrow_mut();
            log::info!("new controller: {} [{:016X}]", new_device.name, new_device.id);
            pd.ctrl_names.insert(new_device.id, new_device.name.clone());
            pd.gui.as_mut().unwrap().controller_dialog_mut().add_device(new_device.id, &new_device.name);
        },

        ControllerToMainThreadMsg::StickEvent(event) => {
            if let stick::Event::Disconnect = event.event {
                log::info!("controller [{:016X}] removed", event.id);
                let mut pd = program_data_rc.borrow_mut();
                pd.gui.as_mut().unwrap().controller_dialog_mut().remove_device(event.id);
            } else {
                if let Some(sel_events) = &mut program_data_rc.borrow_mut().sel_dialog_ctrl_events {
                    sel_events.push((std::time::Instant::now(), event));
                    return;
                }

                dispatch_event(event, program_data_rc);
            }
        },
    }
}

fn dispatch_event(event: StickEvent, program_data_rc: &Rc<RefCell<ProgramData>>) {

    let mut target_action: Option<TargetAction> = None;
    let mut analog_range: Option<ValueRange> = None;
    'block: {
        let actions = &program_data_rc.borrow().ctrl_actions;

        if let Some(src_action) = &actions.mount_axis_1_pos {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis1Pos); break 'block; }
        }

        if let Some(src_action) = &actions.mount_axis_1_neg {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis1Neg); break 'block; }
        }

        if let Some(src_action) = &actions.mount_axis_2_pos {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis2Pos); break 'block; }
        }

        if let Some(src_action) = &actions.mount_axis_2_neg {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis2Neg); break 'block; }
        }

        if let Some(src_action) = &actions.toggle_recording {
            if src_action.matches(&event) { target_action = Some(TargetAction::ToggleRecording); break 'block; }
        }

        if let Some(src_action) = &actions.focuser_in {
            if src_action.matches(&event) {
                target_action = Some(TargetAction::FocuserIn);
                analog_range = src_action.range.clone();
                break 'block;
            }
        }

        if let Some(src_action) = &actions.focuser_out {
            if src_action.matches(&event) {
                target_action = Some(TargetAction::FocuserOut);
                analog_range = src_action.range.clone();
                break 'block;
            }
        }
    } // end of `program_data_rc` borrow
    if target_action.is_none() { return; }

    //TODO: handle analog controls
    match target_action.unwrap() {
        TargetAction::MountAxis1Pos => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = gui::axis_slew(mount::Axis::Primary, true, value, program_data_rc);
            }
        },
        TargetAction::MountAxis1Neg => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = gui::axis_slew(mount::Axis::Primary, false, value, program_data_rc);
            }
        },
        TargetAction::MountAxis2Pos => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = gui::axis_slew(mount::Axis::Secondary, true, value, program_data_rc);
            }
        },
        TargetAction::MountAxis2Neg => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = gui::axis_slew(mount::Axis::Secondary, false, value, program_data_rc);
            }
        },
        TargetAction::ToggleRecording => if let EventValue::Discrete(value) = event_value(&event.event) {
            if value {
                log::warn!("toggling recording via controller not yet implemented");
                //TODO: toggle recording
            }
        },
        TargetAction::FocuserIn => match event_value(&event.event) {
            EventValue::Discrete(value) => if program_data_rc.borrow().focuser_data.focuser.is_some() {
                let _ = gui::focuser_move(focuser::Speed::new(if value { -1.0 } else { 0.0 }), program_data_rc);
            },
            EventValue::Analog(value) => if program_data_rc.borrow().focuser_data.focuser.is_some() {
                let analog_range = analog_range.as_ref().unwrap();
                // we only allow positive analog action values for focuser in/out movement
                let scaled_value = (value.max(0.0) - analog_range.min.max(0.0))
                    / (analog_range.max - analog_range.min.max(0.0));
                let _ = gui::focuser_move(
                    focuser::Speed::new(/*if scaled_value > FOCUSER_ACTION_REL_DEADZONE {*/ -scaled_value /*} else { 0.0 }*/),
                    program_data_rc
                );
            },
        },

        _ => ()
    }
}

fn event_value(event: &stick::Event) -> EventValue {
    match event {
        stick::Event::ActionA(b) => EventValue::Discrete(*b),
        stick::Event::ActionB(b) => EventValue::Discrete(*b),
        stick::Event::ActionC(b) => EventValue::Discrete(*b),
        stick::Event::ActionD(b) => EventValue::Discrete(*b),
        stick::Event::ActionH(b) => EventValue::Discrete(*b),
        stick::Event::ActionL(b) => EventValue::Discrete(*b),
        stick::Event::ActionM(b) => EventValue::Discrete(*b),
        stick::Event::ActionR(b) => EventValue::Discrete(*b),
        stick::Event::ActionV(b) => EventValue::Discrete(*b),
        stick::Event::Apu(b) => EventValue::Discrete(*b),
        stick::Event::AutopilotAlt(b) => EventValue::Discrete(*b),
        stick::Event::AutopilotPath(b) => EventValue::Discrete(*b),
        stick::Event::AutopilotToggle(b) => EventValue::Discrete(*b),
        stick::Event::BoatBackward(b) => EventValue::Discrete(*b),
        stick::Event::BoatForward(b) => EventValue::Discrete(*b),
        stick::Event::Brake(f) => EventValue::Analog(*f),
        stick::Event::Bumper(b) => EventValue::Discrete(*b),
        stick::Event::BumperL(b) => EventValue::Discrete(*b),
        stick::Event::BumperR(b) => EventValue::Discrete(*b),
        stick::Event::Cam(b) => EventValue::Discrete(*b),
        stick::Event::CamX(f) => EventValue::Analog(*f),
        stick::Event::CamY(f) => EventValue::Analog(*f),
        stick::Event::CamZ(f) => EventValue::Analog(*f),
        stick::Event::ChinaBackward(b) => EventValue::Discrete(*b),
        stick::Event::ChinaForward(b) => EventValue::Discrete(*b),
        stick::Event::Context(b) => EventValue::Discrete(*b),
        stick::Event::Down(b) => EventValue::Discrete(*b),
        stick::Event::Dpi(b) => EventValue::Discrete(*b),
        stick::Event::Eac(b) => EventValue::Discrete(*b),
        stick::Event::EngineFuelFlowL(b) => EventValue::Discrete(*b),
        stick::Event::EngineFuelFlowR(b) => EventValue::Discrete(*b),
        stick::Event::EngineIgnitionL(b) => EventValue::Discrete(*b),
        stick::Event::EngineIgnitionR(b) => EventValue::Discrete(*b),
        stick::Event::EngineMotorL(b) => EventValue::Discrete(*b),
        stick::Event::EngineMotorR(b) => EventValue::Discrete(*b),
        stick::Event::Exit(b) => EventValue::Discrete(*b),
        stick::Event::FlapsDown(b) => EventValue::Discrete(*b),
        stick::Event::FlapsUp(b) => EventValue::Discrete(*b),
        stick::Event::Gas(f) => EventValue::Analog(*f),
        stick::Event::HatDown(b) => EventValue::Discrete(*b),
        stick::Event::HatLeft(b) => EventValue::Discrete(*b),
        stick::Event::HatRight(b) => EventValue::Discrete(*b),
        stick::Event::HatUp(b) => EventValue::Discrete(*b),
        stick::Event::Joy(b) => EventValue::Discrete(*b),
        stick::Event::JoyX(f) => EventValue::Analog(*f),
        stick::Event::JoyY(f) => EventValue::Analog(*f),
        stick::Event::JoyZ(f) => EventValue::Analog(*f),
        stick::Event::LandingGearSilence(b) => EventValue::Discrete(*b),
        stick::Event::Left(b) => EventValue::Discrete(*b),
        stick::Event::MenuL(b) => EventValue::Discrete(*b),
        stick::Event::MenuR(b) => EventValue::Discrete(*b),
        stick::Event::MicDown(b) => EventValue::Discrete(*b),
        stick::Event::MicLeft(b) => EventValue::Discrete(*b),
        stick::Event::MicPush(b) => EventValue::Discrete(*b),
        stick::Event::MicRight(b) => EventValue::Discrete(*b),
        stick::Event::MicUp(b) => EventValue::Discrete(*b),
        stick::Event::Mouse(b) => EventValue::Discrete(*b),
        stick::Event::MouseX(f) => EventValue::Analog(*f),
        stick::Event::MouseY(f) => EventValue::Analog(*f),
        stick::Event::Number(_, b) => EventValue::Discrete(*b),
        stick::Event::PaddleLeft(b) => EventValue::Discrete(*b),
        stick::Event::PaddleRight(b) => EventValue::Discrete(*b),
        stick::Event::Pinky(b) => EventValue::Discrete(*b),
        stick::Event::PinkyBackward(b) => EventValue::Discrete(*b),
        stick::Event::PinkyForward(b) => EventValue::Discrete(*b),
        stick::Event::PinkyLeft(b) => EventValue::Discrete(*b),
        stick::Event::PinkyRight(b) => EventValue::Discrete(*b),
        stick::Event::PovDown(b) => EventValue::Discrete(*b),
        stick::Event::PovLeft(b) => EventValue::Discrete(*b),
        stick::Event::PovRight(b) => EventValue::Discrete(*b),
        stick::Event::PovUp(b) => EventValue::Discrete(*b),
        stick::Event::RadarAltimeter(b) => EventValue::Discrete(*b),
        stick::Event::Right(b) => EventValue::Discrete(*b),
        stick::Event::Rudder(f) => EventValue::Analog(*f),
        stick::Event::Scroll(b) => EventValue::Discrete(*b),
        stick::Event::ScrollX(f) => EventValue::Analog(*f),
        stick::Event::ScrollY(f) => EventValue::Analog(*f),
        stick::Event::Slew(f) => EventValue::Analog(*f),
        stick::Event::SpeedbrakeBackward(b) => EventValue::Discrete(*b),
        stick::Event::SpeedbrakeForward(b) => EventValue::Discrete(*b),
        stick::Event::Throttle(f) => EventValue::Analog(*f),
        stick::Event::ThrottleButton(b) => EventValue::Discrete(*b),
        stick::Event::ThrottleL(f) => EventValue::Analog(*f),
        stick::Event::ThrottleR(f) => EventValue::Analog(*f),
        stick::Event::Trigger(b) => EventValue::Discrete(*b),
        stick::Event::TriggerL(f) => EventValue::Analog(*f),
        stick::Event::TriggerR(f) => EventValue::Analog(*f),
        stick::Event::TrimDown(b) => EventValue::Discrete(*b),
        stick::Event::TrimLeft(b) => EventValue::Discrete(*b),
        stick::Event::TrimRight(b) => EventValue::Discrete(*b),
        stick::Event::TrimUp(b) => EventValue::Discrete(*b),
        stick::Event::Up(b) => EventValue::Discrete(*b),
        stick::Event::Volume(f) => EventValue::Analog(*f),
        stick::Event::Wheel(f) => EventValue::Analog(*f),

        _ => panic!("unrecognized event: {:?}", event)
    }
}

/// Returns `true` for button-like events, `false` for analog-axis events.
fn is_discrete(event: &stick::Event) -> bool {
    SerializedEvent::from_event(event).is_discrete()
}

/// For discrete events, returns the first discrete action in `events`.
/// For analog events, returns the analog action which has the largest value range in `events`.
pub fn choose_ctrl_action_based_on_events(
    events: &[(std::time::Instant, workers::controller::StickEvent)],
    ctrl_names: &HashMap<u64, String>,
    analog: bool,
    discrete: bool
) -> Option<SourceAction> {
    if events.is_empty() { return None; }

    #[derive(Eq, PartialEq, Hash)]
    struct AnalogEventKey { ctrl_id: u64, event: SerializedEvent }
    let mut analog_events = std::collections::HashMap::<AnalogEventKey, ValueRange>::new();

    for (_, event) in events {
        if discrete && is_discrete(&event.event) {
            return Some(SourceAction{
                ctrl_id: event.id,
                ctrl_name: ctrl_names.get(&event.id).unwrap().clone(),
                event: SerializedEvent::from_event(&event.event),
                range: None
            });
        }

        if analog && !is_discrete(&event.event) {
            let val = match event_value(&event.event) {
                EventValue::Analog(a) => a,
                _ => unreachable!()
            };

            analog_events
                .entry(AnalogEventKey{ ctrl_id: event.id, event: SerializedEvent::from_event(&event.event) })
                .and_modify(|e| {
                    e.min = e.min.min(val);
                    e.max = e.max.max(val);
                })
                .or_insert(ValueRange{ min: val, max: val });
        }
    }

    // choose the analog action with the largest value range
    match analog_events.iter().max_by(|(_, a), (_, b)| (a.max - a.min).partial_cmp(&(b.max - b.min)).unwrap()) {
        Some((key, value)) => Some(SourceAction{
            ctrl_id: key.ctrl_id,
            ctrl_name: ctrl_names.get(&key.ctrl_id).unwrap().clone(),
            event: key.event.clone(),
            range: Some(value.clone())
        }),

        None => None,
    }
}
