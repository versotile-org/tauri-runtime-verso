#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_runtime_verso::{INVOKE_SYSTEM_SCRIPTS, VersoRuntime};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {name}, You have been greeted from Rust!")
}

fn main() {
    // You can also set the `versoview` executable path yourself
    // tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");

    // To use the devtools, set it like to let verso open a devtools server,
    // and you can connect to it through Firefox's devtools in the `about:debugging` page
    tauri_runtime_verso::set_verso_devtools_port(1234);

    // Set `tauri::Builder`'s generic to `VersoRuntime`
    tauri::Builder::<VersoRuntime>::new()
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            dbg!(app.get_webview_window("main").unwrap().inner_size()).unwrap();
            Ok(())
        })
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS)
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
