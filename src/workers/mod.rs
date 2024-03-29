//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2022 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Worker threads module.
//!

pub mod capture;
#[cfg(feature = "controller")]
pub mod controller;
pub mod histogram;
pub mod recording;
