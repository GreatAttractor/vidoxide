//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Recording output: SER video.
//!

use crate::output::OutputWriter;
use ga_image::ImageView;
use ga_image::utils;
use ga_image;
use std::io::{BufWriter, Seek, SeekFrom, Write};

#[derive(PartialEq)]
enum SerColorFormat {
    Mono      = 0,
    BayerRGGB = 8,
    BayerGRBG = 9,
    BayerGBRG = 10,
    BayerBGGR = 11,
    BayerCYYM = 16,
    BayerYCMY = 17,
    BayerYMCY = 18,
    BayerMYYC = 19,
    RGB       = 100,
    BGR       = 101
}

// see comment for `SerHeader::little_endian`
const SER_LITTLE_ENDIAN: u32 = 0;
const SER_BIG_ENDIAN: u32 = 1;

macro_rules! str_as_byte_array {
    ($string:expr, $len:expr) => {
        {
            let mut array = [0u8; $len];
            let bytes = $string.as_bytes();
            for i in 0..std::cmp::min($len, $string.len()) {
                array[i] = bytes[i];
            }
            array
        }
    }
}

#[repr(C, packed)]
struct SerHeader {
    signature: [u8; 14],
    camera_series_id: u32,
    color_id: u32,
    // Online documentation claims this is 0 when 16-bit pixel data
    // is big-endian, but the meaning is actually reversed.
    little_endian: u32,
    img_width: u32,
    img_height: u32,
    bits_per_channel: u32,
    frame_count: u32,
    observer: [u8; 40],
    instrument: [u8; 40],
    telescope: [u8; 40],
    date_time: i64,
    date_time_utc: i64
}

#[derive(Debug)]
pub struct SerVideo {
    writer: std::io::BufWriter<std::fs::File>,
    /// Frame width, height, pixel format.
    frame_format: Option<(u32, u32, ga_image::PixelFormat)>,
    frame_count: u32
}

impl SerVideo {
    pub fn new(file: std::fs::File) -> SerVideo {
        SerVideo{ writer: BufWriter::new(file), frame_format: None, frame_count: 0 }
    }
}

impl OutputWriter for SerVideo {
    fn write(&mut self, image: &ImageView) -> Result<(), String> {
        match self.frame_format {
            None => {
                self.frame_format = Some((image.width(), image.height(), image.pixel_format()));

                let is_machine_big_endian = 0x1122u16.to_be() == 0x1122;

                let ser_header = SerHeader{
                    signature: str_as_byte_array!("Vidoxide", 14),
                    camera_series_id: 0,
                    color_id: match image.pixel_format() {
                        ga_image::PixelFormat::Mono8 | ga_image::PixelFormat::Mono16 => (SerColorFormat::Mono as u32).to_le(),
                        ga_image::PixelFormat::RGB8 | ga_image::PixelFormat::RGB16 => (SerColorFormat::RGB as u32).to_le(),
                        ga_image::PixelFormat::CfaRGGB8 | ga_image::PixelFormat::CfaRGGB16 => (SerColorFormat::BayerRGGB as u32).to_le(),
                        ga_image::PixelFormat::CfaGRBG8 | ga_image::PixelFormat::CfaGRBG16 => (SerColorFormat::BayerGRBG as u32).to_le(),
                        ga_image::PixelFormat::CfaGBRG8 | ga_image::PixelFormat::CfaGBRG16 => (SerColorFormat::BayerGBRG as u32).to_le(),
                        ga_image::PixelFormat::CfaBGGR8 | ga_image::PixelFormat::CfaBGGR16 => (SerColorFormat::BayerBGGR as u32).to_le(),
                        other => panic!("Recording {:?} as SER video not implemented yet.", other)
                    },
                    little_endian: if is_machine_big_endian { SER_BIG_ENDIAN.to_le() } else { SER_LITTLE_ENDIAN.to_le() },
                    img_width: image.width().to_le(),
                    img_height: image.height().to_le(),
                    bits_per_channel: (image.pixel_format().bytes_per_channel() as u32 * 8).to_le(),
                    frame_count: 0, // will be updated when recording ends
                    observer: [0; 40],   //
                    instrument: [0; 40], // TODO: set something here
                    telescope: [0; 40],  //
                    date_time: 0, // TODO: support the timestamp and timestamp file trailer
                    date_time_utc: 0
                };
                match utils::write_struct(&ser_header, &mut self.writer) {
                    Ok(_) => (),
                    Err(err) => return Err(format!("error writing SER header: {:?}", err))
                }
            }
            Some(f) => if image.width() != f.0 ||
                          image.height() != f.1 ||
                          image.pixel_format() != f.2 {
                return Err(format!("unexpected frame: {}x{}, {:?} (expected {}x{}, {:?})",
                    image.width(), image.height(), image.pixel_format(),
                    f.0, f.1, f.2)
                );
            }
        }

        for y in 0..image.height() {
            let line = image.line_raw(y);
            match self.writer.write_all(line) {
                Err(err) => return Err(format!("{:?}", err)),
                Ok(()) => ()
            }
        }

        self.writer.flush().unwrap();

        self.frame_count += 1;

        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        match self.writer.seek(SeekFrom::Start(38 /* offset of `frame_count` */)) {
            Err(err) => { return Err(format!("I/O error: {:?}", err)); },
            _ => ()
        }
        match utils::write_struct(&self.frame_count.to_le(), &mut self.writer) {
            Err(err) => { return Err(format!("I/O error: {:?}", err)); },
            _ => ()
        }

        Ok(())
    }
}
