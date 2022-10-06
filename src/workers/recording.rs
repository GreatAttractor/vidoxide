//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Recording thread.
//!

use crate::output::OutputWriter;
use crossbeam;
use ga_image::point::Rect;
use ga_image::{Image, ImageView};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

pub const MAX_BUFFERED_KIB: isize = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum Limit {
    FrameCount(usize),
    Duration(std::time::Duration),
    Forever
}

#[derive(Debug)]
struct CfaState {
    crop_fragment_even_x_offset: bool,
    crop_fragment_even_y_offset: bool
}

#[derive(Debug)]
pub struct Job {
    receiver: crossbeam::channel::Receiver<CaptureToRecordingThreadMsg>,
    writer: Box<dyn OutputWriter>,
    cfa_state: Option<CfaState>
}

impl Job {
    pub fn new(
        receiver: crossbeam::channel::Receiver<CaptureToRecordingThreadMsg>,
        writer: Box<dyn OutputWriter>
    ) -> Job {
        Job{ receiver, writer, cfa_state: None }
    }
}

pub enum CaptureToRecordingThreadMsg {
    /// Contains (image, fragment to record, capture timestamp).
    Captured((Arc<Image>, Rect, std::time::SystemTime)),
    Finished
}

pub enum RecordingToMainThreadMsg {
    Info(String),
    Error(String),
    CaptureThreadEnded
}

pub enum MainToRecordingThreadMsg {
    CheckJobQueue,
    Finish
}

pub fn recording_thread(
    jobs: Arc<crossbeam::queue::SegQueue<Job>>,
    sender: glib::Sender<RecordingToMainThreadMsg>,
    receiver_main: crossbeam::channel::Receiver<MainToRecordingThreadMsg>,
    buffered_kib: Arc<AtomicIsize>
) {
    const RECEIVED_FROM_MAIN_THREAD: usize = 0;
    const RECEIVED_FROM_CAPTURE_THREAD: usize = 1;

    let mut job: Option<Job> = None;

    let mut t_last_info_sent = std::time::Instant::now();
    let mut last_kib_written = 0;
    let mut total_kib_written = 0;
    let mut written_kib_since_update = 0;

    macro_rules! end_job { () => {
        match job.as_mut().unwrap().writer.finalize() {
            Err(err) => sender.send(RecordingToMainThreadMsg::Error(err)).unwrap(),
            _ => ()
        }

        job = jobs.pop();
    }}

    loop {
        let mut sel = crossbeam::channel::Select::new();
        sel.recv(&receiver_main);
        if job.is_some() { sel.recv(&job.as_ref().unwrap().receiver); }

        let sel_result = sel.select();
        match sel_result.index() {
            RECEIVED_FROM_MAIN_THREAD => match sel_result.recv(&receiver_main).unwrap() {
                MainToRecordingThreadMsg::CheckJobQueue => if job.is_none() {
                    if let Some(new_job) = jobs.pop() { job = Some(new_job); }
                },

                MainToRecordingThreadMsg::Finish => break
            },

            RECEIVED_FROM_CAPTURE_THREAD => match sel_result.recv(&job.as_ref().unwrap().receiver) {
                Ok(msg) => match msg {
                    CaptureToRecordingThreadMsg::Captured((image, mut fragment, _timestamp)) => {
                        let mut job = job.as_mut().unwrap();

                        let mut diag_error = None; //TODO: remove once diagnosed

                        match &job.cfa_state {
                            None => {
                                if image.pixel_format().is_cfa() {
                                    job.cfa_state = Some(CfaState{
                                        crop_fragment_even_x_offset: fragment.x % 2 == 0,
                                        crop_fragment_even_y_offset: fragment.y % 2 == 0
                                    });
                                }
                            },

                            // Care needs to be taken when recording CFA images with live cropping.
                            // The crop fragment's offset can only be allowed to change with 2-pixel granularity
                            // in each frame, so that always the same resulting CFA pattern is reported to the
                            // output writer.
                            Some(state) => {
                                if state.crop_fragment_even_x_offset && fragment.x % 2 == 1 {
                                    fragment.x -= 1;
                                } else if !state.crop_fragment_even_x_offset && fragment.x % 2 == 0 {
                                    if fragment.x > 0 { fragment.x -= 1; } else { fragment.x += 1; }
                                }

                                if state.crop_fragment_even_y_offset && fragment.y % 2 == 1 {
                                    fragment.y -= 1;
                                } else if !state.crop_fragment_even_y_offset && fragment.y % 2 == 0 {
                                    if fragment.y > 0 { fragment.y -= 1; } else { fragment.y += 1; }
                                }

                                //TODO: remove this once raw color recording issue is diagnosed
                                // fragment.width = (image.width() - fragment.x as u32).min(fragment.width);
                                // fragment.height = (image.height() - fragment.y as u32).min(fragment.height);
                                if !(*image).img_rect().contains_rect(&fragment) {
                                    diag_error = Some(format!("attempted to record fragment {:?} of image sized {}x{}", fragment, image.width(), image.height()));
                                }
                                //END TODO
                            }
                        }

                        //TODO: remove once diagnosed
                        if let Some(e) = diag_error {
                            sender.send(RecordingToMainThreadMsg::Error(e)).unwrap();
                            end_job!();
                        } else {
                        //END TODO
                            match job.writer.write(&ImageView::new(&*image, Some(fragment))) {
                                Err(err) => {
                                    sender.send(RecordingToMainThreadMsg::Error(err)).unwrap();
                                    end_job!();
                                },

                                Ok(()) => {
                                    let kib_written = image.num_pixel_bytes_without_padding() / 1024;
                                    total_kib_written += kib_written;
                                    written_kib_since_update += kib_written;
                                }
                            }
                        }
                    },

                    CaptureToRecordingThreadMsg::Finished => { end_job!(); }
                },

                Err(_) => {
                    println!("WARNING: Capture thread ended with error; current recording job ends.");
                    sender.send(RecordingToMainThreadMsg::CaptureThreadEnded).unwrap();

                    match job.as_mut().unwrap().writer.finalize() {
                        Err(err) => sender.send(RecordingToMainThreadMsg::Error(err)).unwrap(),
                        _ => ()
                    }
                    job = jobs.pop();
                }
            },

            _ => unreachable!()
        }

        if jobs.len() == 0 && job.is_none() {
            sender.send(RecordingToMainThreadMsg::Info("Recording jobs: 0".to_string())).unwrap();
        }

        let t_elapsed = t_last_info_sent.elapsed();
        if t_elapsed >= std::time::Duration::from_secs(1) {
            let num_jobs = jobs.len() + if job.is_some() { 1 } else { 0 };
            let write_rate = (total_kib_written - last_kib_written) as f64 / 1024.0 / t_elapsed.as_secs_f64();
            let current_buffered = buffered_kib.load(Ordering::Relaxed);
            let info = if current_buffered >= 0 as isize {
                format!(
                    "Recording jobs: {}; saving at {:.1} MiB/s; buffered: {} MiB",
                    num_jobs,
                    write_rate,
                    current_buffered / 1024
                )
            } else {
                format!(
                    "Recording jobs: {}; saving at {:.1} MiB/s",
                    num_jobs,
                    write_rate
                )
            };
            sender.send(RecordingToMainThreadMsg::Info(info)).unwrap();
            t_last_info_sent = std::time::Instant::now();
            last_kib_written = total_kib_written;

            buffered_kib.fetch_sub(written_kib_since_update as isize, Ordering::Relaxed);
            written_kib_since_update = 0;
        }
    }
}
