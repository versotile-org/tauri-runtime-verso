#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::WebviewWindowBuilder;
use tauri_runtime_verso::{INVOKE_SYSTEM_SCRIPTS, VersoRuntime};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    // Set `tauri::Builder`'s generic to `VersoRuntime`
    tauri::Builder::<VersoRuntime>::new()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS)
        .setup(|app| {
            WebviewWindowBuilder::new(app, "main", Default::default())
                .inner_size(900., 700.)
                .decorations(false)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
