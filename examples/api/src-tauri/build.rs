use std::{fs, path::PathBuf};

fn main() {
    rename_verso();
    tauri_build::build()
}

fn rename_verso() {
    let target_triple = std::env::var("TARGET").unwrap();
    let base_path = PathBuf::from("../../../../verso/target/debug/");
    let ext = if cfg!(windows) { ".exe" } else { "" };
    fs::copy(
        base_path.join(format!("versoview{ext}")),
        base_path.join(format!("versoview-{target_triple}{ext}")),
    )
    .unwrap();
}
