#![allow(unused_variables)]

use tauri::{LogicalPosition, LogicalSize};
use tauri_runtime::{
    Error, Icon, ProgressBarState, Result, UserAttentionType, UserEvent, WindowDispatch,
    WindowEventId,
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    monitor::Monitor,
    webview::{DetachedWebview, PendingWebview},
    window::{
        CursorIcon, DetachedWindow, PendingWindow, RawWindow, WindowBuilder, WindowBuilderBase,
        WindowEvent, WindowId,
    },
};
use tauri_utils::{Theme, config::WindowConfig};
use verso::{VersoBuilder, VersoviewController};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use std::{
    collections::HashMap,
    fmt::{self, Debug},
    sync::{Arc, Mutex},
};

use crate::{
    RuntimeContext, VersoRuntime, event_loop_ext::TaoEventLoopWindowTargetExt,
    get_verso_devtools_port, get_verso_resource_directory, runtime::Message,
};

pub(crate) struct Window {
    pub(crate) label: String,
    pub(crate) webview: Arc<Mutex<VersoviewController>>,
    pub(crate) on_window_event_listeners: WindowEventListeners,
}

#[derive(Debug, Clone)]
pub struct VersoWindowBuilder {
    pub verso_builder: VersoBuilder,
    pub has_icon: bool,
}

impl Default for VersoWindowBuilder {
    fn default() -> Self {
        let mut verso_builder = VersoBuilder::new();
        if let Some(resource_directory) = get_verso_resource_directory() {
            verso_builder = verso_builder.resources_directory(resource_directory);
        }
        if let Some(devtools_port) = get_verso_devtools_port() {
            verso_builder = verso_builder.devtools_port(devtools_port);
        }
        // Default `decorated` to `true` to align with the wry runtime
        verso_builder = verso_builder.decorated(true);
        // Default `transparent` to `false` to align with the wry runtime
        verso_builder = verso_builder.transparent(false);
        Self {
            verso_builder,
            has_icon: false,
        }
    }
}

impl WindowBuilderBase for VersoWindowBuilder {}

impl WindowBuilder for VersoWindowBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn with_config(config: &WindowConfig) -> Self {
        let builder = Self::default();
        let mut verso_builder = builder.verso_builder;
        verso_builder = verso_builder
            .focused(config.focus)
            .fullscreen(config.fullscreen)
            .maximized(config.maximized)
            .visible(config.visible)
            .inner_size(LogicalSize::new(config.width, config.height))
            .title(config.title.clone())
            .decorated(config.decorations)
            .transparent(config.transparent);

        if let (Some(x), Some(y)) = (config.x, config.y) {
            verso_builder = verso_builder.position(LogicalPosition::new(x, y));
        };

        Self {
            verso_builder,
            has_icon: false,
        }
    }

    /// Unsupported, has no effect
    fn center(self) -> Self {
        self
    }

    /// Note: x and y are in logical unit
    fn position(mut self, x: f64, y: f64) -> Self {
        self.verso_builder = self.verso_builder.position(LogicalPosition::new(x, y));
        self
    }

    /// Note: width and height are in logical unit
    fn inner_size(mut self, width: f64, height: f64) -> Self {
        self.verso_builder = self
            .verso_builder
            .inner_size(LogicalSize::new(width, height));
        self
    }

    /// Unsupported, has no effect
    fn min_inner_size(self, min_width: f64, min_height: f64) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn max_inner_size(self, max_width: f64, max_height: f64) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn inner_size_constraints(
        self,
        constraints: tauri_runtime::window::WindowSizeConstraints,
    ) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn resizable(self, resizable: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn maximizable(self, resizable: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn minimizable(self, resizable: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn closable(self, resizable: bool) -> Self {
        self
    }

    fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.verso_builder = self.verso_builder.title(title);
        self
    }

    fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.verso_builder = self.verso_builder.fullscreen(fullscreen);
        self
    }

    fn focused(mut self, focused: bool) -> Self {
        self.verso_builder = self.verso_builder.focused(focused);
        self
    }

    fn maximized(mut self, maximized: bool) -> Self {
        self.verso_builder = self.verso_builder.maximized(maximized);
        self
    }

    fn visible(mut self, visible: bool) -> Self {
        self.verso_builder = self.verso_builder.visible(visible);
        self
    }

    fn decorations(mut self, decorations: bool) -> Self {
        self.verso_builder = self.verso_builder.decorated(decorations);
        self
    }

    fn always_on_bottom(mut self, always_on_bottom: bool) -> Self {
        self.verso_builder = self.verso_builder.window_level(if always_on_bottom {
            verso::WindowLevel::AlwaysOnTop
        } else {
            verso::WindowLevel::Normal
        });
        self
    }

    fn always_on_top(mut self, always_on_top: bool) -> Self {
        self.verso_builder = self.verso_builder.window_level(if always_on_top {
            verso::WindowLevel::AlwaysOnTop
        } else {
            verso::WindowLevel::Normal
        });
        self
    }

    /// Unsupported, has no effect
    fn visible_on_all_workspaces(self, visible_on_all_workspaces: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn content_protected(self, protected: bool) -> Self {
        self
    }

    fn icon(mut self, icon: Icon<'_>) -> Result<Self> {
        self.verso_builder = self.verso_builder.icon(verso::Icon {
            rgba: icon.rgba.to_vec(),
            width: icon.width,
            height: icon.height,
        });
        self.has_icon = true;
        Ok(self)
    }

    /// Unsupported, has no effect
    fn skip_taskbar(self, skip: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn window_classname<S: Into<String>>(self, classname: S) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn shadow(self, enable: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn parent(self, parent: *mut std::ffi::c_void) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn transient_for(self, parent: &impl gtk::glib::IsA<gtk::Window>) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(windows)]
    fn drag_and_drop(self, enabled: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn title_bar_style(self, style: tauri_utils::TitleBarStyle) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn hidden_title(self, transparent: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn tabbing_identifier(self, identifier: &str) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn traffic_light_position<P: Into<Position>>(self, position: P) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn theme(self, theme: Option<Theme>) -> Self {
        self
    }

    fn has_icon(&self) -> bool {
        self.has_icon
    }

    /// Unsupported, always returns [`None`]
    fn get_theme(&self) -> Option<Theme> {
        None
    }

    /// Unsupported, has no effect
    fn background_color(self, _color: tauri_utils::config::Color) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(windows)]
    fn owner(self, owner: HWND) -> Self {
        self
    }

    /// Unsupported, has no effect
    #[cfg(windows)]
    fn parent(self, parent: HWND) -> Self {
        self
    }

    #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(not(target_os = "macos"), feature = "macos-private-api")))
    )]
    fn transparent(mut self, transparent: bool) -> Self {
        self.verso_builder = self.verso_builder.transparent(transparent);
        self
    }

    /// Unsupported, has no effect
    fn prevent_overflow(self) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn prevent_overflow_with_margin(self, margin: tauri_runtime::dpi::Size) -> Self {
        self
    }
}

pub type WindowEventHandler = Box<dyn Fn(&WindowEvent) + Send>;
pub type WindowEventListeners = Arc<Mutex<HashMap<WindowEventId, WindowEventHandler>>>;

/// The Tauri [`WindowDispatch`] for [`VersoRuntime`].
#[derive(Clone)]
pub struct VersoWindowDispatcher<T: UserEvent> {
    pub(crate) id: WindowId,
    pub(crate) context: RuntimeContext<T>,
    pub(crate) webview: Arc<Mutex<VersoviewController>>,
    pub(crate) on_window_event_listeners: WindowEventListeners,
}

impl<T: UserEvent> Debug for VersoWindowDispatcher<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersoWebviewDispatcher")
            .field("id", &self.id)
            .field("context", &self.context)
            .field("webview", &"VersoviewController")
            .finish()
    }
}

impl<T: UserEvent> WindowDispatch<T> for VersoWindowDispatcher<T> {
    type Runtime = VersoRuntime<T>;

    type WindowBuilder = VersoWindowBuilder;

    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.run_on_main_thread(f)
    }

    /// Currently only [`WindowEvent::CloseRequested`] will be emitted
    fn on_window_event<F: Fn(&WindowEvent) + Send + 'static>(&self, f: F) -> WindowEventId {
        let id = self.context.next_window_event_id();
        self.on_window_event_listeners
            .lock()
            .unwrap()
            .insert(id, Box::new(f));
        id
    }

    fn scale_factor(&self) -> Result<f64> {
        self.webview
            .lock()
            .unwrap()
            .get_scale_factor()
            .map_err(|_| Error::FailedToSendMessage)
    }

    /// Returns the position of the top-left hand corner of the window's client area relative to the top-left hand corner of the desktop.
    ///
    /// ## Platform-specific
    ///
    /// **Wayland**: always return `PhysicalPosition { x: 0, y: 0 }`
    fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_inner_position()
            .map_err(|_| Error::FailedToSendMessage)?
            .unwrap_or_default())
    }

    /// Returns the position of the top-left hand corner of the window relative to the top-left hand corner of the desktop.
    ///
    /// ## Platform-specific
    ///
    /// **Wayland**: always return `PhysicalPosition { x: 0, y: 0 }`
    fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_outer_position()
            .map_err(|_| Error::FailedToSendMessage)?
            .unwrap_or_default())
    }

    fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        self.webview
            .lock()
            .unwrap()
            .get_inner_size()
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        self.webview
            .lock()
            .unwrap()
            .get_outer_size()
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn is_fullscreen(&self) -> Result<bool> {
        self.webview
            .lock()
            .unwrap()
            .is_fullscreen()
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn is_minimized(&self) -> Result<bool> {
        self.webview
            .lock()
            .unwrap()
            .is_minimized()
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn is_maximized(&self) -> Result<bool> {
        self.webview
            .lock()
            .unwrap()
            .is_maximized()
            .map_err(|_| Error::FailedToSendMessage)
    }

    /// Unsupported, always returns false
    fn is_focused(&self) -> Result<bool> {
        Ok(false)
    }

    /// Unsupported, always returns false
    fn is_decorated(&self) -> Result<bool> {
        Ok(false)
    }

    /// Unsupported, always returns true
    fn is_resizable(&self) -> Result<bool> {
        Ok(true)
    }

    /// Unsupported, always returns true
    fn is_maximizable(&self) -> Result<bool> {
        Ok(true)
    }

    /// Unsupported, always returns true
    fn is_minimizable(&self) -> Result<bool> {
        Ok(true)
    }

    /// Unsupported, always returns true
    fn is_closable(&self) -> Result<bool> {
        Ok(true)
    }

    fn is_visible(&self) -> Result<bool> {
        self.webview
            .lock()
            .unwrap()
            .is_visible()
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn title(&self) -> Result<String> {
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_title()
            .map_err(|_| Error::FailedToSendMessage)?)
    }

    /// Unsupported, always returns [`None`]
    fn current_monitor(&self) -> Result<Option<Monitor>> {
        Ok(None)
    }

    fn primary_monitor(&self) -> Result<Option<Monitor>> {
        self.context
            .run_on_main_thread_with_event_loop(|e| e.tauri_primary_monitor())
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        self.context
            .run_on_main_thread_with_event_loop(move |e| e.tauri_monitor_from_point(x, y))
    }

    fn available_monitors(&self) -> Result<Vec<Monitor>> {
        self.context
            .run_on_main_thread_with_event_loop(|e| e.tauri_available_monitors())
    }

    /// Unsupported, always returns [`Theme::Light`]
    fn theme(&self) -> Result<Theme> {
        Ok(Theme::Light)
    }

    /// Unsupported, panics when called
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn gtk_window(&self) -> Result<gtk::ApplicationWindow> {
        unimplemented!()
    }

    /// Unsupported, panics when called
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn default_vbox(&self) -> Result<gtk::Box> {
        unimplemented!()
    }

    /// Unsupported, has no effect when called
    fn center(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        Ok(())
    }

    /// `after_window_creation` not supported
    ///
    /// Only creating the window with a webview is supported,
    /// will return [`tauri_runtime::Error::CreateWindow`] if there is no [`PendingWindow::webview`]
    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &mut self,
        pending: PendingWindow<T, Self::Runtime>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self::Runtime>> {
        self.context.create_window(pending, after_window_creation)
    }

    /// Unsupported, always fail with [`tauri_runtime::Error::CreateWindow`]
    fn create_webview(
        &mut self,
        pending: PendingWebview<T, Self::Runtime>,
    ) -> Result<DetachedWebview<T, Self::Runtime>> {
        Err(tauri_runtime::Error::CreateWindow)
    }

    /// Unsupported, has no effect when called
    fn set_resizable(&self, resizable: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_closable(&self, closable: bool) -> Result<()> {
        Ok(())
    }

    fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_title(title)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn maximize(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_maximized(true)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn unmaximize(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_maximized(false)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn minimize(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_minimized(true)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn unminimize(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_minimized(false)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn show(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_visible(true)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn hide(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_visible(false)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn close(&self) -> Result<()> {
        self.context.send_message(Message::CloseWindow(self.id))?;
        Ok(())
    }

    fn destroy(&self) -> Result<()> {
        self.context.send_message(Message::DestroyWindow(self.id))?;
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_decorations(&self, decorations: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_shadow(&self, shadow: bool) -> Result<()> {
        Ok(())
    }

    fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_window_level(if always_on_bottom {
                verso::WindowLevel::AlwaysOnBottom
            } else {
                verso::WindowLevel::Normal
            })
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_window_level(if always_on_top {
                verso::WindowLevel::AlwaysOnTop
            } else {
                verso::WindowLevel::Normal
            })
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_visible_on_all_workspaces(&self, visible_on_all_workspaces: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_content_protected(&self, protected: bool) -> Result<()> {
        Ok(())
    }

    fn set_size(&self, size: Size) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_size(size)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_min_size(&self, size: Option<Size>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_max_size(&self, size: Option<Size>) -> Result<()> {
        Ok(())
    }

    fn set_position(&self, position: Position) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_position(position)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .set_fullscreen(fullscreen)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    fn set_focus(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .focus()
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_skip_taskbar(&self, skip: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_cursor_position<Pos: Into<Position>>(&self, position: Pos) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        Ok(())
    }

    fn start_dragging(&self) -> Result<()> {
        self.webview
            .lock()
            .unwrap()
            .start_dragging()
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn start_resize_dragging(&self, direction: tauri_runtime::ResizeDirection) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_progress_bar(&self, progress_state: ProgressBarState) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_badge_count(&self, count: Option<i64>, desktop_filename: Option<String>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_overlay_icon(&self, icon: Option<Icon<'_>>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_title_bar_style(&self, style: tauri_utils::TitleBarStyle) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_size_constraints(
        &self,
        constraints: tauri_runtime::window::WindowSizeConstraints,
    ) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_enabled(&self, enabled: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, always returns true
    fn is_enabled(&self) -> Result<bool> {
        Ok(true)
    }

    /// Unsupported, has no effect when called
    fn set_background_color(&self, color: Option<tauri_utils::config::Color>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, will always return an error
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        Err(raw_window_handle::HandleError::NotSupported)
    }

    /// Unsupported, always returns false
    fn is_always_on_top(&self) -> Result<bool> {
        Ok(false)
    }

    /// Unsupported, has no effect when called
    fn set_traffic_light_position(&self, position: Position) -> Result<()> {
        Ok(())
    }
}
