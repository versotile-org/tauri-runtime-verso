#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use tauri::WebviewWindowBuilder;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    tauri_runtime_verso::builder()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            WebviewWindowBuilder::new(app, "main", Default::default())
                .inner_size(900., 700.)
                .decorations(false)
                .build()?;
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
