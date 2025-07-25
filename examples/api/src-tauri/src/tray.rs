use tauri::{
    AppHandle, Runtime,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    const CLICK_ME_ID: &str = "click-me";
    let menu = MenuBuilder::new(app)
        .item(
            &MenuItemBuilder::new("Click me!")
                .id(CLICK_ME_ID)
                .build(app)?,
        )
        .quit()
        .build()?;
    TrayIconBuilder::new()
        .tooltip("Tauri")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|_tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                dbg!("Tray icon clicked!");
            }
            _ => {}
        })
        .on_menu_event(|_app, event| {
            if event.id == CLICK_ME_ID {
                dbg!("Click me clicked!");
            }
        })
        .build(app)?;
    Ok(())
}
