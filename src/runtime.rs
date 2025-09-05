#![allow(unused_variables)]

use tao::{
    event::{Event as TaoEvent, StartCause},
    event_loop::{
        ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy as TaoEventLoopProxy,
        EventLoopWindowTarget as TaoEventLoopWindowTarget,
    },
    platform::run_return::EventLoopExtRunReturn,
};
use tauri_runtime::{
    DeviceEventFilter, Error, EventLoopProxy, ExitRequestedEventAction, Result, RunEvent, Runtime,
    RuntimeHandle, RuntimeInitArgs, UserEvent, WindowEventId,
    dpi::PhysicalPosition,
    monitor::Monitor,
    webview::{DetachedWebview, PendingWebview},
    window::{
        DetachedWindow, DetachedWindowWebview, PendingWindow, RawWindow, WindowBuilder,
        WindowEvent, WindowId,
    },
};
use tauri_utils::Theme;
use url::Url;
use verso::CustomProtocolBuilder;

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Debug},
    ops::Deref,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc::channel,
    },
    thread::{ThreadId, current as current_thread},
};

use crate::{
    event_loop_ext::TaoEventLoopWindowTargetExt,
    get_verso_path,
    utils::{to_tao_theme, to_verso_theme},
    webview::VersoWebviewDispatcher,
    window::{VersoWindowDispatcher, Window},
};

type Task = Box<dyn FnOnce() + Send + 'static>;
type TaskWithEventLoop<T> = Box<dyn FnOnce(&TaoEventLoopWindowTarget<Message<T>>) + Send + 'static>;

pub enum Message<T: UserEvent> {
    Task(Task),
    /// Run task with the [`EventLoopWindowTarget`](TaoEventLoopWindowTarget)
    TaskWithEventLoop(TaskWithEventLoop<T>),
    CloseWindow(WindowId),
    DestroyWindow(WindowId),
    RequestExit(i32),
    UserEvent(T),
}

impl<T: UserEvent> Clone for Message<T> {
    fn clone(&self) -> Self {
        match self {
            Self::UserEvent(t) => Self::UserEvent(t.clone()),
            _ => unimplemented!(),
        }
    }
}

#[derive(Clone)]
pub struct DispatcherMainThreadContext<T: UserEvent> {
    window_target: TaoEventLoopWindowTarget<Message<T>>,
}

// SAFETY: we ensure this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: UserEvent> Send for DispatcherMainThreadContext<T> {}

// SAFETY: we ensure this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: UserEvent> Sync for DispatcherMainThreadContext<T> {}

#[derive(Clone)]
pub struct RuntimeContext<T: UserEvent> {
    windows: Arc<Mutex<HashMap<WindowId, Window>>>,
    prefered_theme: Arc<Mutex<Option<Theme>>>,
    event_proxy: TaoEventLoopProxy<Message<T>>,
    // This must only be used on main thread
    main_thread: DispatcherMainThreadContext<T>,
    main_thread_id: ThreadId,
    next_window_id: Arc<AtomicU32>,
    next_webview_id: Arc<AtomicU32>,
    next_window_event_id: Arc<AtomicU32>,
    next_webview_event_id: Arc<AtomicU32>,
}

impl<T: UserEvent> RuntimeContext<T> {
    pub fn send_message(&self, message: Message<T>) -> Result<()> {
        if current_thread().id() == self.main_thread_id {
            match message {
                Message::Task(task) => {
                    task();
                    return Ok(());
                }
                Message::TaskWithEventLoop(task) => {
                    task(&self.main_thread.window_target);
                    return Ok(());
                }
                _ => {}
            }
        }
        self.event_proxy
            .send_event(message)
            .map_err(|_| Error::FailedToSendMessage)?;
        Ok(())
    }

    /// Run a task on the main thread.
    pub fn run_on_main_thread<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.send_message(Message::Task(Box::new(f)))
    }

    /// Run a task on the main thread.
    pub fn run_on_main_thread_with_event_loop<
        X: Send + Sync + 'static,
        F: FnOnce(&TaoEventLoopWindowTarget<Message<T>>) -> X + Send + 'static,
    >(
        &self,
        f: F,
    ) -> Result<X> {
        let (tx, rx) = channel();
        self.send_message(Message::TaskWithEventLoop(Box::new(move |e| {
            let _ = tx.send(f(e));
        })))?;
        rx.recv()
            .map_err(|_| tauri_runtime::Error::FailedToReceiveMessage)
    }

    pub fn next_window_id(&self) -> WindowId {
        self.next_window_id.fetch_add(1, Ordering::Relaxed).into()
    }

    pub fn next_webview_id(&self) -> u32 {
        self.next_webview_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn next_window_event_id(&self) -> WindowEventId {
        self.next_window_event_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn next_webview_event_id(&self) -> WindowEventId {
        self.next_webview_event_id.fetch_add(1, Ordering::Relaxed)
    }

    /// `after_window_creation` not supported
    ///
    /// Only creating the window with a webview is supported,
    /// will return [`tauri_runtime::Error::CreateWindow`] if there is no [`PendingWindow::webview`]
    pub fn create_window<
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

        let mut window_builder = pending.window_builder;

        if window_builder.get_theme().is_none() {
            window_builder = window_builder.theme(*self.prefered_theme.lock().unwrap());
        }

        let webview = window_builder
            .verso_builder
            .user_scripts(
                pending_webview
                    .webview_attributes
                    .initialization_scripts
                    .into_iter()
                    .map(|script| script.script),
            )
            .custom_protocols(
                pending_webview
                    .uri_scheme_protocols
                    .keys()
                    .map(CustomProtocolBuilder::new),
            )
            .build(get_verso_path(), Url::parse(&pending_webview.url).unwrap());

        let webview_label = label.clone();
        let sender = self.event_proxy.clone();
        let uri_scheme_protocols: HashMap<_, _> = pending_webview
            .uri_scheme_protocols
            .into_iter()
            .map(|(key, value)| (key, Arc::new(value)))
            .collect();
        webview
            .on_web_resource_requested(move |mut request, response_fn| {
                // dbg!(&request);
                // TODO: Servo's EmbedderMsg::WebResourceRequested message is sent too early
                // that it doesn't include Origin header, so I hard coded this for now
                if !request.headers().contains_key("Origin") {
                    #[cfg(windows)]
                    let uri = {
                        let scheme = if pending_webview.webview_attributes.use_https_scheme {
                            "https"
                        } else {
                            "http"
                        };
                        format!("{scheme}://tauri.localhost")
                    };
                    #[cfg(not(windows))]
                    let uri = "tauri://localhost";
                    request.headers_mut().insert("Origin", uri.parse().unwrap());
                }
                for (scheme, handler) in &uri_scheme_protocols {
                    // Since servo doesn't support body in its EmbedderMsg::WebResourceRequested yet,
                    // we use a header instead for now
                    if scheme == "ipc" {
                        if let Some(data) = request
                            .headers_mut()
                            .remove("Tauri-VersoRuntime-Invoke-Body")
                        {
                            if let Ok(body) =
                                percent_encoding::percent_decode(data.as_bytes()).decode_utf8()
                            {
                                *request.body_mut() = body.as_bytes().to_vec();
                            } else {
                                log::error!("IPC invoke body header is not a valid UTF-8 string");
                            }
                        }
                    }
                    #[cfg(windows)]
                    let (uri, http_or_https) = (
                        request.uri().to_string(),
                        if pending_webview.webview_attributes.use_https_scheme {
                            "https"
                        } else {
                            "http"
                        },
                    );
                    #[cfg(windows)]
                    let is_custom_protocol_uri = is_work_around_uri(&uri, http_or_https, scheme);
                    #[cfg(not(windows))]
                    let is_custom_protocol_uri = request.uri().scheme_str() == Some(scheme);
                    if is_custom_protocol_uri {
                        #[cfg(windows)]
                        {
                            if let Ok(reverted) =
                                revert_custom_protocol_work_around(&uri, http_or_https, scheme)
                            {
                                *request.uri_mut() = reverted
                            } else {
                                log::error!("Can't revert the URI work around on: {uri}")
                            };
                        }
                        // Run the handler on main thread, this is needed because Tauri expects this
                        let handler = handler.clone();
                        let webview_label = webview_label.clone();
                        let _ = sender.send_event(Message::Task(Box::new(move || {
                            handler(
                                &webview_label,
                                request,
                                Box::new(move |response| {
                                    response_fn(Some(response.map(Cow::into_owned)));
                                }),
                            );
                        })));
                        return;
                    }
                }
                response_fn(None);
            })
            .map_err(|_| tauri_runtime::Error::CreateWindow)?;

        if let Some(navigation_handler) = pending_webview.navigation_handler {
            if let Err(error) = webview.on_navigation_starting(move |url| navigation_handler(&url))
            {
                log::error!(
                    "Register `on_navigation_starting` failed with {error}, `navigation_handler` will not get called for this window ({label})!"
                );
            }
        }

        let sender = self.event_proxy.clone();
        webview
            .on_close_requested(move || {
                let _ = sender.send_event(Message::CloseWindow(window_id));
            })
            .map_err(|_| tauri_runtime::Error::CreateWindow)?;

        let on_window_event_listeners = Arc::new(Mutex::new(HashMap::new()));

        let webview = Arc::new(Mutex::new(webview));
        let window = Window {
            label: label.clone(),
            webview: webview.clone(),
            on_window_event_listeners: on_window_event_listeners.clone(),
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
                    label,
                    dispatcher: VersoWebviewDispatcher {
                        id: webview_id,
                        context: self.clone(),
                        webview,
                    },
                },
                use_https_scheme: false,
            }),
        })
    }

    /// Handles the close window request by sending the [`WindowEvent::CloseRequested`] event
    /// if the request doesn't request a forced close
    /// and if not prevented, send [`WindowEvent::Destroyed`]
    /// then checks if there're windows left, if not, send [`RunEvent::ExitRequested`]
    /// returns if we should exit the event loop
    pub fn handle_close_window_request<F: FnMut(RunEvent<T>) + 'static>(
        &self,
        callback: &mut F,
        id: WindowId,
        force: bool,
    ) -> bool {
        let mut windows = self.windows.lock().unwrap();
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

        let webview_weak = std::sync::Arc::downgrade(&window.webview);

        windows.remove(&id);
        callback(RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
        });

        // This is required becuase tauri puts in a clone of the window in to WindowEventHandler closure,
        // and we need to clear it for the window to drop or else it will stay there forever
        on_window_event_listeners.lock().unwrap().clear();

        if let Some(webview) = webview_weak.upgrade() {
            log::warn!(
                "The versoview controller reference count is not 0 on window close, \
                there're leaks happening, shutting down this versoview instance regardless"
            );
            if let Err(error) = webview.lock().unwrap().exit() {
                log::error!("Failed to exit the webview: {error}");
            }
        }

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

// Copied from wry
/// WebView2 supports non-standard protocols only on Windows 10+, so we have to use a workaround,
/// conveting `{protocol}://localhost/abc` to `{http_or_https}://{protocol}.localhost/abc`,
/// and this function tests if the URI starts with `{http_or_https}://{protocol}.`
///
/// See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
#[cfg(windows)]
fn is_work_around_uri(uri: &str, http_or_https: &str, protocol: &str) -> bool {
    uri.strip_prefix(http_or_https)
        .and_then(|rest| rest.strip_prefix("://"))
        .and_then(|rest| rest.strip_prefix(protocol))
        .and_then(|rest| rest.strip_prefix("."))
        .is_some()
}

// This is a work around wry did for old version of webview2, and tauri also expects it...
// On Windows, the custom protocol looks like `http://<scheme_name>.<path>` while other platforms, it looks like `<scheme_name>://<path>`
// And we need to revert this here to align with the wry behavior...
#[cfg(windows)]
fn revert_custom_protocol_work_around(
    uri: &str,
    http_or_https: &'static str,
    protocol: &str,
) -> std::result::Result<http::Uri, http::uri::InvalidUri> {
    uri.replace(
        &work_around_uri_prefix(http_or_https, protocol),
        &format!("{protocol}://"),
    )
    .parse()
}

#[cfg(windows)]
fn work_around_uri_prefix(http_or_https: &str, protocol: &str) -> String {
    format!("{http_or_https}://{protocol}.")
}

impl<T: UserEvent> Debug for RuntimeContext<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeContext").finish()
    }
}

/// A handle to the [`VersoRuntime`] runtime.
#[derive(Debug, Clone)]
pub struct VersoRuntimeHandle<T: UserEvent> {
    context: RuntimeContext<T>,
}

impl<T: UserEvent> RuntimeHandle<T> for VersoRuntimeHandle<T> {
    type Runtime = VersoRuntime<T>;

    fn create_proxy(&self) -> EventProxy<T> {
        EventProxy(self.context.event_proxy.clone())
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

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_dock_visibility(&self, visible: bool) -> Result<()> {
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
        self.context.run_on_main_thread(f)
    }

    fn primary_monitor(&self) -> Option<Monitor> {
        self.context
            .run_on_main_thread_with_event_loop(|e| e.tauri_primary_monitor())
            .ok()
            .flatten()
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        self.context
            .run_on_main_thread_with_event_loop(move |e| e.tauri_monitor_from_point(x, y))
            .ok()
            .flatten()
    }

    fn available_monitors(&self) -> Vec<Monitor> {
        self.context
            .run_on_main_thread_with_event_loop(|e| e.tauri_available_monitors())
            .unwrap()
    }

    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        self.context
            .run_on_main_thread_with_event_loop(|e| e.tauri_cursor_position())?
    }

    fn set_theme(&self, theme: Option<Theme>) {
        *self.context.prefered_theme.lock().unwrap() = theme;
        for window in self.context.windows.lock().unwrap().values() {
            if let Err(error) = window
                .webview
                .lock()
                .unwrap()
                .set_theme(theme.map(to_verso_theme))
            {
                log::error!("Failed to set the theme for webview: {error}");
            }
        }
        let _ = self
            .context
            .run_on_main_thread_with_event_loop(move |e| e.set_theme(theme.map(to_tao_theme)));
    }

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

    /// Unsupported, will always return an error
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError>
    {
        Err(raw_window_handle::HandleError::NotSupported)
    }

    /// Unsupported, has no effect, the callback will not be called
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "macos", target_os = "ios"))))]
    fn fetch_data_store_identifiers<F: FnOnce(Vec<[u8; 16]>) + Send + 'static>(
        &self,
        cb: F,
    ) -> Result<()> {
        Ok(())
    }

    /// Unsupported, has no effect, the callback will not be called
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "macos", target_os = "ios"))))]
    fn remove_data_store<F: FnOnce(Result<()>) + Send + 'static>(
        &self,
        uuid: [u8; 16],
        cb: F,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EventProxy<T: UserEvent>(TaoEventLoopProxy<Message<T>>);

impl<T: UserEvent> EventLoopProxy<T> for EventProxy<T> {
    fn send_event(&self, event: T) -> Result<()> {
        self.0
            .send_event(Message::UserEvent(event))
            .map_err(|_| Error::FailedToSendMessage)
    }
}

/// A Tauri Runtime wrapper around Verso.
#[derive(Debug)]
pub struct VersoRuntime<T: UserEvent = tauri::EventLoopMessage> {
    pub context: RuntimeContext<T>,
    event_loop: EventLoop<Message<T>>,
}

impl<T: UserEvent> VersoRuntime<T> {
    fn init(event_loop: EventLoop<Message<T>>) -> Self {
        let context = RuntimeContext {
            windows: Default::default(),
            prefered_theme: Arc::default(),
            event_proxy: event_loop.create_proxy(),
            main_thread: DispatcherMainThreadContext {
                window_target: event_loop.deref().clone(),
            },
            main_thread_id: current_thread().id(),
            next_window_id: Default::default(),
            next_webview_id: Default::default(),
            next_window_event_id: Default::default(),
            next_webview_event_id: Default::default(),
        };
        Self {
            context,
            event_loop,
        }
    }

    fn init_with_builder(
        mut event_loop_builder: EventLoopBuilder<Message<T>>,
        args: RuntimeInitArgs,
    ) -> Self {
        #[cfg(windows)]
        if let Some(hook) = args.msg_hook {
            use tao::platform::windows::EventLoopBuilderExtWindows;
            event_loop_builder.with_msg_hook(hook);
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        if let Some(app_id) = args.app_id {
            use tao::platform::unix::EventLoopBuilderExtUnix;
            event_loop_builder.with_app_id(app_id);
        }
        Self::init(event_loop_builder.build())
    }
}

impl<T: UserEvent> Runtime<T> for VersoRuntime<T> {
    type WindowDispatcher = VersoWindowDispatcher<T>;
    type WebviewDispatcher = VersoWebviewDispatcher<T>;
    type Handle = VersoRuntimeHandle<T>;
    type EventLoopProxy = EventProxy<T>;

    /// `args.msg_hook` hooks on the event loop of this process,
    /// this doesn't work for the event loop of versoview instances
    fn new(args: RuntimeInitArgs) -> Result<Self> {
        let event_loop_builder = EventLoopBuilder::<Message<T>>::with_user_event();
        Ok(Self::init_with_builder(event_loop_builder, args))
    }

    /// `args.msg_hook` hooks on the event loop of this process,
    /// this doesn't work for the event loop of versoview instances
    #[cfg(any(windows, target_os = "linux"))]
    fn new_any_thread(args: RuntimeInitArgs) -> Result<Self> {
        let mut event_loop_builder = EventLoopBuilder::<Message<T>>::with_user_event();
        #[cfg(target_os = "linux")]
        use tao::platform::unix::EventLoopBuilderExtUnix;
        #[cfg(windows)]
        use tao::platform::windows::EventLoopBuilderExtWindows;
        event_loop_builder.with_any_thread(true);
        Ok(Self::init_with_builder(event_loop_builder, args))
    }

    fn create_proxy(&self) -> EventProxy<T> {
        EventProxy(self.event_loop.create_proxy())
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

    fn primary_monitor(&self) -> Option<Monitor> {
        self.event_loop.tauri_primary_monitor()
    }

    fn monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        self.event_loop.tauri_monitor_from_point(x, y)
    }

    fn available_monitors(&self) -> Vec<Monitor> {
        self.event_loop.tauri_available_monitors()
    }

    fn cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        self.event_loop.tauri_cursor_position()
    }

    fn set_theme(&self, theme: Option<Theme>) {
        *self.context.prefered_theme.lock().unwrap() = theme;
        for window in self.context.windows.lock().unwrap().values() {
            if let Err(error) = window
                .webview
                .lock()
                .unwrap()
                .set_theme(theme.map(to_verso_theme))
            {
                log::error!("Failed to set the theme for webview: {error}");
            }
        }
        self.event_loop.set_theme(theme.map(to_tao_theme));
    }

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

    /// Unsupported, has no effect
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    fn set_dock_visibility(&mut self, visible: bool) {}

    /// Unsupported, has no effect when called
    fn set_device_event_filter(&mut self, filter: DeviceEventFilter) {}

    /// Unsupported, has no effect when called
    fn run_iteration<F: FnMut(RunEvent<T>)>(&mut self, callback: F) {}

    fn run<F: FnMut(RunEvent<T>) + 'static>(self, callback: F) {
        let exit_code = self.run_return(callback);
        // std::process::exit(exit_code);
    }

    fn run_return<F: FnMut(RunEvent<T>) + 'static>(mut self, mut callback: F) -> i32 {
        self.event_loop
            .run_return(|event, event_loop, control_flow| {
                if *control_flow != ControlFlow::Exit {
                    *control_flow = ControlFlow::Wait;
                }

                match event {
                    TaoEvent::NewEvents(StartCause::Init) => {
                        callback(RunEvent::Ready);
                    }
                    TaoEvent::NewEvents(StartCause::Poll) => {
                        callback(RunEvent::Resumed);
                    }
                    TaoEvent::MainEventsCleared => {
                        callback(RunEvent::MainEventsCleared);
                    }
                    TaoEvent::LoopDestroyed => {
                        callback(RunEvent::Exit);
                    }
                    TaoEvent::UserEvent(user_event) => match user_event {
                        Message::Task(p) => p(),
                        Message::TaskWithEventLoop(p) => p(event_loop),
                        Message::CloseWindow(id) => {
                            let should_exit =
                                self.context
                                    .handle_close_window_request(&mut callback, id, false);
                            if should_exit {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                        Message::DestroyWindow(id) => {
                            let should_exit =
                                self.context
                                    .handle_close_window_request(&mut callback, id, true);
                            if should_exit {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                        Message::RequestExit(code) => {
                            let (tx, rx) = channel();
                            callback(RunEvent::ExitRequested {
                                code: Some(code),
                                tx,
                            });

                            let recv = rx.try_recv();
                            let should_prevent =
                                matches!(recv, Ok(ExitRequestedEventAction::Prevent));

                            if !should_prevent {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                        Message::UserEvent(user_event) => callback(RunEvent::UserEvent(user_event)),
                    },
                    _ => {}
                }
            })
    }
}
