#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_runtime_verso::{set_verso_path, set_verso_resource_directory, INVOKE_SYSTEM_SCRIPTS};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    set_verso_path("../verso/target/debug/versoview".into());
    set_verso_resource_directory("../verso/resources".into());
    tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            dbg!(app.get_webview_window("main").unwrap().inner_size()).unwrap();
            Ok(())
        })
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
