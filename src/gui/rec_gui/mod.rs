//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Recording GUI.
//!

mod time_widget;

use crate::gui::camera_gui::{ControlWidgetBundle};
use crate::gui::actions;
use crate::output;
use crate::output::{OutputFormat};
use crate::ProgramData;
use crate::timer::OneShotTimer;
use crate::workers::capture::MainToCaptureThreadMsg;
use crate::workers::recording;
use crate::workers::recording::{Limit, MainToRecordingThreadMsg};
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use strum::IntoEnumIterator;
use super::show_message;
use time_widget::TimeWidget;

/// Control padding in pixels.
const PADDING: u32 = 10;

pub struct RecWidgets {
    btn_record: gtk::Button,
    btn_stop: gtk::Button,
    name_prefix: gtk::Entry,
    dest_dir: gtk::FileChooserButton,
    output_fmt_getter: Box<dyn Fn() -> output::OutputFormat>,
    rec_limit_getter: Box<dyn Fn() -> recording::Limit>,
    /// Returns (sequence count, sequence interval).
    sequence_getter: Box<dyn Fn() -> (usize, std::time::Duration)>,
    pub sequence_idx: usize,
    pub sequence_next_start: Option<std::time::Instant>,
    sequence_timer: OneShotTimer,
    others: gtk::Box,
}

impl RecWidgets {
    pub fn on_disconnect(&self) {
        self.btn_record.set_sensitive(false);
        self.btn_stop.set_sensitive(false);
        self.others.set_sensitive(false);
    }

    pub fn on_connect(&self) {
        self.btn_record.set_sensitive(true);
        self.btn_stop.set_sensitive(false);
        self.others.set_sensitive(true);
    }

    pub fn on_start_recording(&self) {
        self.btn_record.set_sensitive(false);
        self.btn_stop.set_sensitive(true);
        self.others.set_sensitive(false);
    }

    pub fn on_stop_recording(&mut self) {
        self.sequence_timer.stop();
        self.btn_record.set_sensitive(true);
        self.btn_stop.set_sensitive(false);
        self.others.set_sensitive(true);
    }

    pub fn on_recording_ended(&self) {
        self.btn_record.set_sensitive(true);
        self.btn_stop.set_sensitive(false);
        self.others.set_sensitive(true);
    }

    pub fn rec_limit(&self) -> recording::Limit {
        (*self.rec_limit_getter)()
    }

    pub fn dest_dir(&self) -> String {
        self.dest_dir.filename().unwrap().as_path().to_str().unwrap().to_string()
    }

    pub fn name_prefix(&self) -> String {
        self.name_prefix.text().as_str().to_string()
    }

    /// Returns (sequence count, sequence interval).
    pub fn sequence(&self) -> (usize, std::time::Duration) {
        (*self.sequence_getter)()
    }
}

fn on_start_recording(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let dest_path;
    let rec_limit;
    let output_fmt;
    let name_prefix;
    let sequence_suffix;

    {
        let mut program_data = program_data_rc.borrow_mut();
        let rec_widgets = &mut program_data.gui.as_mut().unwrap().rec_widgets;
        let (sequence_count, _) = rec_widgets.sequence();
        sequence_suffix = if sequence_count > 1 { format!("_{:05}", rec_widgets.sequence_idx + 1) } else { "".to_string() };
        name_prefix = rec_widgets.name_prefix();

        output_fmt = (*rec_widgets.output_fmt_getter)();

        dest_path = {
            match output_fmt {
                OutputFormat::SerVideo | OutputFormat::AviVideo => {
                    let dest_fname = name_prefix.clone() + &sequence_suffix +
                        if output_fmt == OutputFormat::AviVideo { ".avi" } else { ".ser" };
                    Path::new(&rec_widgets.dest_dir()).join(dest_fname).to_str().unwrap().to_string()
                },
                OutputFormat::BmpSequence | OutputFormat::TiffSequence => {
                    rec_widgets.dest_dir()
                }
            }
        };

        rec_limit = program_data.gui.as_ref().unwrap().rec_widgets.rec_limit();
    } // end of mutable borrow of `program_data_rc` (we must end it before showing a modal dialog by `show_message`)

    if Path::new(&dest_path).exists() && !output_fmt.is_image_sequence() {
        show_message(&format!("File already exists:\n{}", dest_path), "Error", gtk::MessageType::Error);
        return;
    }

    let (rec_sender, rec_receiver) = crossbeam::channel::unbounded();

    if program_data_rc.borrow_mut().capture_thread_data.as_ref().unwrap().sender.send(
        MainToCaptureThreadMsg::StartRecording((rec_sender, rec_limit))
    ).is_err() {
        crate::on_capture_thread_failure(program_data_rc);
        return;
    }

    let writer: Box<dyn output::OutputWriter> = match output_fmt {
        OutputFormat::AviVideo | OutputFormat::SerVideo => {
            let file = std::fs::OpenOptions::new().read(false).write(true).create(true).open(&dest_path).unwrap();
            if output_fmt == OutputFormat::AviVideo {
                show_message("Recording as AVI not implemented yet.", "Error", gtk::MessageType::Error);
                return;
            } else {
                Box::new(output::ser::SerVideo::new(file))
            }
        },

        OutputFormat::BmpSequence | OutputFormat::TiffSequence => {
            Box::new(output::file_seq::FileSequence::new(
                &dest_path, &(name_prefix + &sequence_suffix), output_fmt.file_type()
            ))
        }
    };

    let new_job = recording::Job{ receiver: rec_receiver, writer };

    let mut program_data = program_data_rc.borrow_mut();
    program_data.recording_thread_data.jobs.push(new_job);
    program_data.rec_job_active = true;
    program_data.recording_thread_data.sender.send(MainToRecordingThreadMsg::CheckJobQueue).unwrap();

    program_data.gui.as_ref().unwrap().rec_widgets.on_start_recording();

    save_camera_controls_state(&dest_path, &program_data);
}

/// Saves current data & time, camera name and camera controls' state to a text file
/// at the same directory as `rec_dest_path`.
fn save_camera_controls_state(rec_dest_path: &str, program_data: &ProgramData) {
    let parent_dir = match Path::new(rec_dest_path).parent() {
        Some(p) => p.to_str().unwrap().to_string(),
        None => "".to_string()
    };
    let dest_fpath = Path::new(&parent_dir).join(
        Path::new(rec_dest_path).file_stem().unwrap().to_str().unwrap().to_string() + "_settings.txt"
    );
    let mut file = std::fs::OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .truncate(true)
        .open(dest_fpath)
        .unwrap();

    writeln!(file, "Recorded with Vidoxide\n").unwrap(); //TODO: print Vidoxide version
    writeln!(
        file,
        "{} ({} UTC)",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    ).unwrap();
    writeln!(file, "{}\n", program_data.camera.as_ref().unwrap().name()).unwrap();

    for ctrl_widgets in &program_data.gui.as_ref().unwrap().control_widgets {
        if !(ctrl_widgets.1).0.h_box.get_visible() {
            continue;
        }

        write!(file, "{}: ", (ctrl_widgets.1).0.name).unwrap();

        if let Some(auto) = &(ctrl_widgets.1).0.auto {
            if auto.is_active() { write!(file, "auto, ").unwrap(); }
        }

        if let Some(on_off) = &(ctrl_widgets.1).0.on_off {
            write!(file, "{}, ", if on_off.is_active() { "on" } else { "off" }).unwrap();
        }

        match &(ctrl_widgets.1).1 {
            ControlWidgetBundle::ListControl(list_ctrl) =>
                write!(file, "{}", list_ctrl.combo.active_text().unwrap()).unwrap(),

            ControlWidgetBundle::NumberControl(num_ctrl) =>
                write!(file, "{:.6}", num_ctrl.slider.borrow().get().value()).unwrap(),

            ControlWidgetBundle::BooleanControl(bool_ctrl) =>
                write!(file, "{}", if bool_ctrl.state_checkbox.is_active() { "true" } else { "false" }).unwrap()
        }

        write!(file, "\n").unwrap();
    }

}

pub fn on_stop_recording(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut program_data = program_data_rc.borrow_mut();

    let _ = program_data.capture_thread_data.as_ref().unwrap().sender.send(MainToCaptureThreadMsg::StopRecording);
    program_data.rec_job_active = false;

    let pd_gui = program_data.gui.as_mut().unwrap();
    pd_gui.status_bar.current_recording_info.set_label(&"");
    pd_gui.rec_widgets.on_stop_recording();
    pd_gui.rec_widgets.sequence_next_start = None;
}

/// Returns (top-level box, RecWidgets).
pub fn create_recording_panel(program_data_rc: &Rc<RefCell<ProgramData>>) -> (gtk::Box, RecWidgets) {
    let btn_record = gtk::Button::with_label("⏺");
    btn_record.set_tooltip_text(Some("Start recording"));
    btn_record.set_sensitive(false);
    btn_record.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| {
        program_data_rc.borrow_mut().gui.as_mut().unwrap().rec_widgets.sequence_idx = 0;
        on_start_recording(&program_data_rc)
    }));

    let btn_stop = gtk::Button::with_label("⏹");
    btn_stop.set_tooltip_text(Some("Stop recording"));
    btn_stop.set_sensitive(false);
    btn_stop.connect_clicked(clone!(@weak program_data_rc => @default-panic, move |_| on_stop_recording(&program_data_rc)));

    let btn_snapshot = gtk::Button::with_label("✷");
    btn_snapshot.set_tooltip_text(Some("Take snapshot"));
    btn_snapshot.set_action_name(Some(&actions::prefixed(actions::TAKE_SNAPSHOT)));

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    btn_box.pack_start(&btn_record, false, false, PADDING);
    btn_box.pack_start(&btn_stop, false, false, PADDING);
    btn_box.pack_end(&btn_snapshot, false, false, PADDING);

    let others = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let dest_dir_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let dest_dir = gtk::FileChooserButton::new("Destination directory", gtk::FileChooserAction::SelectFolder);

    if let Some(prev_dest_dir) = program_data_rc.borrow().config.recording_dest_path() {
        dest_dir.set_filename(prev_dest_dir);
    } else {
        dest_dir.set_filename(".");
    }

    dest_dir_box.pack_start(&gtk::Label::new(Some("Dest. directory:")), false, false, PADDING);
    dest_dir_box.pack_start(&dest_dir, false, false, PADDING);
    others.pack_start(&dest_dir_box, false, false, PADDING);

    let prefix_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let name_prefix = gtk::EntryBuilder::new().text("rec").build();
    prefix_box.pack_start(&gtk::Label::new(Some("Name prefix:")), false, false, PADDING);
    prefix_box.pack_start(&name_prefix, false, false, PADDING);
    others.pack_start(&prefix_box, false, false, PADDING);

    let output_fmt_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    output_fmt_box.pack_start(&gtk::Label::new(Some("Output format:")), false, false, PADDING);
    let output_formats = gtk::ComboBoxText::new();
    for ofmt in OutputFormat::iter() {
        output_formats.append_text(&format!("{}", ofmt));
    }
    output_formats.set_active(Some(0));

    output_fmt_box.pack_start(&output_formats, false, false, PADDING);
    others.pack_start(&output_fmt_box, false, false, PADDING);

    let limit_frame = gtk::Frame::new(Some("Limit"));
    let frame_contents = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let box_limit_duration = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let rb_limit_duration = gtk::RadioButton::with_label("duration:");
    box_limit_duration.pack_start(&rb_limit_duration, false, false, PADDING);
    let duration_widget = TimeWidget::new_with_value(0, 0, 10);
    box_limit_duration.pack_start(duration_widget.get(), false, false, PADDING);
    frame_contents.pack_start(&box_limit_duration, false, false, PADDING);

    let box_limit_frames = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let rb_limit_frames = gtk::RadioButton::with_label_from_widget(&rb_limit_duration, "frames:");
    box_limit_frames.pack_start(&rb_limit_frames, false, false, PADDING);
    let sb_limit_frames = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(100.0, 1.0, 1_000_000.0, 1.0, 10.0, 0.0)), 1.1, 0
    );
    box_limit_frames.pack_start(&sb_limit_frames, false, false, PADDING);
    frame_contents.pack_start(&box_limit_frames, false, false, PADDING);

    // put it in a box to match margins of the previous radio buttons
    let box_limit_forever = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let rb_limit_forever = gtk::RadioButton::with_label_from_widget(&rb_limit_duration, "record forever");
    box_limit_forever.pack_start(&rb_limit_forever, false, false, PADDING);
    frame_contents.pack_start(&box_limit_forever, false, false, PADDING);

    limit_frame.add(&frame_contents);

    others.pack_start(&limit_frame, false, false, PADDING);

    let box_sequence = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    box_sequence.pack_start(&gtk::Label::new(Some("Record")), false, false, PADDING);
    let btn_rec_count = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(1.0, 1.0, 1_000_000.0, 1.0, 10.0, 0.0)), 1.0, 0
    );
    btn_rec_count.set_orientation(gtk::Orientation::Vertical);
    box_sequence.pack_start(&btn_rec_count, false, false, PADDING);
    box_sequence.pack_start(&gtk::Label::new(Some("time(s) with interval")), false, false, PADDING);
    let sequence_interval = TimeWidget::new_with_value(0, 0, 10);
    box_sequence.pack_start(sequence_interval.get(), false, false, PADDING);

    others.pack_start(&box_sequence, false, false, PADDING);

    let box_all = gtk::Box::new(gtk::Orientation::Vertical, 0);
    box_all.pack_start(&btn_box, false, false, PADDING);
    box_all.pack_start(&others, false, false, PADDING);

    (box_all, RecWidgets{
        btn_record,
        btn_stop,
        name_prefix,
        dest_dir,
        others,
        output_fmt_getter: Box::new(
            move || {
                OutputFormat::iter().skip(output_formats.active().unwrap() as usize).next().unwrap()
            }
        ),
        rec_limit_getter: Box::new(
            move || {
                if rb_limit_duration.is_active() {
                    Limit::Duration(duration_widget.duration())
                } else if rb_limit_frames.is_active() {
                    Limit::FrameCount(sb_limit_frames.value() as usize)
                } else {
                    Limit::Forever
                }
            }
        ),
        sequence_getter: Box::new(move || (btn_rec_count.value() as usize, sequence_interval.duration())),
        sequence_idx: 0,
        sequence_next_start: None,
        sequence_timer: OneShotTimer::new()
    })
}

pub fn on_recording_finished(program_data_rc: &Rc<RefCell<ProgramData>>) {
    let mut pd = program_data_rc.borrow_mut();
    pd.rec_job_active = false;
    let pd_gui = pd.gui.as_mut().unwrap();
    pd_gui.rec_widgets.sequence_idx += 1;
    let (sequence_count, sequence_interval) = pd_gui.rec_widgets.sequence();
    if pd_gui.rec_widgets.sequence_idx < sequence_count {
        pd_gui.rec_widgets.sequence_next_start = Some(std::time::Instant::now() + sequence_interval);

        pd_gui.rec_widgets.sequence_timer.run_once(sequence_interval, clone!(@weak program_data_rc
            => @default-panic, move || {
                program_data_rc.borrow_mut().gui.as_mut().unwrap().rec_widgets.sequence_next_start = None;
                on_start_recording(&program_data_rc);
            }
        ));
    } else {
        pd_gui.rec_widgets.on_recording_ended();
        pd_gui.status_bar.current_recording_info.set_label(&"");
    }
}
