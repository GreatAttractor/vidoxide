//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Recording output: image file sequence.
//!

use crate::output::OutputWriter;
use ga_image::{FileType, ImageView};
use std::path::Path;

#[derive(Debug)]
pub struct FileSequence {
    output_dir: String,
    file_name_prefix: String,
    counter: usize
}

impl FileSequence {
    pub fn new(output_dir: &str, file_name_prefix: &str) -> FileSequence {
        FileSequence{
            output_dir: output_dir.to_string(),
            file_name_prefix: file_name_prefix.to_string(),
            counter: 0
        }
    }
}

impl OutputWriter for FileSequence {
    fn write(&mut self, image: &ImageView) -> Result<(), String> {
        let result = image.save(
            &Path::new(&self.output_dir)
                .join(format!("{}_{:05}.tif", self.file_name_prefix, self.counter))
                .to_str().unwrap().to_string(),
            FileType::Tiff
        );

        match result {
            Err(err) => Err(format!("{:?}", err)),
            Ok(()) => { self.counter += 1; Ok(()) }
        }
    }

    fn finalize(&mut self) -> Result<(), String> {
        Ok(())
    }
}
