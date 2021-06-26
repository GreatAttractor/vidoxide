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

Download MSYS2 from http://www.msys2.org/ and follow its installation instructions. Then install the Rust toolchain: go to https://forge.rust-lang.org/infra/other-installation-methods.html and install the `x86_64-pc-windows-gnu` variant. The warnings about "Visual C++ prerequisites" being required and "Install the C++ build tools before proceeding" can be ignored. Note that you must customize "Current installation options" and change the "default host triple" to "x86_64-pc-windows-gnu".

Open the "MSYS2 MinGW 64-bit" shell (from the Start menu, or directly via `C:\msys64\msys2_shell.cmd -mingw64`), and install the build prerequisites:
```bash
$ pacman -S git base-devel mingw-w64-x86_64-toolchain mingw-w64-x86_64-gtk3
```

From now on it is assumed FlyCapture2 camera API is to be used. Download and install the FlyCapture2 SDK, go to the location of FC2 binaries (by default, "C:\Program Files\Point Grey Research\FlyCapture2\bin64") and check if the `FlyCapture2_C.dll` file exists. If not, make a copy of the corresponding versioned file (e.g., `FlyCapture2_C_v100.dll`) in the same location and rename it `FlyCapture2_C.dll` (this is required due to the `libflycapture2-sys` crate's expectations).

Pull Rust binaries into `$PATH`:
```bash
$ export PATH=$PATH:/c/Users/MY_USERNAME/.cargo/bin
```
then change to the Vidoxide source directory and build it:
```bash
$ RUSTFLAGS="-L C:\Progra~1\PointG~1\FlyCapture2\bin64" cargo build --release --features "camera_flycap2 mount_ascom"
```
Initially it will take several minutes, as all dependencies have to be downloaded and built first. Note that the location to FC2 DLLs must be given in `RUSTFLAGS`, and spaces in directory names are not allowed (tilde-shortened directory names can be checked by running `dir /x` in a Windows shell). This shall be changed in the future to using a Cargo configure script.

After a successful build, Vidoxide can be run locally with:
```bash
$ PATH="$PATH:C:\Program Files\Point Grey Research\FlyCapture2\bin64" target/release/vidoxide.exe
```

*Upcoming: creating a binary distribution*


## 4. Bugs

- Live crop during recording is broken for raw color video modes.
