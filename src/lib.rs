//! # Tauri Runtime Verso
//!
//! Use Verso as the backend for Tauri
//!
//! ## Usage
//!
//! ```
//! use tauri::Manager;
//! use tauri_runtime_verso::{set_verso_path, set_verso_resource_directory, INVOKE_SYSTEM_SCRIPTS};
//!
//! fn main() {
//!     // You need to set this to the path of the versoview executable
//!     // before creating any of the webview windows
//!     set_verso_path("../verso/target/debug/versoview");
//!     // Set this to verso/servo's resources directory before creating any of the webview windows
//!     // this is optional but recommanded, this directory will include very important things
//!     // like user agent stylesheet
//!     set_verso_resource_directory("../verso/resources");
//!     tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
//!         // Make sure to do this or some of the commands will not work
//!         .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
//!         .run(tauri::generate_context!())
//!         .unwrap();
//! }
//! ```
//!
//! ## Cargo features
//!
//! - **macos-private-api**: Matching with Tauri's macos-private-api feature, required if you use that

#![allow(unused)]

use tauri::{LogicalPosition, LogicalSize};
use tauri_runtime::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    monitor::Monitor,
    webview::{DetachedWebview, PendingWebview},
    window::{
        CursorIcon, DetachedWindow, DetachedWindowWebview, PendingWindow, RawWindow, WebviewEvent,
        WindowBuilder, WindowBuilderBase, WindowEvent, WindowId,
    },
    DeviceEventFilter, Error, EventLoopProxy, ExitRequestedEventAction, Icon, ProgressBarState,
    Result, RunEvent, Runtime, RuntimeHandle, RuntimeInitArgs, UserAttentionType, UserEvent,
    WebviewDispatch, WebviewEventId, WindowDispatch, WindowEventId,
};
use tauri_utils::{config::WindowConfig, Theme};
use url::Url;
use verso::{VersoBuilder, VersoviewController};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use std::{
    collections::HashMap,
    fmt::{self, Debug},
    fs,
    io::{self},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{channel, sync_channel, Receiver, SyncSender},
        Arc, Mutex, OnceLock,
    },
};

static VERSO_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Sets the Verso executable path to ues for the webviews,
/// much be called before you create any webviews
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub fn set_verso_path(path: impl Into<PathBuf>) {
    VERSO_PATH
        .set(path.into())
        .expect("Verso path is already set, you can't set it multiple times");
}

fn get_verso_path() -> &'static Path {
    VERSO_PATH
        .get()
        .expect("Verso path not set! You need to call set_verso_path before creating any webviews!")
}

static VERSO_RESOURCES_DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Sets the Verso resources directory to ues for the webviews,
/// note this only affects webviews created after you set this
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri_runtime_verso::set_verso_resource_directory("../verso/resources");
///     tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub fn set_verso_resource_directory(path: impl Into<PathBuf>) {
    VERSO_RESOURCES_DIRECTORY
        .lock()
        .unwrap()
        .replace(path.into());
}

fn get_verso_resource_directory() -> Option<PathBuf> {
    VERSO_RESOURCES_DIRECTORY.lock().unwrap().clone()
}

/// You need to set this on [`tauri::Builder::invoke_system`] for the invoke system to work
///
/// ### Example:
///
/// ```
/// fn main() {
///     tauri_runtime_verso::set_verso_path("../verso/target/debug/versoview");
///     tauri_runtime_verso::set_verso_resource_directory("../verso/resources");
///     tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
///         .invoke_system(tauri_runtime_verso::INVOKE_SYSTEM_SCRIPTS.to_owned())
///         .run(tauri::generate_context!())
///         .unwrap();
/// }
/// ```
pub const INVOKE_SYSTEM_SCRIPTS: &str = include_str!("./invoke-system-initialization-script.js");

static DEV_TOOLS_PORT: Mutex<Option<u16>> = Mutex::new(None);

/// Sets the Verso devtools port to ues for the webviews, note this only affects webviews created after you set this
///
/// Since Verso doesn't have devtools built-in,
/// you need to use the one from Firefox,
/// this setting allows you to let verso open a port for it
pub fn set_verso_devtools_port(port: u16) {
    DEV_TOOLS_PORT.lock().unwrap().replace(port);
}

fn get_verso_devtools_port() -> Option<u16> {
    *DEV_TOOLS_PORT.lock().unwrap()
}

enum Message<T> {
    Task(Box<dyn FnOnce() + Send>),
    CloseWindow(WindowId),
    DestroyWindow(WindowId),
    RequestExit(i32),
    UserEvent(T),
}

type WindowEventHandler = Box<dyn Fn(&WindowEvent) + Send>;
type WindowEventListeners = Arc<Mutex<HashMap<WindowEventId, WindowEventHandler>>>;

struct Window {
    label: String,
    webview: Arc<Mutex<VersoviewController>>,
    on_window_event_listeners: WindowEventListeners,
    temp_dir: tempfile::TempDir,
}

#[derive(Clone)]
pub struct RuntimeContext<T> {
    windows: Arc<Mutex<HashMap<WindowId, Window>>>,
    run_tx: SyncSender<Message<T>>,
    next_window_id: Arc<AtomicU32>,
    next_webview_id: Arc<AtomicU32>,
    next_window_event_id: Arc<AtomicU32>,
    next_webview_event_id: Arc<AtomicU32>,
}

impl<T: UserEvent> RuntimeContext<T> {
    fn send_message(&self, message: Message<T>) -> Result<()> {
        self.run_tx
            .send(message)
            .map_err(|_| Error::FailedToSendMessage)
    }

    fn next_window_id(&self) -> WindowId {
        self.next_window_id.fetch_add(1, Ordering::Relaxed).into()
    }

    fn next_webview_id(&self) -> u32 {
        self.next_webview_id.fetch_add(1, Ordering::Relaxed)
    }

    fn next_window_event_id(&self) -> WindowEventId {
        self.next_window_event_id.fetch_add(1, Ordering::Relaxed)
    }

    fn next_webview_event_id(&self) -> WindowEventId {
        self.next_webview_event_id.fetch_add(1, Ordering::Relaxed)
    }

    /// `after_window_creation` not supported
    ///
    /// Only creating the window with a webview is supported,
    /// will return [`tauri_runtime::Error::CreateWindow`] if there is no [`PendingWindow::webview`]
    fn create_window<
        R: Runtime<
            T,
            WindowDispatcher = VersoWindowDispatcher<T>,
            WebviewDispatcher = VersoWebviewDispatcher<T>,
        >,
        F: Fn(RawWindow<'_>) + Send + 'static,
    >(
        &self,
        pending: PendingWindow<T, R>,
        _after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, R>> {
        let label = pending.label;
        let Some(pending_webview) = pending.webview else {
            return Err(tauri_runtime::Error::CreateWindow);
        };

        let window_id = self.next_window_id();
        let webview_id = self.next_webview_id();

        // TODO: Remove this after https://github.com/servo/servo/pull/35388
        let temp_dir = initialization_scripts_to_files(
            pending_webview.webview_attributes.initialization_scripts,
        )
        .map_err(|_| tauri_runtime::Error::CreateWindow)?;

        let webview = pending
            .window_builder
            .verso_builder
            .userscripts_directory(temp_dir.path().to_string_lossy())
            .build(get_verso_path(), Url::parse(&pending_webview.url).unwrap());

        let webview_label = label.clone();
        webview
            .on_web_resource_requested(move |mut request, response_fn| {
                // dbg!(&request);
                // TODO: Servo's EmbedderMsg::WebResourceRequested message is sent too early
                // that it doesn't include Origin header, so I hard coded this for now
                if !request.request.headers().contains_key("Origin") {
                    request
                        .request
                        .headers_mut()
                        .insert("Origin", "http://tauri.localhost/".parse().unwrap());
                }
                for (scheme, handler) in &pending_webview.uri_scheme_protocols {
                    // Since servo doesn't support body in its EmbedderMsg::WebResourceRequested yet,
                    // we use a head instead for now
                    if scheme == "ipc" {
                        if let Some(data) = request
                            .request
                            .headers_mut()
                            .remove("Tauri-VersoRuntime-Invoke-Body")
                        {
                            *request.request.body_mut() = data.as_bytes().to_vec();
                        }
                    }
                    if is_custom_protocol_uri(&request.request.uri().to_string(), "http", scheme) {
                        handler(
                            &webview_label,
                            request.request,
                            Box::new(move |response| {
                                response_fn(Some(response.map(|body| body.to_vec())));
                            }),
                        );
                        return;
                    }
                }
                response_fn(None);
            })
            .map_err(|_| tauri_runtime::Error::CreateWindow)?;
        // for init_script in pending_webview.webview_attributes.initialization_scripts {
        //     webview.add_init_script(init_script);
        // }
        // webview.add_init_script("console.log('1')".to_owned());

        if let Some(navigation_handler) = pending_webview.navigation_handler {
            if let Err(error) = webview.on_navigation_starting(move |url| navigation_handler(&url))
            {
                log::error!("Register `on_navigation_starting` failed with {error}, `navigation_handler` will not get called for this window ({label})!");
            }
        }

        let sender = self.run_tx.clone();
        webview
            .on_close_requested(move || {
                let _ = sender.send(Message::CloseWindow(window_id));
            })
            .map_err(|_| tauri_runtime::Error::CreateWindow)?;

        let on_window_event_listeners = Arc::new(Mutex::new(HashMap::new()));

        let webview = Arc::new(Mutex::new(webview));
        let window = Window {
            label: label.clone(),
            webview: webview.clone(),
            on_window_event_listeners: on_window_event_listeners.clone(),
            temp_dir,
        };

        self.windows.lock().unwrap().insert(window_id, window);

        Ok(DetachedWindow {
            id: window_id,
            label: label.clone(),
            dispatcher: VersoWindowDispatcher {
                id: window_id,
                context: self.clone(),
                webview: webview.clone(),
                on_window_event_listeners,
            },
            webview: Some(DetachedWindowWebview {
                webview: DetachedWebview {
                    label: label.clone(),
                    dispatcher: VersoWebviewDispatcher {
                        id: webview_id,
                        context: self.clone(),
                        webview: webview.clone(),
                    },
                },
                use_https_scheme: false,
            }),
        })
    }
}

// Copied from wry
fn is_custom_protocol_uri(uri: &str, http_or_https: &'static str, protocol: &str) -> bool {
    let uri_len = uri.len();
    let scheme_len = http_or_https.len();
    let protocol_len = protocol.len();

    // starts with `http` or `https``
    &uri[..scheme_len] == http_or_https
    // followed by `://`
    && &uri[scheme_len..scheme_len + 3] == "://"
    // followed by custom protocol name
    && scheme_len + 3 + protocol_len < uri_len && &uri[scheme_len + 3.. scheme_len + 3 + protocol_len] == protocol
    // and a dot
    && scheme_len + 3 + protocol_len < uri_len && uri.as_bytes()[scheme_len + 3 + protocol_len] == b'.'
}

impl<T> Debug for RuntimeContext<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeContext").finish()
    }
}

/// A handle to the [`VersoRuntime`] runtime.
#[derive(Debug, Clone)]
pub struct VersoRuntimeHandle<T> {
    context: RuntimeContext<T>,
}

impl<T: UserEvent> RuntimeHandle<T> for VersoRuntimeHandle<T> {
    type Runtime = VersoRuntime<T>;

    fn create_proxy(&self) -> EventProxy<T> {
        EventProxy {
            run_tx: self.context.run_tx.clone(),
        }
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_activation_policy(
        &self,
        activation_policy: tauri_runtime::ActivationPolicy,
    ) -> Result<()> {
        Ok(())
    }

    fn request_exit(&self, code: i32) -> Result<()> {
        self.context.send_message(Message::RequestExit(code))
    }

    /// `after_window_creation` not supported
    ///
    /// Only creating the window with a webview is supported,
    /// will return [`tauri_runtime::Error::CreateWindow`] if there is no [`PendingWindow::webview`]
    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &self,
        pending: PendingWindow<T, Self::Runtime>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self::Runtime>> {
        self.context.create_window(pending, after_window_creation)
    }

    /// Unsupported, always fail with [`tauri_runtime::Error::CreateWindow`]
    fn create_webview(
        &self,
        window_id: WindowId,
        pending: PendingWebview<T, Self::Runtime>,
    ) -> Result<DetachedWebview<T, Self::Runtime>> {
        Err(tauri_runtime::Error::CreateWindow)
    }

    /// Run a task on the main thread.
    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
    }

    /// Unsupported, always returns [`None`]
    fn primary_monitor(&self) -> Option<Monitor> {
        None
    }

    /// Unsupported, always returns [`None`]
    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        None
    }

    /// Unsupported, always returns an empty vector
    fn available_monitors(&self) -> Vec<Monitor> {
        Vec::new()
    }

    /// Unsupported, has no effect when called
    fn set_theme(&self, theme: Option<Theme>) {}

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn show(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    fn hide(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, will always return PhysicalPosition { x: 0, y: 0 }
    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        Ok(PhysicalPosition::default())
    }

    /// Unsupported, will always return an error
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

/// The Tauri [`WebviewDispatch`] for [`VersoRuntime`].
#[derive(Clone)]
pub struct VersoWebviewDispatcher<T> {
    id: u32,
    context: RuntimeContext<T>,
    webview: Arc<Mutex<VersoviewController>>,
}

impl<T> Debug for VersoWebviewDispatcher<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersoWebviewDispatcher")
            .field("id", &self.id)
            .field("context", &self.context)
            .field("webview", &"VersoviewController")
            .finish()
    }
}

/// The Tauri [`WindowDispatch`] for [`VersoRuntime`].
#[derive(Clone)]
pub struct VersoWindowDispatcher<T> {
    id: WindowId,
    context: RuntimeContext<T>,
    webview: Arc<Mutex<VersoviewController>>,
    on_window_event_listeners: WindowEventListeners,
}

impl<T> Debug for VersoWindowDispatcher<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersoWebviewDispatcher")
            .field("id", &self.id)
            .field("context", &self.context)
            .field("webview", &"VersoviewController")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct VersoWindowBuilder {
    pub verso_builder: VersoBuilder,
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
        Self { verso_builder }
    }
}

impl WindowBuilderBase for VersoWindowBuilder {}

impl WindowBuilder for VersoWindowBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn with_config(config: &WindowConfig) -> Self {
        let mut builder = Self::default();
        let mut verso_builder = builder.verso_builder;
        verso_builder = verso_builder
            .focused(config.focus)
            .fullscreen(config.fullscreen)
            .maximized(config.maximized)
            .visible(config.visible)
            .inner_size(LogicalSize::new(config.width, config.height));

        if let (Some(x), Some(y)) = (config.x, config.y) {
            verso_builder = verso_builder.position(LogicalPosition::new(x, y));
        };

        Self { verso_builder }
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

    /// Unsupported, has no effect
    fn title<S: Into<String>>(self, title: S) -> Self {
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

    /// Unsupported, has no effect
    fn decorations(self, decorations: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn always_on_bottom(self, always_on_bottom: bool) -> Self {
        self
    }

    /// Unsupported, has no effect
    fn always_on_top(self, always_on_top: bool) -> Self {
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

    /// Unsupported, has no effect
    fn icon(self, icon: Icon<'_>) -> Result<Self> {
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
    fn theme(self, theme: Option<Theme>) -> Self {
        self
    }

    /// Unsupported, always returns false
    fn has_icon(&self) -> bool {
        false
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

    /// Unsupported, has no effect
    #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(not(target_os = "macos"), feature = "macos-private-api")))
    )]
    fn transparent(self, transparent: bool) -> Self {
        self
    }
}

impl<T: UserEvent> WebviewDispatch<T> for VersoWebviewDispatcher<T> {
    type Runtime = VersoRuntime<T>;

    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
    }

    /// Unsupported, has no effect when called, the callback will not be called
    fn on_webview_event<F: Fn(&WebviewEvent) + Send + 'static>(&self, f: F) -> WebviewEventId {
        self.context.next_window_event_id()
    }

    /// Unsupported, has no effect when called, the callback will not be called
    fn with_webview<F: FnOnce(Box<dyn std::any::Any>) + Send + 'static>(&self, f: F) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_zoom(&self, scale_factor: f64) -> Result<()> {
        Ok(())
    }

    fn eval_script<S: Into<String>>(&self, script: S) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview
            .lock()
            .unwrap()
            .execute_script(script.into())
            .unwrap();
        Ok(())
    }

    fn url(&self) -> Result<String> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_current_url()
            .unwrap()
            .to_string())
    }

    fn bounds(&self) -> Result<tauri_runtime::Rect> {
        Ok(tauri_runtime::Rect {
            position: self.position()?.into(),
            size: self.size()?.into(),
        })
    }

    fn position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(PhysicalPosition { x: 0, y: 0 })
    }

    fn size(&self) -> Result<PhysicalSize<u32>> {
        // TODO: Find a good enum value to map and propagate the error
        let size = self.webview.lock().unwrap().get_size().unwrap();
        Ok(size)
    }

    fn navigate(&self, url: Url) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().navigate(url).unwrap();
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn print(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn close(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn set_bounds(&self, bounds: tauri_runtime::Rect) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn set_size(&self, _size: Size) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn set_position(&self, _position: Position) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn set_focus(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn reparent(&self, window_id: WindowId) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_auto_resize(&self, auto_resize: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn clear_all_browsing_data(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn hide(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called,
    /// the versoview controls both the webview and the window
    /// use the method from the parent window instead
    fn show(&self) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_background_color(&self, color: Option<tauri_utils::config::Color>) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn open_devtools(&self) {}

    /// Unsupported, has no effect when called
    fn close_devtools(&self) {}

    /// Always false since we don't have devtools built-in
    fn is_devtools_open(&self) -> Result<bool> {
        Ok(false)
    }
}

impl<T: UserEvent> WindowDispatch<T> for VersoWindowDispatcher<T> {
    type Runtime = VersoRuntime<T>;

    type WindowBuilder = VersoWindowBuilder;

    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
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
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().get_scale_factor().unwrap())
    }

    /// Always return PhysicalPosition { x: 0, y: 0 } on Wayland
    fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_position()
            .unwrap()
            .unwrap_or_default())
    }

    /// Always return PhysicalPosition { x: 0, y: 0 } on Wayland
    fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self
            .webview
            .lock()
            .unwrap()
            .get_position()
            .unwrap()
            .unwrap_or_default())
    }

    fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().get_size().unwrap())
    }

    fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().get_size().unwrap())
    }

    fn is_fullscreen(&self) -> Result<bool> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().is_fullscreen().unwrap())
    }

    fn is_minimized(&self) -> Result<bool> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().is_minimized().unwrap())
    }

    fn is_maximized(&self) -> Result<bool> {
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().is_maximized().unwrap())
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
        // TODO: Find a good enum value to map and propagate the error
        Ok(self.webview.lock().unwrap().is_visible().unwrap())
    }

    /// Unsupported, always returns empty string
    fn title(&self) -> Result<String> {
        Ok(String::new())
    }

    /// Unsupported, always returns [`None`]
    fn current_monitor(&self) -> Result<Option<Monitor>> {
        Ok(None)
    }

    /// Unsupported, always returns [`None`]
    fn primary_monitor(&self) -> Result<Option<Monitor>> {
        Ok(None)
    }

    /// Unsupported, always returns [`None`]
    fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        Ok(None)
    }

    /// Unsupported, always returns an empty vector
    fn available_monitors(&self) -> Result<Vec<Monitor>> {
        Ok(Vec::new())
    }

    /// Unsupported, always returns [Theme::Light]
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

    /// Unsupported, has no effect when called
    fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        Ok(())
    }

    fn maximize(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_maximized(true).unwrap();
        Ok(())
    }

    fn unmaximize(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_maximized(false).unwrap();
        Ok(())
    }

    fn minimize(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_minimized(true).unwrap();
        Ok(())
    }

    fn unminimize(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_minimized(false).unwrap();
        Ok(())
    }

    fn show(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_visible(true).unwrap();
        Ok(())
    }

    fn hide(&self) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_visible(false).unwrap();
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

    /// Unsupported, has no effect when called
    fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
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
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_size(size).unwrap();
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
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().set_position(position).unwrap();
        Ok(())
    }

    fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        // TODO: Find a good enum value to map and propagate the error
        self.webview
            .lock()
            .unwrap()
            .set_fullscreen(fullscreen)
            .unwrap();
        Ok(())
    }

    /// Unsupported, has no effect when called
    fn set_focus(&self) -> Result<()> {
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
        // TODO: Find a good enum value to map and propagate the error
        self.webview.lock().unwrap().start_dragging().unwrap();
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
}

#[derive(Debug, Clone)]
pub struct EventProxy<T> {
    run_tx: SyncSender<Message<T>>,
}

impl<T: UserEvent> EventLoopProxy<T> for EventProxy<T> {
    fn send_event(&self, event: T) -> Result<()> {
        self.run_tx
            .send(Message::UserEvent(event))
            .map_err(|_| Error::FailedToSendMessage)
    }
}

/// A Tauri Runtime wrapper around Verso.
#[derive(Debug)]
pub struct VersoRuntime<T: UserEvent = tauri::EventLoopMessage> {
    pub context: RuntimeContext<T>,
    run_rx: Receiver<Message<T>>,
}

impl<T: UserEvent> VersoRuntime<T> {
    fn init() -> Self {
        let (tx, rx) = sync_channel(256);
        let context = RuntimeContext {
            windows: Default::default(),
            run_tx: tx,
            next_window_id: Default::default(),
            next_webview_id: Default::default(),
            next_window_event_id: Default::default(),
            next_webview_event_id: Default::default(),
        };
        Self {
            context,
            run_rx: rx,
        }
    }

    /// Handles the close window request by sending the [`WindowEvent::CloseRequested`] event
    /// if the request doesn't request a forced close
    /// and if not prevented, send [`WindowEvent::Destroyed`]
    /// then checks if there're windows left, if not, send [`RunEvent::ExitRequested`]
    /// returns if we should exit the event loop
    fn handle_close_window_request<F: FnMut(RunEvent<T>) + 'static>(
        &self,
        callback: &mut F,
        id: WindowId,
        force: bool,
    ) -> bool {
        let mut windows = self.context.windows.lock().unwrap();
        let Some(window) = windows.get(&id) else {
            return false;
        };
        let label = window.label.clone();
        let on_window_event_listeners = window.on_window_event_listeners.clone();

        if !force {
            let (tx, rx) = channel();
            let window_event = WindowEvent::CloseRequested {
                signal_tx: tx.clone(),
            };
            for handler in on_window_event_listeners.lock().unwrap().values() {
                handler(&window_event);
            }
            callback(RunEvent::WindowEvent {
                label: label.clone(),
                event: WindowEvent::CloseRequested { signal_tx: tx },
            });

            let should_prevent = matches!(rx.try_recv(), Ok(true));
            if should_prevent {
                return false;
            }
        }

        windows.remove(&id);
        callback(RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
        });

        // This is required becuase tauri puts in a clone of the window in to WindowEventHandler closure,
        // and we need to clear it for the window to drop or else it will stay there forever
        on_window_event_listeners.lock().unwrap().clear();

        let is_empty = windows.is_empty();
        if !is_empty {
            return false;
        }

        let (tx, rx) = channel();
        callback(RunEvent::ExitRequested { code: None, tx });

        let recv = rx.try_recv();
        let should_prevent = matches!(recv, Ok(ExitRequestedEventAction::Prevent));

        !should_prevent
    }
}

impl<T: UserEvent> Runtime<T> for VersoRuntime<T> {
    type WindowDispatcher = VersoWindowDispatcher<T>;
    type WebviewDispatcher = VersoWebviewDispatcher<T>;
    type Handle = VersoRuntimeHandle<T>;
    type EventLoopProxy = EventProxy<T>;

    /// `args` not supported
    fn new(_args: RuntimeInitArgs) -> Result<Self> {
        Ok(Self::init())
    }

    /// `args` not supported
    #[cfg(any(windows, target_os = "linux"))]
    fn new_any_thread(_args: RuntimeInitArgs) -> Result<Self> {
        Ok(Self::init())
    }

    fn create_proxy(&self) -> EventProxy<T> {
        EventProxy {
            run_tx: self.context.run_tx.clone(),
        }
    }

    fn handle(&self) -> Self::Handle {
        VersoRuntimeHandle {
            context: self.context.clone(),
        }
    }

    /// `after_window_creation` not supported
    ///
    /// Only creating the window with a webview is supported,
    /// will return [`tauri_runtime::Error::CreateWindow`] if there is no [`PendingWindow::webview`]
    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &self,
        pending: PendingWindow<T, Self>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self>> {
        self.context.create_window(pending, after_window_creation)
    }

    /// Unsupported, always fail with [`tauri_runtime::Error::CreateWindow`]
    fn create_webview(
        &self,
        window_id: WindowId,
        pending: PendingWebview<T, Self>,
    ) -> Result<DetachedWebview<T, Self>> {
        Err(tauri_runtime::Error::CreateWindow)
    }

    /// Unsupported, always returns [`None`]
    fn primary_monitor(&self) -> Option<Monitor> {
        None
    }

    /// Unsupported, always returns [`None`]
    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        None
    }

    /// Unsupported, always returns an empty vector
    fn available_monitors(&self) -> Vec<Monitor> {
        Vec::new()
    }

    /// Unsupported, has no effect when called
    fn set_theme(&self, theme: Option<Theme>) {}

    /// Unsupported, has no effect when called
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_activation_policy(&mut self, activation_policy: tauri_runtime::ActivationPolicy) {}

    /// Unsupported, has no effect when called
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn show(&self) {}

    /// Unsupported, has no effect when called
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn hide(&self) {}

    /// Unsupported, has no effect when called
    fn set_device_event_filter(&mut self, filter: DeviceEventFilter) {}

    /// Unsupported, has no effect when called
    #[cfg(any(
        target_os = "macos",
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn run_iteration<F: FnMut(RunEvent<T>)>(&mut self, callback: F) {}

    fn run<F: FnMut(RunEvent<T>) + 'static>(self, mut callback: F) {
        callback(RunEvent::Ready);

        while let Ok(m) = self.run_rx.recv() {
            match m {
                Message::Task(p) => p(),
                Message::CloseWindow(id) => {
                    let should_exit = self.handle_close_window_request(&mut callback, id, false);
                    if should_exit {
                        break;
                    }
                }
                Message::DestroyWindow(id) => {
                    let should_exit = self.handle_close_window_request(&mut callback, id, true);
                    if should_exit {
                        break;
                    }
                }
                Message::RequestExit(code) => {
                    let (tx, rx) = channel();
                    callback(RunEvent::ExitRequested {
                        code: Some(code),
                        tx,
                    });

                    let recv = rx.try_recv();
                    let should_prevent = matches!(recv, Ok(ExitRequestedEventAction::Prevent));

                    if !should_prevent {
                        break;
                    }
                }
                Message::UserEvent(user_event) => callback(RunEvent::UserEvent(user_event)),
            }
        }

        callback(RunEvent::Exit);
    }

    /// Unsupported, will always return PhysicalPosition { x: 0, y: 0 }
    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        Ok(PhysicalPosition::default())
    }
}

fn initialization_scripts_to_files(
    initialization_scripts: Vec<String>,
) -> io::Result<tempfile::TempDir> {
    let temp_dir = tempfile::env::temp_dir().join("versoview-servo-userscripts");
    // if let Ok(entries) = fs::read_dir(&temp_dir) {
    //     for entry in entries {
    //         let _ = fs::remove_dir_all(entry?.path());
    //     }
    // }
    fs::create_dir_all(&temp_dir)?;
    let temp_dir_for_this_instance = tempfile::tempdir_in(&temp_dir)?;
    let mut x = 1;
    for init_script in initialization_scripts {
        fs::write(
            temp_dir_for_this_instance.path().join(format!("{x}.js")),
            init_script,
        )?;
        x += 1;
    }
    Ok(temp_dir_for_this_instance)
}

/// Clean up temporary initialization scripts folder
pub fn cleanup_initialization_scripts_folder() -> io::Result<()> {
    let temp_dir = tempfile::env::temp_dir().join("versoview-servo-userscripts");
    fs::remove_dir(temp_dir)
}
