//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
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

#[derive(PartialEq, strum_macros::EnumIter)]
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
