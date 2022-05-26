//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Recording output module.
//!

pub mod file_seq;
pub mod ser;

use ga_image::ImageView;

pub trait OutputWriter: std::fmt::Debug + Send {
    #[must_use]
    fn write(&mut self, image: &ImageView) -> Result<(), String>;

    #[must_use]
    fn finalize(&mut self) -> Result<(), String>;
}

#[derive(Debug, PartialEq, strum_macros::EnumIter)]
pub enum OutputFormat {
    SerVideo,
    AviVideo,
    BmpSequence,
    TiffSequence
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            OutputFormat::SerVideo => "SER video",
            OutputFormat::AviVideo => "AVI video",
            OutputFormat::BmpSequence => "Image sequence (BMP)",
            OutputFormat::TiffSequence => "Image sequence (TIFF)"
        })
    }
}

impl OutputFormat {
    pub fn file_type(&self) -> ga_image::FileType {
        match self {
            OutputFormat::BmpSequence => ga_image::FileType::Bmp,
            OutputFormat::TiffSequence => ga_image::FileType::Tiff,

            _ => panic!("Not an image sequence: {:?}", self)
        }
    }

    pub fn is_image_sequence(&self) -> bool {
        match self {
            OutputFormat::SerVideo => false,
            OutputFormat::AviVideo => false,
            OutputFormat::BmpSequence => true,
            OutputFormat::TiffSequence => true
        }
    }
}
