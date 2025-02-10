#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_runtime_verso::set_verso_path;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    set_verso_path("../verso/target/debug/versoview.exe".into());
    tauri::Builder::<tauri_runtime_verso::MockRuntime>::new()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
