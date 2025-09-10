//! # Tauri Runtime Verso Build
//!
//! This is a crate to help with getting started with using verso as a webview without building it yourself
//!
//! ## Example
//!
//! To use it, first add it to your build dependency, and in your build script:
//!
//! ```no_run
//! fn main() {
//!     tauri_runtime_verso_build::get_verso_as_external_bin().unwrap();
//!     tauri_build::build();
//! }
//! ```
//!
//! and in your tauri config file (`tauri.conf.json`), add the following
//!
//! ```json
//! {
//!   "bundle": {
//!     "externalBin": [
//!       "versoview/versoview"
//!     ]
//!   }
//! }
//! ```
//!

use std::{io, path::PathBuf};

pub use versoview_build;

/// Downloads and extracts the pre-built versoview executable
/// to `./versoview/versoview(.exe)` relative to the directory containing your `Cargo.toml` file
pub fn get_verso_as_external_bin() -> io::Result<()> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "android" || target_os == "ios" {
        return Err(io::Error::other(
            "Versoview doesn't support mobile platforms yet",
        ));
    }

    let target_triple = std::env::var("TARGET").unwrap();

    let project_directory = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let output_directory = PathBuf::from(project_directory).join("versoview");

    let extension = if cfg!(windows) { ".exe" } else { "" };
    let output_executable = output_directory.join(format!("versoview-{target_triple}{extension}"));
    let output_version = output_directory.join("versoview-version.txt");

    if std::fs::exists(&output_executable)?
        && std::fs::read_to_string(&output_version).unwrap_or_default()
            == versoview_build::VERSO_VERSION
    {
        return Ok(());
    }

    versoview_build::download_and_extract_verso(&output_directory)?;

    let extracted_versoview_path = output_directory.join(format!("versoview{extension}"));
    std::fs::rename(extracted_versoview_path, &output_executable)?;
    std::fs::write(&output_version, versoview_build::VERSO_VERSION)?;

    println!("cargo:rerun-if-changed={}", output_executable.display());
    println!("cargo:rerun-if-changed={}", output_version.display());

    Ok(())
}
