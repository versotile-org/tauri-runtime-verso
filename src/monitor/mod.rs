// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// This is copied from https://github.com/tauri-apps/tauri/tree/tauri-v2.6.0/crates/tauri-runtime-wry/src/monitor

use tauri_runtime::dpi::PhysicalRect;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub trait MonitorExt {
    /// Get the work area of this monitor
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn work_area(&self) -> PhysicalRect<i32, u32>;
}
