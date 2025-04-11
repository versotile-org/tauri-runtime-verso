fn main() {
    tauri_runtime_verso_build::get_verso_as_external_bin().unwrap();
    tauri_build::build();
}
