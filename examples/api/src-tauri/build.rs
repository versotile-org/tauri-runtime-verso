use std::{
    fs,
    path::{self, PathBuf},
};

fn main() {
    rename_verso();
    tauri_build::build()
}

fn rename_verso() {
    let target_triple = std::env::var("TARGET").unwrap();
    let base_path = PathBuf::from("../../../../verso/target/debug/");
    let ext = if cfg!(windows) { ".exe" } else { "" };

    let from_path = path::absolute(base_path.join(format!("versoview{ext}"))).unwrap();
    let to_path =
        path::absolute(base_path.join(format!("versoview-{target_triple}{ext}"))).unwrap();

    fs::copy(&from_path, &to_path).unwrap();

    println!("cargo:rerun-if-changed={}", from_path.display());
    println!("cargo:rerun-if-changed={}", to_path.display());
}
