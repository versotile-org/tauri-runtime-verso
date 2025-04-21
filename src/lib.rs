//! # Tauri Runtime Verso
//!
//! Use Verso as the backend for Tauri
//!
//! ## Usage
//!
//! To get started, you need to add this crate to your project, and use `default-feature = false` on `tauri` to disable the `wry` feature
//!
//! ```diff
//!   [build-dependencies]
//!   tauri-build = "2"
//! + tauri-runtime-verso-build = { git = "https://github.com/versotile-org/tauri-runtime-verso.git" }
//!
//!   [dependencies]
//! - tauri = { version = "2", features = [] }
//! + tauri = { version = "2", default-features = false, features = ["common-controls-v6"] }
//! + tauri-runtime-verso = { git = "https://github.com/versotile-org/tauri-runtime-verso.git" }
//! ```
//!
//! In your build script, add the `tauri-runtime-verso-build` script, which will download the pre-built `versoview` to `versoview/versoview-{target-triple}`
//!
//! > Note we currently only have pre-built `versoview` for x64 Linux, Windows, MacOS and arm64 MacOS, also the download might take a bit of time if you have a slow internet connection
//!
//! ```diff
//! fn main() {
//! +   tauri_runtime_verso_build::get_verso_as_external_bin().unwrap();
//!     tauri_build::build();
//! }
//! ```
//!
//! Then add the downloaded executable to your tauri config file (`tauri.conf.json`) as an external binary file
//!
//! ```diff
//!   {
//! +   "bundle": {
//! +     "externalBin": [
//! +       "versoview/versoview"
//! +     ]
//! +   }
//!   }
//! ```
//!
//! Finally, setup the code like this:
//!
//! ```diff
//! fn main() {
//! -   tauri::Builder::new()
//! +   tauri_runtime_verso::builder()
//!         .run(tauri::generate_context!())
//!         .unwrap();
//! }
//! ```
//!
//! ## Tips
//!
//! ### Devtools
//!
//! Since Verso doesn't have a devtools built-in, you'll need to use the one from the Firefox, first put in this in your code
//!
//! ```rust
//! // This will make the webviews created afters this open up a devtools server on this port,
//! // setting it to 0 for a random port
//! tauri_runtime_verso::set_verso_devtools_port(1234);
//! ```
//!
//! Then go to `about:debugging` in Firefox and connect to `localhost:1234` there
//!
//! ## Cargo features
//!
//! - **macos-private-api**: Matching with Tauri's macos-private-api feature, required if you use that

mod event_loop_ext;
mod runtime;
mod window;

pub use runtime::{
    EventProxy, RuntimeContext, VersoRuntime, VersoRuntimeHandle, VersoWebviewDispatcher,
};
pub use window::{VersoWindowBuilder, VersoWindowDispatcher};

use std::{
    env::current_exe,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static VERSO_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Sets the Verso executable path to ues for the webviews,
/// must be called before you create any webviews if you don't have the `externalBin` setup
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri_runtime_verso::builder()
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub fn set_verso_path(path: impl Into<PathBuf>) {
    VERSO_PATH
        .set(path.into())
        .expect("Verso path is already set, you can't set it multiple times");
}

fn get_verso_path() -> &'static Path {
    VERSO_PATH.get_or_init(|| {
        relative_command_path("versoview").expect(
            "Verso path not set! You need to call set_verso_path before creating any webviews!",
        )
    })
}

fn relative_command_path(name: &str) -> Option<PathBuf> {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    current_exe()
        .ok()?
        .parent()?
        .join(format!("{name}{extension}"))
        .canonicalize()
        .ok()
}

static VERSO_RESOURCES_DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Sets the Verso resources directory to ues for the webviews,
/// note this only affects webviews created after you set this
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri_runtime_verso::set_verso_resource_directory("../verso/resources");
///     tauri_runtime_verso::builder()
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub fn set_verso_resource_directory(path: impl Into<PathBuf>) {
    VERSO_RESOURCES_DIRECTORY
        .lock()
        .unwrap()
        .replace(path.into());
}

fn get_verso_resource_directory() -> Option<PathBuf> {
    VERSO_RESOURCES_DIRECTORY.lock().unwrap().clone()
}

/// You need to set this on [`tauri::Builder::invoke_system`] for the invoke system to work,
/// you can skip this if you're using [`tauri_runtime_verso::builder`](builder)
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri_runtime_verso::set_verso_resource_directory("../verso/resources");
///     tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
///         .invoke_system(tauri_runtime_verso::INVOKE_SYSTEM_SCRIPTS.to_owned())
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub const INVOKE_SYSTEM_SCRIPTS: &str = include_str!("./invoke-system-initialization-script.js");

static DEV_TOOLS_PORT: Mutex<Option<u16>> = Mutex::new(None);

/// Sets the Verso devtools port to ues for the webviews, 0 for random port,
/// note this only affects webviews created after you set this
///
/// Since Verso doesn't have devtools built-in,
/// you need to use the one from Firefox from the `about:debugging` page,
/// this setting allows you to let verso open a port for it
pub fn set_verso_devtools_port(port: u16) {
    DEV_TOOLS_PORT.lock().unwrap().replace(port);
}

fn get_verso_devtools_port() -> Option<u16> {
    *DEV_TOOLS_PORT.lock().unwrap()
}

/// Creates a new [`tauri::Builder`] using the [`VersoRuntime`]
///
/// ### Example:
///
/// ```no_run
/// fn main() {
///     // instead of `tauri::Builder::new()`
///     tauri_runtime_verso::builder()
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub fn builder() -> tauri::Builder<VersoRuntime> {
    tauri::Builder::new().invoke_system(INVOKE_SYSTEM_SCRIPTS)
}
