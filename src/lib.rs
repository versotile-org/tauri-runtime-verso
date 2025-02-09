#![allow(dead_code)]
#![allow(unused)]
#![allow(missing_docs)]

use tauri_runtime::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    monitor::Monitor,
    webview::{DetachedWebview, PendingWebview},
    window::{
        CursorIcon, DetachedWindow, DetachedWindowWebview, PendingWindow, RawWindow, WindowBuilder,
        WindowBuilderBase, WindowEvent, WindowId,
    },
    DeviceEventFilter, Error, EventLoopProxy, ExitRequestedEventAction, Icon, ProgressBarState,
    Result, RunEvent, Runtime, RuntimeHandle, RuntimeInitArgs, UserAttentionType, UserEvent,
    WebviewDispatch, WindowDispatch, WindowEventId,
};
use tauri_utils::{config::WindowConfig, Theme};
use url::Url;
use verso::VersoviewController;
use windows::Win32::Foundation::HWND;

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Debug},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{channel, sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
};

type ShortcutMap = HashMap<String, Box<dyn Fn() + Send + 'static>>;

enum Message {
    Task(Box<dyn FnOnce() + Send>),
    CloseWindow(WindowId),
    DestroyWindow(WindowId),
}

struct Window {
    label: String,
    webview: Arc<Mutex<VersoviewController>>,
}

#[derive(Clone)]
pub struct RuntimeContext {
    is_running: Arc<AtomicBool>,
    windows: Arc<Mutex<HashMap<WindowId, Window>>>,
    run_tx: SyncSender<Message>,
    next_window_id: Arc<AtomicU32>,
    next_webview_id: Arc<AtomicU32>,
    next_window_event_id: Arc<AtomicU32>,
    next_webview_event_id: Arc<AtomicU32>,
}

impl RuntimeContext {
    fn send_message(&self, message: Message) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            self.run_tx
                .send(message)
                .map_err(|_| Error::FailedToSendMessage)
        } else {
            match message {
                Message::Task(task) => task(),
                Message::CloseWindow(id) | Message::DestroyWindow(id) => {
                    self.windows.lock().unwrap().remove(&id);
                }
            }
            Ok(())
        }
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

    fn create_window<
        T: UserEvent,
        R: Runtime<
            T,
            WindowDispatcher = MockWindowDispatcher,
            WebviewDispatcher = MockWebviewDispatcher,
        >,
        F: Fn(RawWindow<'_>) + Send + 'static,
    >(
        &self,
        pending: PendingWindow<T, R>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, R>> {
        let label = pending.label;
        let pending_webview = pending.webview.unwrap();

        let window_id = self.next_window_id();
        let webview_id = self.next_webview_id();

        let webview = VersoviewController::new(
            "../verso/target/debug/versoview.exe",
            Url::parse(&pending_webview.url).unwrap(),
        );
        let webview_label = label.clone();
        webview.on_web_resource_requested(move |mut request, response_fn| {
            dbg!(&request);
            // TODO: Servo's EmbedderMsg::WebResourceRequested message is sent too early
            // that it doesn't include Origin header, so I hard coded this for now
            if !request.request.headers().contains_key("Origin") {
                request
                    .request
                    .headers_mut()
                    .insert("Origin", "http://tauri.localhost/".parse().unwrap());
            }
            for (scheme, handler) in &pending_webview.uri_scheme_protocols {
                if is_custom_protocol_uri(&request.request.uri().to_string(), "http", &scheme) {
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
        });

        let webview = Arc::new(Mutex::new(webview));
        let window = Window {
            label: label.clone(),
            webview: webview.clone(),
        };

        self.windows.lock().unwrap().insert(window_id, window);

        Ok(DetachedWindow {
            id: window_id,
            label: label.clone(),
            dispatcher: MockWindowDispatcher {
                id: window_id,
                context: self.clone(),
            },
            webview: Some(DetachedWindowWebview {
                webview: DetachedWebview {
                    label: label.clone(),
                    dispatcher: MockWebviewDispatcher {
                        id: webview_id,
                        context: self.clone(),
                        webview: webview.clone(),
                        url: Arc::new(Mutex::new(pending_webview.url)),
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

impl fmt::Debug for RuntimeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeContext").finish()
    }
}

#[derive(Debug, Clone)]
pub struct MockRuntimeHandle {
    context: RuntimeContext,
}

impl<T: UserEvent> RuntimeHandle<T> for MockRuntimeHandle {
    type Runtime = MockRuntime;

    fn create_proxy(&self) -> EventProxy {
        EventProxy {}
    }

    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_activation_policy(
        &self,
        activation_policy: tauri_runtime::ActivationPolicy,
    ) -> Result<()> {
        Ok(())
    }

    fn request_exit(&self, code: i32) -> Result<()> {
        unimplemented!()
    }

    /// Create a new webview window.
    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &self,
        pending: PendingWindow<T, Self::Runtime>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self::Runtime>> {
        self.context.create_window(pending, after_window_creation)
    }

    fn create_webview(
        &self,
        window_id: WindowId,
        pending: PendingWebview<T, Self::Runtime>,
    ) -> Result<DetachedWebview<T, Self::Runtime>> {
        todo!()
    }

    /// Run a task on the main thread.
    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
    }

    fn primary_monitor(&self) -> Option<Monitor> {
        unimplemented!()
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        unimplemented!()
    }

    fn available_monitors(&self) -> Vec<Monitor> {
        unimplemented!()
    }

    fn set_theme(&self, theme: Option<Theme>) {
        unimplemented!()
    }

    /// Shows the application, but does not automatically focus it.
    #[cfg(target_os = "macos")]
    fn show(&self) -> Result<()> {
        Ok(())
    }

    /// Hides the application.
    #[cfg(target_os = "macos")]
    fn hide(&self) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "android")]
    fn find_class<'a>(
        &self,
        env: &mut jni::JNIEnv<'a>,
        activity: &jni::objects::JObject<'_>,
        name: impl Into<String>,
    ) -> std::result::Result<jni::objects::JClass<'a>, jni::errors::Error> {
        todo!()
    }

    #[cfg(target_os = "android")]
    fn run_on_android_context<F>(&self, f: F)
    where
        F: FnOnce(&mut jni::JNIEnv, &jni::objects::JObject, &jni::objects::JObject)
            + Send
            + 'static,
    {
        todo!()
    }

    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        todo!()
    }

    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle, raw_window_handle::HandleError> {
        todo!()
    }
}

#[derive(Clone)]
pub struct MockWebviewDispatcher {
    id: u32,
    context: RuntimeContext,
    webview: Arc<Mutex<VersoviewController>>,
    url: Arc<Mutex<String>>,
}

impl Debug for MockWebviewDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockWebviewDispatcher")
            .field("id", &self.id)
            .field("context", &self.context)
            .field("webview", &"VersoviewController")
            .field("url", &self.url)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct MockWindowDispatcher {
    id: WindowId,
    context: RuntimeContext,
}

#[derive(Debug, Clone)]
pub struct MockWindowBuilder {}

impl WindowBuilderBase for MockWindowBuilder {}

impl WindowBuilder for MockWindowBuilder {
    fn new() -> Self {
        Self {}
    }

    fn with_config(config: &WindowConfig) -> Self {
        Self {}
    }

    fn center(self) -> Self {
        self
    }

    fn position(self, x: f64, y: f64) -> Self {
        self
    }

    fn inner_size(self, min_width: f64, min_height: f64) -> Self {
        self
    }

    fn min_inner_size(self, min_width: f64, min_height: f64) -> Self {
        self
    }

    fn max_inner_size(self, max_width: f64, max_height: f64) -> Self {
        self
    }

    fn inner_size_constraints(
        self,
        constraints: tauri_runtime::window::WindowSizeConstraints,
    ) -> Self {
        self
    }

    fn resizable(self, resizable: bool) -> Self {
        self
    }

    fn maximizable(self, resizable: bool) -> Self {
        self
    }

    fn minimizable(self, resizable: bool) -> Self {
        self
    }

    fn closable(self, resizable: bool) -> Self {
        self
    }

    fn title<S: Into<String>>(self, title: S) -> Self {
        self
    }

    fn fullscreen(self, fullscreen: bool) -> Self {
        self
    }

    fn focused(self, focused: bool) -> Self {
        self
    }

    fn maximized(self, maximized: bool) -> Self {
        self
    }

    fn visible(self, visible: bool) -> Self {
        self
    }

    fn decorations(self, decorations: bool) -> Self {
        self
    }

    fn always_on_bottom(self, always_on_bottom: bool) -> Self {
        self
    }

    fn always_on_top(self, always_on_top: bool) -> Self {
        self
    }

    fn visible_on_all_workspaces(self, visible_on_all_workspaces: bool) -> Self {
        self
    }

    fn content_protected(self, protected: bool) -> Self {
        self
    }

    fn icon(self, icon: Icon<'_>) -> Result<Self> {
        Ok(self)
    }

    fn skip_taskbar(self, skip: bool) -> Self {
        self
    }

    fn window_classname<S: Into<String>>(self, classname: S) -> Self {
        self
    }

    fn shadow(self, enable: bool) -> Self {
        self
    }

    #[cfg(target_os = "macos")]
    fn parent(self, parent: *mut std::ffi::c_void) -> Self {
        self
    }

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

    #[cfg(windows)]
    fn drag_and_drop(self, enabled: bool) -> Self {
        self
    }

    #[cfg(target_os = "macos")]
    fn title_bar_style(self, style: TitleBarStyle) -> Self {
        self
    }

    #[cfg(target_os = "macos")]
    fn hidden_title(self, transparent: bool) -> Self {
        self
    }

    #[cfg(target_os = "macos")]
    fn tabbing_identifier(self, identifier: &str) -> Self {
        self
    }

    fn theme(self, theme: Option<Theme>) -> Self {
        self
    }

    fn has_icon(&self) -> bool {
        false
    }

    fn get_theme(&self) -> Option<Theme> {
        None
    }

    fn background_color(self, _color: tauri_utils::config::Color) -> Self {
        self
    }

    fn owner(self, owner: HWND) -> Self {
        todo!()
    }

    fn parent(self, parent: HWND) -> Self {
        todo!()
    }

    fn transparent(self, transparent: bool) -> Self {
        todo!()
    }
}

impl<T: UserEvent> WebviewDispatch<T> for MockWebviewDispatcher {
    type Runtime = MockRuntime;

    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
    }

    fn on_webview_event<F: Fn(&tauri_runtime::window::WebviewEvent) + Send + 'static>(
        &self,
        f: F,
    ) -> tauri_runtime::WebviewEventId {
        self.context.next_window_event_id()
    }

    fn with_webview<F: FnOnce(Box<dyn std::any::Any>) + Send + 'static>(&self, f: F) -> Result<()> {
        Ok(())
    }

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
        Ok(self.url.lock().unwrap().clone())
    }

    fn bounds(&self) -> Result<tauri_runtime::Rect> {
        Ok(tauri_runtime::Rect::default())
    }

    fn position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(PhysicalPosition { x: 0, y: 0 })
    }

    fn size(&self) -> Result<PhysicalSize<u32>> {
        Ok(PhysicalSize {
            width: 0,
            height: 0,
        })
    }

    fn navigate(&self, url: Url) -> Result<()> {
        *self.url.lock().unwrap() = url.to_string();
        Ok(())
    }

    fn print(&self) -> Result<()> {
        Ok(())
    }

    fn close(&self) -> Result<()> {
        Ok(())
    }

    fn set_bounds(&self, bounds: tauri_runtime::Rect) -> Result<()> {
        Ok(())
    }

    fn set_size(&self, _size: Size) -> Result<()> {
        Ok(())
    }

    fn set_position(&self, _position: Position) -> Result<()> {
        Ok(())
    }

    fn set_focus(&self) -> Result<()> {
        Ok(())
    }

    fn reparent(&self, window_id: WindowId) -> Result<()> {
        Ok(())
    }

    fn set_auto_resize(&self, auto_resize: bool) -> Result<()> {
        Ok(())
    }

    fn clear_all_browsing_data(&self) -> Result<()> {
        Ok(())
    }

    fn hide(&self) -> Result<()> {
        Ok(())
    }

    fn show(&self) -> Result<()> {
        Ok(())
    }

    fn set_background_color(&self, color: Option<tauri_utils::config::Color>) -> Result<()> {
        Ok(())
    }

    fn open_devtools(&self) {
        todo!()
    }

    fn close_devtools(&self) {
        todo!()
    }

    fn is_devtools_open(&self) -> Result<bool> {
        todo!()
    }
}

impl<T: UserEvent> WindowDispatch<T> for MockWindowDispatcher {
    type Runtime = MockRuntime;

    type WindowBuilder = MockWindowBuilder;

    fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.context.send_message(Message::Task(Box::new(f)))
    }

    fn on_window_event<F: Fn(&WindowEvent) + Send + 'static>(&self, f: F) -> WindowEventId {
        self.context.next_window_event_id()
    }

    fn scale_factor(&self) -> Result<f64> {
        Ok(1.0)
    }

    fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(PhysicalPosition { x: 0, y: 0 })
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        Ok(PhysicalPosition { x: 0, y: 0 })
    }

    fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        Ok(PhysicalSize {
            width: 0,
            height: 0,
        })
    }

    fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        Ok(PhysicalSize {
            width: 0,
            height: 0,
        })
    }

    fn is_fullscreen(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_minimized(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_maximized(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_focused(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_decorated(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_resizable(&self) -> Result<bool> {
        Ok(false)
    }

    fn is_maximizable(&self) -> Result<bool> {
        Ok(true)
    }

    fn is_minimizable(&self) -> Result<bool> {
        Ok(true)
    }

    fn is_closable(&self) -> Result<bool> {
        Ok(true)
    }

    fn is_visible(&self) -> Result<bool> {
        Ok(true)
    }

    fn title(&self) -> Result<String> {
        Ok(String::new())
    }

    fn current_monitor(&self) -> Result<Option<Monitor>> {
        Ok(None)
    }

    fn primary_monitor(&self) -> Result<Option<Monitor>> {
        Ok(None)
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        Ok(None)
    }

    fn available_monitors(&self) -> Result<Vec<Monitor>> {
        Ok(Vec::new())
    }

    fn theme(&self) -> Result<Theme> {
        Ok(Theme::Light)
    }

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

    fn center(&self) -> Result<()> {
        Ok(())
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        Ok(())
    }

    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &mut self,
        pending: PendingWindow<T, Self::Runtime>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self::Runtime>> {
        self.context.create_window(pending, after_window_creation)
    }

    fn create_webview(
        &mut self,
        pending: PendingWebview<T, Self::Runtime>,
    ) -> Result<DetachedWebview<T, Self::Runtime>> {
        todo!()
    }

    fn set_resizable(&self, resizable: bool) -> Result<()> {
        Ok(())
    }

    fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        Ok(())
    }

    fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        Ok(())
    }

    fn set_closable(&self, closable: bool) -> Result<()> {
        Ok(())
    }

    fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        Ok(())
    }

    fn maximize(&self) -> Result<()> {
        Ok(())
    }

    fn unmaximize(&self) -> Result<()> {
        Ok(())
    }

    fn minimize(&self) -> Result<()> {
        Ok(())
    }

    fn unminimize(&self) -> Result<()> {
        Ok(())
    }

    fn show(&self) -> Result<()> {
        Ok(())
    }

    fn hide(&self) -> Result<()> {
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

    fn set_decorations(&self, decorations: bool) -> Result<()> {
        Ok(())
    }

    fn set_shadow(&self, shadow: bool) -> Result<()> {
        Ok(())
    }

    fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        Ok(())
    }

    fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        Ok(())
    }

    fn set_visible_on_all_workspaces(&self, visible_on_all_workspaces: bool) -> Result<()> {
        Ok(())
    }

    fn set_content_protected(&self, protected: bool) -> Result<()> {
        Ok(())
    }

    fn set_size(&self, size: Size) -> Result<()> {
        Ok(())
    }

    fn set_min_size(&self, size: Option<Size>) -> Result<()> {
        Ok(())
    }

    fn set_max_size(&self, size: Option<Size>) -> Result<()> {
        Ok(())
    }

    fn set_position(&self, position: Position) -> Result<()> {
        Ok(())
    }

    fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        Ok(())
    }

    fn set_focus(&self) -> Result<()> {
        Ok(())
    }

    fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        Ok(())
    }

    fn set_skip_taskbar(&self, skip: bool) -> Result<()> {
        Ok(())
    }

    fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        Ok(())
    }

    fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        Ok(())
    }

    fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        Ok(())
    }

    fn set_cursor_position<Pos: Into<Position>>(&self, position: Pos) -> Result<()> {
        Ok(())
    }

    fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        Ok(())
    }

    fn start_dragging(&self) -> Result<()> {
        Ok(())
    }

    fn start_resize_dragging(&self, direction: tauri_runtime::ResizeDirection) -> Result<()> {
        Ok(())
    }

    fn set_progress_bar(&self, progress_state: ProgressBarState) -> Result<()> {
        Ok(())
    }

    fn set_badge_count(&self, count: Option<i64>, desktop_filename: Option<String>) -> Result<()> {
        Ok(())
    }

    fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        Ok(())
    }

    fn set_overlay_icon(&self, icon: Option<Icon<'_>>) -> Result<()> {
        Ok(())
    }

    fn set_title_bar_style(&self, style: tauri_utils::TitleBarStyle) -> Result<()> {
        Ok(())
    }

    fn set_size_constraints(
        &self,
        constraints: tauri_runtime::window::WindowSizeConstraints,
    ) -> Result<()> {
        Ok(())
    }

    fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        Ok(())
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool> {
        Ok(true)
    }

    fn set_background_color(&self, color: Option<tauri_utils::config::Color>) -> Result<()> {
        Ok(())
    }

    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct EventProxy {}

impl<T: UserEvent> EventLoopProxy<T> for EventProxy {
    fn send_event(&self, event: T) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct MockRuntime {
    is_running: Arc<AtomicBool>,
    pub context: RuntimeContext,
    run_rx: Receiver<Message>,
}

impl MockRuntime {
    fn init() -> Self {
        let is_running = Arc::new(AtomicBool::new(false));
        let (tx, rx) = sync_channel(256);
        let context = RuntimeContext {
            is_running: is_running.clone(),
            windows: Default::default(),
            run_tx: tx,
            next_window_id: Default::default(),
            next_webview_id: Default::default(),
            next_window_event_id: Default::default(),
            next_webview_event_id: Default::default(),
        };
        Self {
            is_running,
            context,
            run_rx: rx,
        }
    }
}

impl<T: UserEvent> Runtime<T> for MockRuntime {
    type WindowDispatcher = MockWindowDispatcher;
    type WebviewDispatcher = MockWebviewDispatcher;
    type Handle = MockRuntimeHandle;
    type EventLoopProxy = EventProxy;

    fn new(_args: RuntimeInitArgs) -> Result<Self> {
        Ok(Self::init())
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn new_any_thread(_args: RuntimeInitArgs) -> Result<Self> {
        Ok(Self::init())
    }

    fn create_proxy(&self) -> EventProxy {
        EventProxy {}
    }

    fn handle(&self) -> Self::Handle {
        MockRuntimeHandle {
            context: self.context.clone(),
        }
    }

    fn create_window<F: Fn(RawWindow<'_>) + Send + 'static>(
        &self,
        pending: PendingWindow<T, Self>,
        after_window_creation: Option<F>,
    ) -> Result<DetachedWindow<T, Self>> {
        self.context.create_window(pending, after_window_creation)
    }

    fn create_webview(
        &self,
        window_id: WindowId,
        pending: PendingWebview<T, Self>,
    ) -> Result<DetachedWebview<T, Self>> {
        todo!()
    }

    fn primary_monitor(&self) -> Option<Monitor> {
        unimplemented!()
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        unimplemented!()
    }

    fn available_monitors(&self) -> Vec<Monitor> {
        unimplemented!()
    }

    fn set_theme(&self, theme: Option<Theme>) {
        unimplemented!()
    }

    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_activation_policy(&mut self, activation_policy: tauri_runtime::ActivationPolicy) {}

    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn show(&self) {}

    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn hide(&self) {}

    fn set_device_event_filter(&mut self, filter: DeviceEventFilter) {}

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
        self.is_running.store(true, Ordering::Relaxed);
        callback(RunEvent::Ready);

        loop {
            if let Ok(m) = self.run_rx.try_recv() {
                match m {
                    Message::Task(p) => p(),
                    Message::CloseWindow(id) => {
                        let mut windows = self.context.windows.lock().unwrap();
                        let label = windows.get(&id).map(|w| w.label.clone());
                        if let Some(label) = label {
                            let (tx, rx) = channel();
                            callback(RunEvent::WindowEvent {
                                label,
                                event: WindowEvent::CloseRequested { signal_tx: tx },
                            });

                            let should_prevent = matches!(rx.try_recv(), Ok(true));
                            if !should_prevent {
                                windows.remove(&id);

                                let is_empty = windows.is_empty();
                                if is_empty {
                                    let (tx, rx) = channel();
                                    callback(RunEvent::ExitRequested { code: None, tx });

                                    let recv = rx.try_recv();
                                    let should_prevent =
                                        matches!(recv, Ok(ExitRequestedEventAction::Prevent));

                                    if !should_prevent {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Message::DestroyWindow(id) => {
                        let mut windows = self.context.windows.lock().unwrap();
                        let removed = windows.remove(&id).is_some();
                        if removed {
                            let is_empty = windows.is_empty();
                            if is_empty {
                                let (tx, rx) = channel();
                                callback(RunEvent::ExitRequested { code: None, tx });

                                let recv = rx.try_recv();
                                let should_prevent =
                                    matches!(recv, Ok(ExitRequestedEventAction::Prevent));

                                if !should_prevent {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            callback(RunEvent::MainEventsCleared);

            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        callback(RunEvent::Exit);
    }

    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        todo!()
    }
}
