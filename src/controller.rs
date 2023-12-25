use crate::{workers, workers::controller::ControllerToMainThreadMsg, ProgramData};
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
    let mut pd = program_data_rc.borrow_mut();

    match msg {
        ControllerToMainThreadMsg::NewDevice(new_device) => {
            log::info!("new controller: {} [{:016X}]", new_device.name, new_device.id);
            pd.ctrl_names.insert(new_device.id, new_device.name.clone());
            pd.gui.as_mut().unwrap().controller_dialog_mut().add_device(new_device.id, &new_device.name);
        },

        ControllerToMainThreadMsg::StickEvent(event) => {
            if let stick::Event::Disconnect = event.event {
                log::info!("controller [{:016X}] removed", event.id);
                pd.gui.as_mut().unwrap().controller_dialog_mut().remove_device(event.id);
            } else if let Some(sel_events) = &mut pd.sel_dialog_ctrl_events {
                sel_events.push(event);
            }
        },
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
