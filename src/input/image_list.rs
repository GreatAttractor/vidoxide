//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Video input: image file sequence.
//!

use crate::input::{ImageSequence, ImgSeqError};

pub fn create_image_list(file_paths: Vec<std::path::PathBuf>) -> Box<dyn ImageSequence> {
    Box::new(ImageList{ file_paths })
}

struct ImageList {
    file_paths: Vec<std::path::PathBuf>
}

impl ImageSequence for ImageList {
    fn get_image(&mut self, index: usize) -> Result<ga_image::Image, ImgSeqError> {
        let image = ga_image::Image::load(self.file_paths[index].to_str().unwrap(), ga_image::FileType::Auto).unwrap();
        Ok(image)
    }

    fn num_images(&self) -> usize { self.file_paths.len() }
}
