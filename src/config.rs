//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Program configuration.
//!

use std::path::{Path, PathBuf};

mod groups {
    pub const UI: &str = "UI";
    pub const MOUNT: &str = "Mount";
    pub const MAIN: &str = "Main";
}

mod keys {
    pub const RECORDING_DEST_PATH: &str = "RecordingDestPath";

    pub const MAIN_WINDOW_POS_SIZE: &str = "MainWindowPosSize";
    pub const MAIN_WINDOW_MAXIMIZED: &str = "MainWindowMaximized";
    /// Position of the divider between preview area and controls panel.
    pub const MAIN_WINDOW_PANED_POS: &str = "MainWindowPanedPos";
    /// Position of the divider between camera controls and histogram.
    pub const CAMERA_CONTROLS_PANED_POS: &str = "CameraControlsPanedPos";
    pub const INFO_OVERLAY_FONT_SIZE: &str = "InfoOverlayFontSize";
    pub const TOOLBAR_ICON_SIZE: &str = "ToolbarIconSize";

    pub const SW_LAST_DEVICE: &str = "SkyWatcherLastDevice";
    pub const ASCOM_LAST_DRIVER: &str = "AscomLastDriver";
}

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

    pub fn main_window_pos(&self) -> Option<gtk::Rectangle> {
        self.read_rect(groups::UI, keys::MAIN_WINDOW_POS_SIZE)
    }

    pub fn set_main_window_pos(&self, pos_size: gtk::Rectangle) {
        self.store_rect(groups::UI, keys::MAIN_WINDOW_POS_SIZE, pos_size);
    }

    pub fn main_window_maximized(&self) -> Option<bool> {
        self.key_file.get_boolean(groups::UI, keys::MAIN_WINDOW_MAXIMIZED).ok()
    }

    pub fn set_main_window_maximized(&self, value: bool) {
        self.key_file.set_boolean(groups::UI, keys::MAIN_WINDOW_MAXIMIZED, value);
    }

    pub fn main_window_paned_pos(&self) -> Option<i32> {
        self.key_file.get_integer(groups::UI, keys::MAIN_WINDOW_PANED_POS).ok()
    }

    pub fn set_main_window_paned_pos(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::MAIN_WINDOW_PANED_POS, value);
    }

    pub fn camera_controls_paned_pos(&self) -> Option<i32> {
        self.key_file.get_integer(groups::UI, keys::CAMERA_CONTROLS_PANED_POS).ok()
    }

    pub fn set_camera_controls_paned_pos(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::CAMERA_CONTROLS_PANED_POS, value)
    }

    pub fn info_overlay_font_size(&self) -> Option<f64> {
        self.key_file.get_double(groups::UI, keys::INFO_OVERLAY_FONT_SIZE).ok()
    }

    pub fn set_info_overlay_font_size(&self, value: f64)  {
        self.key_file.set_double(groups::UI, keys::INFO_OVERLAY_FONT_SIZE, value);
    }

    pub fn ascom_last_driver(&self) -> Option<String> {
        self.key_file.get_string(groups::MOUNT, keys::ASCOM_LAST_DRIVER).ok().map(|s| s.to_string())
    }

    pub fn set_ascom_last_driver(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::ASCOM_LAST_DRIVER, value);
    }

    pub fn skywatcher_last_device(&self) -> Option<String> {
        self.key_file.get_string(groups::MOUNT, keys::SW_LAST_DEVICE).ok().map(|s| s.to_string())
    }

    pub fn set_skywatcher_last_device(&self, value: &str) {
        self.key_file.set_string(groups::MOUNT, keys::SW_LAST_DEVICE, value);
    }

    fn store_rect(&self, group: &str, key: &str, rect: gtk::Rectangle) {
        self.key_file.set_string(group, key, &format!("{};{};{};{}", rect.x, rect.y, rect.width, rect.height));
    }

    fn read_rect(&self, group: &str, key: &str) -> Option<gtk::Rectangle> {
        let rect_str = match self.key_file.get_string(group, key) {
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

    pub fn recording_dest_path(&self) -> Option<String> {
        self.key_file.get_string(groups::MAIN, keys::RECORDING_DEST_PATH).ok().map(|s| s.to_string())
    }

    pub fn set_recording_dest_path(&self, value: &str) {
        self.key_file.set_string(groups::MAIN, keys::RECORDING_DEST_PATH, value);
    }

    pub fn toolbar_icon_size(&self) -> Option<i32> {
        self.key_file.get_integer(groups::UI, keys::TOOLBAR_ICON_SIZE).ok()
    }

    pub fn set_toolbar_icon_size(&self, value: i32) {
        self.key_file.set_integer(groups::UI, keys::TOOLBAR_ICON_SIZE, value)
    }
}

fn config_file_path() -> PathBuf {
    Path::new(
        &dirs::config_dir().or(Some(Path::new("").to_path_buf())).unwrap()
    ).join("vidoxide.cfg")
}
