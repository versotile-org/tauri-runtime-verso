#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use tauri::{
    App, Manager, WebviewWindowBuilder, path::BaseDirectory, utils::platform::current_exe,
};
use tauri_runtime_verso::{
    INVOKE_SYSTEM_SCRIPTS, VersoRuntime, set_verso_path, set_verso_resource_directory,
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    tauri::Builder::<VersoRuntime>::new()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .setup(|app| {
            // Note: with this approach, you can't create windows from the config file,
            // since that runs before this setup hook
            setup_verso_paths(&app)?;

            WebviewWindowBuilder::new(app, "main", Default::default())
                .inner_size(900., 700.)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}

fn setup_verso_paths(app: &App<VersoRuntime>) -> Result<(), Box<dyn std::error::Error>> {
    let verso_resources_path = app
        .path()
        .resolve("verso-resources", BaseDirectory::Resource)?;
    set_verso_resource_directory(verso_resources_path);
    let verso_path = side_car_path("versoview").ok_or("Can't get verso path")?;
    set_verso_path(verso_path);
    Ok(())
}

fn side_car_path(name: &str) -> Option<PathBuf> {
    Some(current_exe().ok()?.parent()?.join(name))
}
