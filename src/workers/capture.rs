//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Capture thread.
//!

use crate::camera::CameraError;
use crate::camera::FrameCapturer;
use crate::tracking::ImageTracker;
use crate::workers::recording;
use crate::{TrackingData, TrackingMode};
use ga_image::Image;
use ga_image::point::{Point, Rect};
use ga_image;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::TryRecvError;

#[derive(Debug)]
pub struct Info {
    pub recording_info: Option<String>,
    pub capture_fps: f64
}

#[derive(Debug)]
pub enum CaptureToMainThreadMsg {
    PreviewImageReady(Arc<Image>),
    Paused,
    CaptureError(CameraError),
    RecordingFinished,
    Info(Info),
    /// Contains (updated tracking data, updated crop area).
    TrackingUpdate((TrackingData, Option<Rect>)),
    TrackingFailed
}

type RecordingSender = crossbeam::channel::Sender<recording::CaptureToRecordingThreadMsg>;

#[derive(Debug)]
pub enum MainToCaptureThreadMsg {
    Finish,
    Pause,
    Resume,
    /// Contains a sender accepting frames + capture timestamps.
    StartRecording((RecordingSender, recording::Limit)),
    StopRecording,
    EnableCentroidTracking(Rect),
    EnableAnchorTracking(Point),
    EnableRecordingCrop(Rect),
}

struct RecData {
    sender: RecordingSender,
    limit: recording::Limit,
    tstart: std::time::Instant,
    frame_counter: usize
}

/// Specifies live cropping parameters to use for recording.
struct CropData {
    /// If `Some`, crop area follows the tracking position.
    tracking_pos_offset: Option<Point>,
    area: Rect
}

pub fn capture_thread(
    mut camera: Box<dyn FrameCapturer + Send>,
    sender: glib::Sender<CaptureToMainThreadMsg>,
    receiver: std::sync::mpsc::Receiver<MainToCaptureThreadMsg>,
    buffered_kib: Arc<AtomicIsize>,
    new_preview_wanted: Arc<AtomicBool>
) {
    // To avoid unneccessary allocations, we (the capture thread) have two `Arc`-wrapped capture buffers.
    // One is provided to the main thread for preview, the other to the recording thread (if recording is in progress).
    // We only start allocating more buffers if the recording thread cannot keep up (e.g., due to slow I/O) with
    // the framerate (otherwise it will drop its `Arc` copy before we receive a new frame).
    //
    // If we are the sole owner of one of the buffers, it is used as the capture destination. If both are being shared,
    // it means the main thread and the recording thread still hold their copies and we must allocate a new one
    // to capture into. We send a buffer to the main thread only if it has already released the previously sent buffer.
    //
    // Initially create dummy 1x1 images; they will be resized and set up correctly by `capture_frame`
    // (also after each video mode/ROI size/pixel format change).
    //
    let img0 = Image::new(1, 1, None, ga_image::PixelFormat::Mono8, None, false);
    let mut capture_buf = [
        Arc::new(img0.clone()),
        Arc::new(img0)
    ];

    let mut paused = false;

    let mut rec_data: Option<RecData> = None;

    let mut tracking: Option<ImageTracker> = None;

    let mut t_last_info = std::time::Instant::now();

    let mut num_dropped_frames = 0;

    let mut most_recently_captured_buf_idx: Option<usize> = None;

    let mut crop_data: Option<CropData> = None;

    let mut fps_counter: i32 = 0;

    loop {
        let recording_finished = match rec_data {
            Some(ref data) => {
                match data.limit {
                    recording::Limit::Duration(duration) => data.tstart.elapsed() >= duration,
                    recording::Limit::FrameCount(count) => data.frame_counter == count,
                    recording::Limit::Forever => false
                }
            },
            None => false
        };

        if recording_finished {
            rec_data.take().unwrap().sender.send(recording::CaptureToRecordingThreadMsg::Finished).unwrap();
            sender.send(CaptureToMainThreadMsg::RecordingFinished).unwrap();
            num_dropped_frames = 0;
        }

        if !paused {
            let (current_buf_idx, capture_result) = {
                let (current_buf_idx, dest_img) = match Arc::get_mut(&mut capture_buf[0]) {
                    Some(img) => (0, img),
                    None => match Arc::get_mut(&mut capture_buf[1]) {
                        Some(img) => (1, img),
                        None => {
                            // both buffers are being used; allocate a new one and replace the buffer
                            // sent to the recording thread
                            capture_buf[1] = Arc::new((*capture_buf[1]).clone());
                            (1, Arc::get_mut(&mut capture_buf[1]).unwrap())
                        }
                    }
                };

                (current_buf_idx, camera.capture_frame(dest_img))
            };

            fps_counter += 1;

            most_recently_captured_buf_idx = Some(current_buf_idx);

            let mut info: Option<Info> = None;
            if t_last_info.elapsed() >= std::time::Duration::from_secs(1) {
                info = Some(Info{ capture_fps: fps_counter as f64, recording_info: None });
                fps_counter = 0;
                t_last_info = std::time::Instant::now();
            }


            match capture_result {
                Err(err) => match err {
                    CameraError::FrameUnavailable => (/* no error, do nothing */),
                    other_err => {
                        sender.send(CaptureToMainThreadMsg::CaptureError(other_err)).unwrap();
                        break;
                    }
                },
                Ok(()) => {
                    if let Some(ref mut rec_data) = rec_data {
                        on_recording(
                            rec_data,
                            &capture_buf[current_buf_idx],
                            &buffered_kib,
                            info.as_mut(),
                            &mut num_dropped_frames,
                            &crop_data
                        );
                    }

                    if let Some(ref mut tracker) = tracking {
                        if on_tracking(tracker, &capture_buf[current_buf_idx], &sender, &mut crop_data).is_err() {
                            tracking = None;
                        }
                    }

                    if new_preview_wanted.swap(false, Ordering::Relaxed) == true {
                        match sender.send(
                            CaptureToMainThreadMsg::PreviewImageReady(Arc::clone(&capture_buf[current_buf_idx]))
                        ) {
                            Ok(()) => (),
                            Err(err) => panic!("Capture thread: unexpected sender error {:?}.", err)
                        }
                    }
                }
            }

            if let Some(info) = info {
                sender.send(CaptureToMainThreadMsg::Info(info)).unwrap();
            }
        }

        match receiver.try_recv() {
            Err(e) => if e != TryRecvError::Empty {
                panic!("Capture thread: unexpected receiver error {:?}.", e)
            },
            Ok(msg) => match msg {
                MainToCaptureThreadMsg::Finish => break,

                MainToCaptureThreadMsg::Pause => {
                    camera.pause().unwrap();
                    sender.send(CaptureToMainThreadMsg::Paused).unwrap();
                    paused = true;
                },

                MainToCaptureThreadMsg::Resume => {
                    camera.resume().unwrap();
                    paused = false;
                },

                MainToCaptureThreadMsg::StartRecording((sender, limit)) => {
                    rec_data = Some(RecData{ sender, limit, tstart: std::time::Instant::now(), frame_counter: 0 });
                },

                MainToCaptureThreadMsg::StopRecording => {
                    if rec_data.is_some() {
                        rec_data.take().unwrap().sender.send(recording::CaptureToRecordingThreadMsg::Finished).unwrap();
                    }
                },

                MainToCaptureThreadMsg::EnableCentroidTracking(rect) => {
                    if let Some(idx) = most_recently_captured_buf_idx {
                        tracking = Some(ImageTracker::new_with_centroid(rect, &capture_buf[idx]));
                    }
                },

                MainToCaptureThreadMsg::EnableAnchorTracking(pos) => {
                    if let Some(idx) = most_recently_captured_buf_idx {
                        tracking = Some(ImageTracker::new_with_anchor(pos, &capture_buf[idx]));
                    }
                },

                MainToCaptureThreadMsg::EnableRecordingCrop(area) => {
                    let tracking_pos_offset: Option<Point> = match &tracking {
                        Some(tracking) => Some(area.pos() - tracking.position().unwrap()),
                        None => None
                    };

                    crop_data = Some(CropData{ tracking_pos_offset, area });
                }
            }
        }
    }

    if let Some(data) = rec_data {
        data.sender.send(recording::CaptureToRecordingThreadMsg::Finished).unwrap();
    }
}

fn on_tracking(
    tracking: &mut ImageTracker,
    image: &Arc<Image>,
    sender: &glib::Sender<CaptureToMainThreadMsg>,
    crop_data: &mut Option<CropData>
) -> Result<(), ()> {
    if tracking.update(image, Point{ x: 0, y: 0 }).is_err() {
        sender.send(CaptureToMainThreadMsg::TrackingFailed).unwrap();
        Err(())
    } else {
        let tracking_pos = tracking.position().unwrap();
        let mode = if let Some(rect) = tracking.centroid_area() {
            TrackingMode::Centroid(rect)
        } else {
            TrackingMode::Anchor(tracking.position().unwrap())
        };

        if let Some(crop_data) = crop_data {
            match crop_data.tracking_pos_offset {
                None => crop_data.tracking_pos_offset = Some(crop_data.area.pos() - tracking_pos),
                Some(offs) => {
                    let mut new_pos = tracking_pos + offs;
                    let width = crop_data.area.width as i32;
                    let height = crop_data.area.height as i32;

                    if new_pos.x < 0 { new_pos.x = 0; }
                    if new_pos.y < 0 { new_pos.y = 0; }
                    if new_pos.x + width > image.width() as i32 { new_pos.x = image.width() as i32 - width; }
                    if new_pos.y + height > image.height() as i32 { new_pos.y = image.height() as i32 - height; }

                    crop_data.area.set_pos(new_pos);
                }
            }
        }

        sender.send(CaptureToMainThreadMsg::TrackingUpdate((
            TrackingData{ pos: tracking_pos, mode },
            match crop_data { Some(crop_data) => Some(crop_data.area), None => None }
        ))).unwrap();

        Ok(())
    }
}

fn on_recording(
    rec_data: &mut RecData,
    image: &Arc<Image>,
    buffered_kib: &Arc<AtomicIsize>,
    info: Option<&mut Info>,
    num_dropped_frames: &mut usize,
    crop_data: &Option<CropData>
) {
    let frame_kib_amount = image.num_pixel_bytes_without_padding() / 1024;
    if buffered_kib.load(Ordering::Relaxed) <= recording::MAX_BUFFERED_KIB {
        rec_data.sender.send(recording::CaptureToRecordingThreadMsg::Captured((
            Arc::clone(image),
            if let Some(crop_data) = crop_data { crop_data.area } else { image.img_rect() },
            std::time::SystemTime::now()
        ))).unwrap();
        rec_data.frame_counter += 1;
        buffered_kib.fetch_add(frame_kib_amount as isize, Ordering::Relaxed);
    } else {
        match rec_data.limit {
            recording::Limit::Duration(_) => *num_dropped_frames += 1,
            _ => ()
        }
    }

    if let Some(info) = info {
        info.recording_info = Some(match rec_data.limit {
            recording::Limit::FrameCount(count) => {
                format!("Recorded {}/{} frames", rec_data.frame_counter, count)
            },
            recording::Limit::Duration(duration) => {
                let total_secs_left = {
                    let elapsed = rec_data.tstart.elapsed();
                    if duration > elapsed {
                        (duration - elapsed).as_secs()
                    } else {
                        0
                    }
                };
                let hh = total_secs_left / 3600;
                let mm = (total_secs_left % 3600) / 60;
                let ss = ((total_secs_left % 3600) % 60) % 60;
                format!(
                    "Recorded {} frames ({} dropped), time left: {:02}:{:02}:{:02}",
                    rec_data.frame_counter, num_dropped_frames, hh, mm, ss
                )
            },
            recording::Limit::Forever => {
                format!("Recorded {} frames", rec_data.frame_counter)
            }
        });
    }
}
