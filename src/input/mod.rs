//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Video input module.
//!

mod image_list;
mod ser;

pub use image_list::create_image_list;
pub use ser::open_ser_video;

#[derive(Debug)]
pub struct ImgSeqError {
    description: String
}

impl ImgSeqError {
    fn new(description: String) -> ImgSeqError {
        ImgSeqError{ description }
    }
}

impl std::fmt::Display for ImgSeqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let _ = write!(f, "{}", self.description);
        Ok(())
    }
}

pub trait ImageSequence: Send {
    fn get_image(&mut self, index: usize) -> Result<ga_image::Image, ImgSeqError>;

    fn num_images(&self) -> usize;
}
