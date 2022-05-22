//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2020-2021 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Build script.
//!

fn main() {
    let output_dir = std::env::var("OUT_DIR").unwrap();
    let version_path = std::path::Path::new(&output_dir).join("version");

    let version_str = format!("{}", get_commit_hash());

    std::fs::write(version_path, version_str).unwrap();
}

fn get_commit_hash() -> String {
    let output = std::process::Command::new("git")
        .arg("log").arg("-1")
        .arg("--pretty=format:%h")
        .arg("--abbrev=8")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        "unspecified".to_string()
    }
}
