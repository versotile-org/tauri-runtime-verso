#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_runtime_verso::{INVOKE_SYSTEM_SCRIPTS, set_verso_path, set_verso_resource_directory};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    // You need to set this to the path of the versoview executable
    // before creating any of the webview windows
    set_verso_path("../verso/target/debug/versoview");
    // Set this to verso/servo's resources directory before creating any of the webview windows
    // this is optional but recommended, this directory will include very important things
    // like user agent stylesheet
    set_verso_resource_directory("../verso/resources");
    tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            dbg!(app.get_webview_window("main").unwrap().inner_size()).unwrap();
            Ok(())
        })
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
