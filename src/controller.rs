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

use crate::{mount, workers, workers::controller::{ControllerToMainThreadMsg, StickEvent}, ProgramData};
use scan_fmt::scan_fmt;
use std::{cell::RefCell, collections::HashMap, error::Error, rc::Rc};
use strum::IntoEnumIterator;

mod serialized_event {
    use std::error::Error;

    #[derive(Clone, Debug)]
    pub struct SerializedEvent(String);

    impl SerializedEvent {
        pub fn as_str(&self) -> &str { &self.0 }

        pub fn from_str(s: &str) -> Result<SerializedEvent, Box<dyn Error>> {
            match s {
                "ActionA"
                | "ActionB"
                | "ActionC"
                | "ActionD"
                | "ActionH"
                | "ActionL"
                | "ActionM"
                | "ActionR"
                | "ActionV"
                | "Apu"
                | "AutopilotAlt"
                | "AutopilotPath"
                | "AutopilotToggle"
                | "BoatBackward"
                | "BoatForward"
                | "Brake"
                | "Bumper"
                | "BumperL"
                | "BumperR"
                | "Cam"
                | "CamX"
                | "CamY"
                | "CamZ"
                | "ChinaBackward"
                | "ChinaForward"
                | "Context"
                | "Down"
                | "Dpi"
                | "Eac"
                | "EngineFuelFlowL"
                | "EngineFuelFlowR"
                | "EngineIgnitionL"
                | "EngineIgnitionR"
                | "EngineMotorL"
                | "EngineMotorR"
                | "Exit"
                | "FlapsDown"
                | "FlapsUp"
                | "Gas"
                | "HatDown"
                | "HatLeft"
                | "HatRight"
                | "HatUp"
                | "Joy"
                | "JoyX"
                | "JoyY"
                | "JoyZ"
                | "LandingGearSilence"
                | "Left"
                | "MenuL"
                | "MenuR"
                | "MicDown"
                | "MicLeft"
                | "MicPush"
                | "MicRight"
                | "MicUp"
                | "Mouse"
                | "MouseX"
                | "MouseY"
                | "Number0"
                | "Number1"
                | "Number2"
                | "Number3"
                | "Number4"
                | "Number5"
                | "Number6"
                | "Number7"
                | "Number8"
                | "Number9"
                | "Number10"
                | "Number11"
                | "Number12"
                | "Number13"
                | "Number14"
                | "Number15"
                | "Number16"
                | "Number17"
                | "Number18"
                | "Number19"
                | "Number20"
                | "Number21"
                | "Number22"
                | "Number23"
                | "Number24"
                | "Number25"
                | "Number26"
                | "Number27"
                | "Number28"
                | "Number29"
                | "Number30"
                | "Number31"
                | "PaddleLeft"
                | "PaddleRight"
                | "Pinky"
                | "PinkyBackward"
                | "PinkyForward"
                | "PinkyLeft"
                | "PinkyRight"
                | "PovDown"
                | "PovLeft"
                | "PovRight"
                | "PovUp"
                | "RadarAltimeter"
                | "Right"
                | "Rudder"
                | "Scroll"
                | "ScrollX"
                | "ScrollY"
                | "Slew"
                | "SpeedbrakeBackward"
                | "SpeedbrakeForward"
                | "Throttle"
                | "ThrottleButton"
                | "ThrottleL"
                | "ThrottleR"
                | "Trigger"
                | "TriggerL"
                | "TriggerR"
                | "TrimDown"
                | "TrimLeft"
                | "TrimRight"
                | "TrimUp"
                | "Up"
                | "Volume"
                | "Wheel" => Ok(SerializedEvent(s.to_string())),

                _ => Err(format!("invalid event: {}", s).into())
            }
        }

        pub fn from_event(event: &stick::Event) -> SerializedEvent {
            SerializedEvent(match event {
                stick::Event::ActionA(_) => "ActionA",
                stick::Event::ActionB(_) => "ActionB",
                stick::Event::ActionC(_) => "ActionC",
                stick::Event::ActionD(_) => "ActionD",
                stick::Event::ActionH(_) => "ActionH",
                stick::Event::ActionL(_) => "ActionL",
                stick::Event::ActionM(_) => "ActionM",
                stick::Event::ActionR(_) => "ActionR",
                stick::Event::ActionV(_) => "ActionV",
                stick::Event::Apu(_) => "Apu",
                stick::Event::AutopilotAlt(_) => "AutopilotAlt",
                stick::Event::AutopilotPath(_) => "AutopilotPath",
                stick::Event::AutopilotToggle(_) => "AutopilotToggle",
                stick::Event::BoatBackward(_) => "BoatBackward",
                stick::Event::BoatForward(_) => "BoatForward",
                stick::Event::Brake(_) => "Brake",
                stick::Event::Bumper(_) => "Bumper",
                stick::Event::BumperL(_) => "BumperL",
                stick::Event::BumperR(_) => "BumperR",
                stick::Event::Cam(_) => "Cam",
                stick::Event::CamX(_) => "CamX",
                stick::Event::CamY(_) => "CamY",
                stick::Event::CamZ(_) => "CamZ",
                stick::Event::ChinaBackward(_) => "ChinaBackward",
                stick::Event::ChinaForward(_) => "ChinaForward",
                stick::Event::Context(_) => "Context",
                stick::Event::Down(_) => "Down",
                stick::Event::Dpi(_) => "Dpi",
                stick::Event::Eac(_) => "Eac",
                stick::Event::EngineFuelFlowL(_) => "EngineFuelFlowL",
                stick::Event::EngineFuelFlowR(_) => "EngineFuelFlowR",
                stick::Event::EngineIgnitionL(_) => "EngineIgnitionL",
                stick::Event::EngineIgnitionR(_) => "EngineIgnitionR",
                stick::Event::EngineMotorL(_) => "EngineMotorL",
                stick::Event::EngineMotorR(_) => "EngineMotorR",
                stick::Event::Exit(_) => "Exit",
                stick::Event::FlapsDown(_) => "FlapsDown",
                stick::Event::FlapsUp(_) => "FlapsUp",
                stick::Event::Gas(_) => "Gas",
                stick::Event::HatDown(_) => "HatDown",
                stick::Event::HatLeft(_) => "HatLeft",
                stick::Event::HatRight(_) => "HatRight",
                stick::Event::HatUp(_) => "HatUp",
                stick::Event::Joy(_) => "Joy",
                stick::Event::JoyX(_) => "JoyX",
                stick::Event::JoyY(_) => "JoyY",
                stick::Event::JoyZ(_) => "JoyZ",
                stick::Event::LandingGearSilence(_) => "LandingGearSilence",
                stick::Event::Left(_) => "Left",
                stick::Event::MenuL(_) => "MenuL",
                stick::Event::MenuR(_) => "MenuR",
                stick::Event::MicDown(_) => "MicDown",
                stick::Event::MicLeft(_) => "MicLeft",
                stick::Event::MicPush(_) => "MicPush",
                stick::Event::MicRight(_) => "MicRight",
                stick::Event::MicUp(_) => "MicUp",
                stick::Event::Mouse(_) => "Mouse",
                stick::Event::MouseX(_) => "MouseX",
                stick::Event::MouseY(_) => "MouseY",
                stick::Event::Number(0, _) => "Number0",
                stick::Event::Number(1, _) => "Number1",
                stick::Event::Number(2, _) => "Number2",
                stick::Event::Number(3, _) => "Number3",
                stick::Event::Number(4, _) => "Number4",
                stick::Event::Number(5, _) => "Number5",
                stick::Event::Number(6, _) => "Number6",
                stick::Event::Number(7, _) => "Number7",
                stick::Event::Number(8, _) => "Number8",
                stick::Event::Number(9, _) => "Number9",
                stick::Event::Number(10, _) => "Number10",
                stick::Event::Number(11, _) => "Number11",
                stick::Event::Number(12, _) => "Number12",
                stick::Event::Number(13, _) => "Number13",
                stick::Event::Number(14, _) => "Number14",
                stick::Event::Number(15, _) => "Number15",
                stick::Event::Number(16, _) => "Number16",
                stick::Event::Number(17, _) => "Number17",
                stick::Event::Number(18, _) => "Number18",
                stick::Event::Number(19, _) => "Number19",
                stick::Event::Number(20, _) => "Number20",
                stick::Event::Number(21, _) => "Number21",
                stick::Event::Number(22, _) => "Number22",
                stick::Event::Number(23, _) => "Number23",
                stick::Event::Number(24, _) => "Number24",
                stick::Event::Number(25, _) => "Number25",
                stick::Event::Number(26, _) => "Number26",
                stick::Event::Number(27, _) => "Number27",
                stick::Event::Number(28, _) => "Number28",
                stick::Event::Number(29, _) => "Number29",
                stick::Event::Number(30, _) => "Number30",
                stick::Event::Number(31, _) => "Number31",
                stick::Event::PaddleLeft(_) => "PaddleLeft",
                stick::Event::PaddleRight(_) => "PaddleRight",
                stick::Event::Pinky(_) => "Pinky",
                stick::Event::PinkyBackward(_) => "PinkyBackward",
                stick::Event::PinkyForward(_) => "PinkyForward",
                stick::Event::PinkyLeft(_) => "PinkyLeft",
                stick::Event::PinkyRight(_) => "PinkyRight",
                stick::Event::PovDown(_) => "PovDown",
                stick::Event::PovLeft(_) => "PovLeft",
                stick::Event::PovRight(_) => "PovRight",
                stick::Event::PovUp(_) => "PovUp",
                stick::Event::RadarAltimeter(_) => "RadarAltimeter",
                stick::Event::Right(_) => "Right",
                stick::Event::Rudder(_) => "Rudder",
                stick::Event::Scroll(_) => "Scroll",
                stick::Event::ScrollX(_) => "ScrollX",
                stick::Event::ScrollY(_) => "ScrollY",
                stick::Event::Slew(_) => "Slew",
                stick::Event::SpeedbrakeBackward(_) => "SpeedbrakeBackward",
                stick::Event::SpeedbrakeForward(_) => "SpeedbrakeForward",
                stick::Event::Throttle(_) => "Throttle",
                stick::Event::ThrottleButton(_) => "ThrottleButton",
                stick::Event::ThrottleL(_) => "ThrottleL",
                stick::Event::ThrottleR(_) => "ThrottleR",
                stick::Event::Trigger(_) => "Trigger",
                stick::Event::TriggerL(_) => "TriggerL",
                stick::Event::TriggerR(_) => "TriggerR",
                stick::Event::TrimDown(_) => "TrimDown",
                stick::Event::TrimLeft(_) => "TrimLeft",
                stick::Event::TrimRight(_) => "TrimRight",
                stick::Event::TrimUp(_) => "TrimUp",
                stick::Event::Up(_) => "Up",
                stick::Event::Volume(_) => "Volume",
                stick::Event::Wheel(_) => "Wheel",

                _ => {
                    let msg = format!("unrecognized event: {:?}", event);
                    log::error!("{}", msg);
                    panic!("{}", msg)
                }
            }.to_string())
        }
    }
}

pub use serialized_event::SerializedEvent;

enum EventValue {
    Discrete(bool),
    Analog(f64)
}

#[derive(Clone, Debug)]
pub struct SourceAction {
    pub ctrl_id: u64,
    pub ctrl_name: String, // only for user information, not used to filter controller events
    pub event: SerializedEvent
}

impl SourceAction {
    pub fn serialize(&self) -> String {
        format!("[{:016X}][{}]{}", self.ctrl_id, self.ctrl_name, self.event.as_str())
    }

    pub fn matches(&self, event: &StickEvent) -> bool {
        self.ctrl_id == event.id && SerializedEvent::from_event(&event.event).as_str() == self.event.as_str()
    }
}

impl std::str::FromStr for SourceAction {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ctrl_id, ctrl_name, event_str) =
            scan_fmt!(s, "[{x}][{[^]]}]{}", [hex u64], String, String)?;

        Ok(SourceAction{ ctrl_id, ctrl_name, event: SerializedEvent::from_str(&event_str)? })
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
                    sel_events.push(event);
                    return;
                }

                dispatch_event(event, program_data_rc);
            }
        },
    }
}

fn dispatch_event(event: StickEvent, program_data_rc: &Rc<RefCell<ProgramData>>) {

    let mut target_action: Option<TargetAction> = None;
    loop { // only for early exit from block
        let actions = &program_data_rc.borrow().ctrl_actions;

        if let Some(src_action) = &actions.mount_axis_1_pos {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis1Pos); break; }
        }

        if let Some(src_action) = &actions.mount_axis_1_neg {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis1Neg); break; }
        }

        if let Some(src_action) = &actions.mount_axis_2_pos {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis2Pos); break; }
        }

        if let Some(src_action) = &actions.mount_axis_2_neg {
            if src_action.matches(&event) { target_action = Some(TargetAction::MountAxis2Neg); break; }
        }

        break;
    } // end of `program_data_rc` borrow
    if target_action.is_none() { return; }

    //TODO: handle analog controls
    match target_action.unwrap() {
        TargetAction::MountAxis1Pos => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = crate::gui::axis_slew(mount::Axis::Primary, true, value, program_data_rc);
            }
        },
        TargetAction::MountAxis1Neg => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = crate::gui::axis_slew(mount::Axis::Primary, false, value, program_data_rc);
            }
        },
        TargetAction::MountAxis2Pos => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = crate::gui::axis_slew(mount::Axis::Secondary, true, value, program_data_rc);
            }
        },
        TargetAction::MountAxis2Neg => if let EventValue::Discrete(value) = event_value(&event.event) {
            if program_data_rc.borrow().mount_data.mount.is_some() {
                let _ = crate::gui::axis_slew(mount::Axis::Secondary, false, value, program_data_rc);
            }
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
    match event {
        stick::Event::Exit(_)
        | stick::Event::ActionA(_)
        | stick::Event::ActionB(_)
        | stick::Event::ActionC(_)
        | stick::Event::ActionH(_)
        | stick::Event::ActionV(_)
        | stick::Event::ActionD(_)
        | stick::Event::MenuL(_)
        | stick::Event::MenuR(_)
        | stick::Event::Joy(_)
        | stick::Event::Cam(_)
        | stick::Event::BumperL(_)
        | stick::Event::BumperR(_)
        | stick::Event::Up(_)
        | stick::Event::Down(_)
        | stick::Event::Left(_)
        | stick::Event::Right(_)
        | stick::Event::PovUp(_)
        | stick::Event::PovDown(_)
        | stick::Event::PovLeft(_)
        | stick::Event::PovRight(_)
        | stick::Event::HatUp(_)
        | stick::Event::HatDown(_)
        | stick::Event::HatLeft(_)
        | stick::Event::HatRight(_)
        | stick::Event::TrimUp(_)
        | stick::Event::TrimDown(_)
        | stick::Event::TrimLeft(_)
        | stick::Event::TrimRight(_)
        | stick::Event::MicUp(_)
        | stick::Event::MicDown(_)
        | stick::Event::MicLeft(_)
        | stick::Event::MicRight(_)
        | stick::Event::MicPush(_)
        | stick::Event::Trigger(_)
        | stick::Event::Bumper(_)
        | stick::Event::ActionM(_)
        | stick::Event::ActionL(_)
        | stick::Event::ActionR(_)
        | stick::Event::Pinky(_)
        | stick::Event::PinkyForward(_)
        | stick::Event::PinkyBackward(_)
        | stick::Event::FlapsUp(_)
        | stick::Event::FlapsDown(_)
        | stick::Event::BoatForward(_)
        | stick::Event::BoatBackward(_)
        | stick::Event::AutopilotPath(_)
        | stick::Event::AutopilotAlt(_)
        | stick::Event::EngineMotorL(_)
        | stick::Event::EngineMotorR(_)
        | stick::Event::EngineFuelFlowL(_)
        | stick::Event::EngineFuelFlowR(_)
        | stick::Event::EngineIgnitionL(_)
        | stick::Event::EngineIgnitionR(_)
        | stick::Event::SpeedbrakeBackward(_)
        | stick::Event::SpeedbrakeForward(_)
        | stick::Event::ChinaBackward(_)
        | stick::Event::ChinaForward(_)
        | stick::Event::Apu(_)
        | stick::Event::RadarAltimeter(_)
        | stick::Event::LandingGearSilence(_)
        | stick::Event::Eac(_)
        | stick::Event::AutopilotToggle(_)
        | stick::Event::ThrottleButton(_)
        | stick::Event::Mouse(_)
        | stick::Event::Number(_, _)
        | stick::Event::PaddleLeft(_)
        | stick::Event::PaddleRight(_)
        | stick::Event::PinkyLeft(_)
        | stick::Event::PinkyRight(_)
        | stick::Event::Context(_)
        | stick::Event::Dpi(_)
        | stick::Event::Scroll(_) => true,

        _ => false
    }
}

pub fn choose_ctrl_action_based_on_events(
    events: &[workers::controller::StickEvent],
    ctrl_names: &HashMap<u64, String>
) -> Option<SourceAction> {
    if events.is_empty() { return None; }

    for event in events {
        if is_discrete(&event.event) {
            return Some(SourceAction{
                ctrl_id: event.id,
                ctrl_name: ctrl_names.get(&event.id).unwrap().clone(),
                event: SerializedEvent::from_event(&event.event) });
        }
    }

    None
}
