# Vidoxide
Copyright (C) 2020-2021 Filip Szczerek (ga.software@yahoo.com)

*This program is licensed under MIT license (see LICENSE.txt for details).*

----------------------------------------

- 1\. Introduction
- 2\. Features
- 3\. Building
  - 3\.1\. Linux and alikes
  - 3\.2\. MS Windows
- 4\. Bugs

----------------------------------------

![Main window](doc/screenshots/main_window.png)

## 1. Introduction

Vidoxide is a video capture tool with features targeted at Solar System astrophotographers.

Demonstration video: *upcoming*


## 2. Features

**Cross-platform**
  - Builds & runs on any platform supporting Rust and GTK

**Supported camera APIs:**
  - IIDC (DC1394); multiplatform
  - FlyCapture2 (FLIR, formerly Point Grey); multiplatform
  - Video4Linux2 – extremely basic support (only YUYV video modes, no camera controls); Linux only

**Supported telescope mounts:**
  - Sky-Watcher direct serial connection (tested with a 2014 HEQ5), multiplatform
  - *upcoming:* ASCOM/EQMod, MS Windows only

**Image feature tracking:**
  - self-guiding (with supported mounts): selected image feature (or a planet – via centroid) stays in the same place of the FOV
  - live crop: only a ROI around a selected image feature is recorded

**Output formats:**
  - TIFF or BMP image sequence
  - SER video
  - *upcoming:* AVI video


## 3. Building

Clone the repository:
```Bash
$ git clone --recurse-submodules https://github.com/GreatAttractor/vidoxide.git
```

Camera drivers to build are selected as features in invocation of `cargo`, e.g.:
```Bash
$ cargo build --release --features "camera_iidc camera_v4l2 camera_flycap2"
```
will build Vidoxide with the IIDC, V4L2 and FlyCapture 2 drivers.


### 3.1. Linux and alikes

Install the [Rust toolchain](https://www.rust-lang.org/learn/get-started). C & C++ toolchain is also required, as are GTK3 development libraries, and those needed by camera drivers you wish to use.

*Detailed instructions: to be provided.*


### 3.2. MS Windows

Building under MS Windows has been tested in [MSYS2](https://www.msys2.org/) environment and the GNU variant of the [Rust toolchain](https://www.rust-lang.org/learn/get-started).

*Detailed instructions: to be provided.*


## 4. Bugs

- Live crop during recording is broken for raw color video modes.
