//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Program configuration.
//!

use cgmath::Vector2;
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;

#[cfg(feature = "controller")]
use crate::{controller, controller::{ActionAssignments, TargetAction}};

mod groups {
    pub const CONTROLLER: &str = "Controller";
    pub const MAIN: &str = "Main";
    pub const MOUNT: &str = "Mount";
    pub const UI: &str = "UI";
}

mod keys {
    // group: MAIN
    pub const RECORDING_DEST_PATH: &str = "RecordingDestPath";
    pub const DISABLED_DRIVERS: &str = "DisabledDrivers";
    pub const PREVIEW_FPS_LIMIT: &str = "PreviewFpsLimit";
    pub const SIM_VIDEO_FILE: &str = "SimulatorVideoFile";

    // group: UI
    pub const MAIN_WINDOW_POS_SIZE: &str = "MainWindowPosSize";
    pub const MAIN_WINDOW_MAXIMIZED: &str = "MainWindowMaximized";
    /// Position of the divider between preview area and controls panel.
    pub const MAIN_WINDOW_PANED_POS: &str = "MainWindowPanedPos";
    /// Position of the divider between camera controls and histogram.
    pub const CAMERA_CONTROLS_PANED_POS: &str = "CameraControlsPanedPos";
    pub const INFO_OVERLAY_FONT_SIZE: &str = "InfoOverlayFontSize";
    pub const TOOLBAR_ICON_SIZE: &str = "ToolbarIconSize";

    // group MOUNT
    pub const IOPTRON_LAST_DEVICE: &str = "iOptronLastDevice";
    pub const SW_LAST_DEVICE: &str = "SkyWatcherLastDevice";
    pub const ZWO_LAST_DEVICE: &str = "ZWOLastDevice";
    pub const ASCOM_LAST_DRIVER: &str = "AscomLastDriver";
    pub const SIM_SKY_ROTATION_DIR_IN_IMG_SPACE: &str = "SimulatorSkyRotationDirInImgSpace";
    pub const SIM_PRIMARY_AXIS_SLEW_DIR_IN_IMG_SPACE: &str = "SimulatorPrimaryAxisSlewDirInImgSpace";
    //TODO: orientation of secondary axis' slew direction rel. to primary's (to simulate the usage of a star diagonal)
    pub const SIM_SKY_ROTATION_SPEED_PIX_PER_SEC: &str = "SimulatorSkyRotationSpeedPixelsPerSecond";
}

const DEFAULT_PREVIEW_FPS_LIMIT: i32 = 60;

pub struct Configuration {
    key_file: glib::KeyFile
}

impl Configuration {
    pub fn store(&self) -> Result<(), glib::error::Error> {
        self.key_file.save_to_file(config_file_path())
    }

    pub fn new() -> Configuration {
        let key_file = glib::KeyFile::new();
        let file_path = config_file_path();
        if key_file.load_from_file(
            file_path.clone(),
            glib::KeyFileFlags::NONE
        ).is_err() {
            println!("WARNING: Failed to load configuration from {}.", file_path.to_str().unwrap());
        }

        Configuration{ key_file }
    }

    #[cfg(feature = "controller")]
    pub fn save_controller_actions(&self, actions: &ActionAssignments) {
        for target_action in TargetAction::iter() {
            let s = if let Some(src_action) = actions.get(target_action) {
                src_action.serialize()
            } else {
                "".to_string()
            };
            self.key_file.set_string(groups::CONTROLLER, target_action.config_key(), &s);
        }
    }

    #[cfg(feature = "controller")]
    pub fn controller_actions(&self) -> ActionAssignments {
        use crate::controller::SourceAction;

        let mut result = ActionAssignments::default();

        for target_action in TargetAction::iter() {
            if let Ok(s) = self.key_file.string(groups::CONTROLLER, target_action.config_key()).map(|s| s.to_string()) {
                match s.parse::<SourceAction>() {
                    Ok(src_action) => result.set(target_action, Some(src_action)),
                    Err(e) => log::warn!("invalid action assignment: {}", e)
                }
            }
        }

        result
    }

    pub fn main_window_pos(&self) -> Option<gtk::Rectangle> {
        self.read_rect(groups::UI, keys::MAIN_WINDOW_POS_SIZE)
    }

    pub fn set_main_window_pos(&self, pos_size: gtk::Rectangle) {
        self.store_rect(groups::UI, keys::MAIN_WINDOW_POS_SIZE, pos_size);
    }

    pub fn main_window_maximized(&self) -> Option<bool> {
        self.key_file.boolean(groups::UI, keys::MAIN_WINDOW_MAXIMIZED).ok()
    }

    pub fn set_main_window_maximized(&self, value: bool) {
        self.key_file.set_boolean(groups::UI, keys::MAIN_WINDOW_MAXIMIZED, value);
    }

    pub fn main_window_paned_pos(&self) -> Option<i32> {
        self.key_file.integer(groups::UI, keys::MAIN_WINDOW_PANED_POS).ok()
    }

    pub fn set_main_window_paned_pos(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::MAIN_WINDOW_PANED_POS, value);
    }

    pub fn camera_controls_paned_pos(&self) -> Option<i32> {
        self.key_file.integer(groups::UI, keys::CAMERA_CONTROLS_PANED_POS).ok()
    }

    pub fn set_camera_controls_paned_pos(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::CAMERA_CONTROLS_PANED_POS, value)
    }

    pub fn info_overlay_font_size(&self) -> Option<f64> {
        self.key_file.double(groups::UI, keys::INFO_OVERLAY_FONT_SIZE).ok()
    }

    pub fn set_info_overlay_font_size(&self, value: f64)  {
        self.key_file.set_double(groups::UI, keys::INFO_OVERLAY_FONT_SIZE, value);
    }

    pub fn ascom_last_driver(&self) -> Option<String> {
        self.key_file.string(groups::MOUNT, keys::ASCOM_LAST_DRIVER).ok().map(|s| s.to_string())
    }

    pub fn set_ascom_last_driver(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::ASCOM_LAST_DRIVER, value);
    }

    pub fn skywatcher_last_device(&self) -> Option<String> {
        self.key_file.string(groups::MOUNT, keys::SW_LAST_DEVICE).ok().map(|s| s.to_string())
    }

    pub fn set_skywatcher_last_device(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::SW_LAST_DEVICE, value);
    }

    pub fn ioptron_last_device(&self) -> Option<String> {
        self.key_file.string(groups::MOUNT, keys::IOPTRON_LAST_DEVICE).ok().map(|s| s.to_string())
    }

    pub fn set_ioptron_last_device(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::IOPTRON_LAST_DEVICE, value);
    }

    pub fn mount_simulator_sky_rotation_dir_in_img_space(&self) -> Option<Vector2<i32>> {
        self.read_vec2(groups::MOUNT, keys::SIM_SKY_ROTATION_DIR_IN_IMG_SPACE)
    }

    pub fn mount_simulator_primary_axis_slew_dir_in_img_space(&self) -> Option<Vector2<i32>> {
        self.read_vec2(groups::MOUNT, keys::SIM_PRIMARY_AXIS_SLEW_DIR_IN_IMG_SPACE)
    }

    pub fn zwo_last_device(&self) -> Option<String> {
        self.key_file.string(groups::MOUNT, keys::ZWO_LAST_DEVICE).ok().map(|s| s.to_string())
    }

    pub fn set_zwo_last_device(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::ZWO_LAST_DEVICE, value);
    }

    pub fn mount_simulator_sky_rotation_speed_pix_per_sec(&self) -> Option<u32> {
        match self.key_file.integer(groups::MOUNT, keys::SIM_SKY_ROTATION_SPEED_PIX_PER_SEC) {
            Ok(value) => if value >= 0 { Some(value as u32) } else { None },
            Err(_) => None
        }
    }

    fn store_rect(&self, group: &str, key: &str, rect: gtk::Rectangle) {
        self.key_file.set_string(group, key, &format!("{};{};{};{}", rect.x, rect.y, rect.width, rect.height));
    }

    fn read_rect(&self, group: &str, key: &str) -> Option<gtk::Rectangle> {
        let rect_str = match self.key_file.string(group, key) {
            Ok(s) => s,
            Err(_) => return None
        };

        let mut numbers: Vec<i32> = vec![];
        for frag in rect_str.split(';') {
            let num = match frag.parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    println!("WARNING: invalid configuration value for {}/{}: {}", group, key, frag);
                    return None;
                }
            };
            numbers.push(num);
        }

        if numbers.len() != 4 {
            println!("WARNING: invalid configuration value for {}/{}: {}", group, key, rect_str);
            return None;
        }

        Some(gtk::Rectangle{ x: numbers[0], y: numbers[1], width: numbers[2], height: numbers[3] })
    }

    fn read_vec2(&self, group: &str, key: &str) -> Option<Vector2<i32>> {
        let vec2_str = match self.key_file.string(group, key) {
            Ok(s) => s,
            Err(_) => return None
        };

        let mut numbers: Vec<i32> = vec![];
        for frag in vec2_str.split(';') {
            let num = match frag.parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    println!("WARNING: invalid configuration value for {}/{}: {}", group, key, frag);
                    return None;
                }
            };
            numbers.push(num);
        }

        if numbers.len() != 2 {
            println!("WARNING: invalid configuration value for {}/{}: {}", group, key, vec2_str);
            return None;
        }

        Some(Vector2{ x: numbers[0], y: numbers[1] })
    }

    // TODO: encode a `Path` somehow
    // pub fn recording_dest_path(&self) -> Option<String> {
    //     self.key_file.string(groups::MAIN, keys::RECORDING_DEST_PATH).ok().map(|s| s.to_string())
    // }

    // pub fn set_recording_dest_path(&self, value: &str) {
    //     self.key_file.set_string(groups::MAIN, keys::RECORDING_DEST_PATH, value);
    // }

    pub fn toolbar_icon_size(&self) -> Option<i32> {
        self.key_file.integer(groups::UI, keys::TOOLBAR_ICON_SIZE).ok()
    }

    pub fn set_toolbar_icon_size(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::TOOLBAR_ICON_SIZE, value)
    }

    pub fn disabled_drivers(&self) -> String {
        self.key_file.string(groups::MAIN, keys::DISABLED_DRIVERS)
            .ok()
            .map(|s| s.to_string())
            .unwrap_or("".to_string())
    }

    pub fn preview_fps_limit(&self) -> Option<i32> {
        match self.key_file.integer(groups::MAIN, keys::PREVIEW_FPS_LIMIT) {
            Ok(value) => if value > 0 {
                Some(value)
            } else {
                println!("WARNING: invalid configuration value for {}/{}: {}", groups::MAIN, keys::PREVIEW_FPS_LIMIT, value);
                None
            },

            _ => Some(DEFAULT_PREVIEW_FPS_LIMIT)
        }
    }

    pub fn simulator_video_file(&self) -> Option<std::path::PathBuf> {
        match self.key_file.string(groups::MAIN, keys::SIM_VIDEO_FILE).ok() {
            Some(s) => Some(std::path::PathBuf::from(s.as_str())),
            None => None
        }
    }
}

fn config_file_path() -> PathBuf {
    Path::new(
        &dirs::config_dir().or(Some(Path::new("").to_path_buf())).unwrap()
    ).join("vidoxide.cfg")
}
