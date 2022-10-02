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

enum Contents {
    Paths(Vec<std::path::PathBuf>),
    Images(Vec<ga_image::Image>)
}

struct ImageList {
    contents: Contents
}

pub fn create_image_list_from_paths(file_paths: Vec<std::path::PathBuf>) -> Box<dyn ImageSequence> {
    Box::new(ImageList{ contents: Contents::Paths(file_paths) })
}

pub fn create_image_list(images: Vec<ga_image::Image>) -> Box<dyn ImageSequence> {
    Box::new(ImageList{ contents: Contents::Images(images) })
}

impl ImageSequence for ImageList {
    fn get_image(&mut self, index: usize) -> Result<ga_image::Image, ImgSeqError> {
        let image = match &self.contents {
            Contents::Paths(paths) =>
                ga_image::Image::load(paths[index].to_str().unwrap(), ga_image::FileType::Auto).unwrap(),

            Contents::Images(images) => images[index].clone()
        };

        Ok(image)
    }

    fn num_images(&self) -> usize {
        match &self.contents {
            Contents::Paths(paths) => paths.len(),
            Contents::Images(images) => images.len()
        }
    }
}
