pub fn to_verso_theme(theme: tauri_utils::Theme) -> verso::Theme {
    match theme {
        tauri_utils::Theme::Dark => verso::Theme::Dark,
        _ => verso::Theme::Light,
    }
}

pub fn from_verso_theme(theme: verso::Theme) -> tauri_utils::Theme {
    match theme {
        verso::Theme::Dark => tauri_utils::Theme::Dark,
        verso::Theme::Light => tauri_utils::Theme::Light,
    }
}

pub fn to_tao_theme(theme: tauri_utils::Theme) -> tao::window::Theme {
    match theme {
        tauri_utils::Theme::Dark => tao::window::Theme::Dark,
        _ => tao::window::Theme::Light,
    }
}
