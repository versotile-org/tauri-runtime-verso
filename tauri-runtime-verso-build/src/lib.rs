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

use std::{io, path::PathBuf};

pub use versoview_build;

/// Downloads and extracts the pre-built versoview executable
/// to `./versoview/versoview(.exe)` relative to the directory containing your `Cargo.toml` file
pub fn get_verso_as_external_bin() -> io::Result<()> {
    let target_triple = std::env::var("TARGET").unwrap();
    let project_directory = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let output_directory = PathBuf::from(project_directory).join("versoview");

    versoview_build::download_and_extract_verso(&output_directory)?;

    let ext = if cfg!(windows) { ".exe" } else { "" };
    std::fs::rename(
        output_directory.join(format!("versoview{ext}")),
        output_directory.join(format!("versoview-{target_triple}{ext}")),
    )?;

    Ok(())
}
