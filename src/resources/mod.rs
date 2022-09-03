//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Program resources.
//!

use ga_image;
use gtk::gdk_pixbuf;
use gtk::prelude::*;

pub enum ToolbarIcon {
    ZoomIn,
    ZoomOut,
    ZoomCustom,
    SelectRoi,
    RoiOff
}

impl ToolbarIcon {
    pub fn contents(&self) -> &[u8] {
        match self {
            ToolbarIcon::ZoomIn =>     include_bytes!("images/toolbar/zoom_in.svg"),
            ToolbarIcon::ZoomOut =>    include_bytes!("images/toolbar/zoom_out.svg"),
            ToolbarIcon::ZoomCustom => include_bytes!("images/toolbar/zoom_custom.svg"),
            ToolbarIcon::SelectRoi =>  include_bytes!("images/toolbar/select_roi.svg"),
            ToolbarIcon::RoiOff =>     include_bytes!("images/toolbar/roi_off.svg"),
        }
    }
}

pub enum SimulatorImage {
    SunHAlphaFullDisk,
    Landscape,
    Star1
}

impl SimulatorImage {
    pub fn contents(&self) -> &[u8] {
        match self {
            SimulatorImage::SunHAlphaFullDisk => include_bytes!("images/simulator/sun_ha_fd.jpg"),
            SimulatorImage::Landscape => include_bytes!("images/simulator/landscape_rgb.jpg"),
            SimulatorImage::Star1 => include_bytes!("images/simulator/star1.jpg")
        }
    }

    pub fn gdk_pixbuf_img_format(&self) -> &str {
        match self {
            SimulatorImage::SunHAlphaFullDisk
            | SimulatorImage::Landscape
            | SimulatorImage::Star1 => "jpeg"
        }
    }
}


pub fn load_svg(image: ToolbarIcon, size: i32) -> Result<gtk::Image, glib::error::Error> {
    let loader = gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
    loader.set_size(size, size);
    loader.write(image.contents())?;
    loader.close()?;

    Ok(gtk::Image::from_pixbuf(loader.pixbuf().as_ref()))
}

pub fn load_sim_image(image: SimulatorImage) -> Result<ga_image::Image, glib::error::Error> {
    let loader = gdk_pixbuf::PixbufLoader::with_type(image.gdk_pixbuf_img_format()).unwrap();
    loader.write(image.contents())?;
    loader.close()?;

    let pix_buf = loader.pixbuf();

    assert!(pix_buf.as_ref().unwrap().colorspace() == gdk_pixbuf::Colorspace::Rgb);

    let src_bytes = pix_buf.as_ref().unwrap().read_pixel_bytes().unwrap();
    let src_stride = pix_buf.as_ref().unwrap().rowstride() as usize;

    let mut image = ga_image::Image::new(
        pix_buf.as_ref().unwrap().width() as u32,
        pix_buf.as_ref().unwrap().height() as u32,
        None,
        ga_image::PixelFormat::RGB8,
        None,
        false
    );

    let dest_line_num_bytes = image.width() as usize * image.pixel_format().bytes_per_pixel();

    for y in 0..pix_buf.unwrap().height() {
        let dest_line = image.line_mut::<u8>(y as u32);
        let start_ofs = y as usize * src_stride;
        dest_line.copy_from_slice(&src_bytes[start_ofs..start_ofs + dest_line_num_bytes]);
    }

    Ok(image)
}
