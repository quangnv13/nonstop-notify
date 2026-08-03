mod actions;
mod event;
mod ipc;
mod toast_store;

use std::io::{self, BufRead, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actions::build_url;
use event::parse_event;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, PhysicalPosition, Position, Size, WebviewUrl, WebviewWindowBuilder,
};
use toast_store::{ToastStore, ToastView};

const DASHBOARD_BASE: &str = "http://127.0.0.1:4137";
const NOTIFICATION_SOUND: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/notification.wav"));
const WINDOW_WIDTH: f64 = 430.0;
const MAX_WINDOW_HEIGHT: f64 = 760.0;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum NotificationPosition {
    TopLeft,
    TopRight,
    #[default]
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct NotifyConfig {
    position: NotificationPosition,
    offset_left: u16,
    offset_right: u16,
    border_width: u16,
    sound_path: Option<PathBuf>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            position: NotificationPosition::default(),
            offset_left: 30,
            offset_right: 30,
            border_width: 1,
            sound_path: None,
        }
    }
}

fn parse_config(input: &str) -> Result<NotifyConfig, serde_json::Error> {
    serde_json::from_str(input)
}

fn load_config(path: Option<&Path>) -> Result<NotifyConfig, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(NotifyConfig::default());
    };
    let input = std::fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read notify config {}: {error}", path.display()),
        )
    })?;
    parse_config(&input).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid notify config {}: {error}", path.display()),
        )
        .into()
    })
}

fn config_path_from_args(args: &[String]) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if let Some(index) = args.iter().position(|arg| arg == "--config") {
        let path = args.get(index + 1).filter(|value| !value.starts_with("--"));
        return path.map(PathBuf::from).map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "--config requires a file path").into()
        });
    }
    Ok(std::env::var_os("NONSTOP_NOTIFY_CONFIG").map(PathBuf::from))
}

type SharedStore = Arc<Mutex<ToastStore>>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToastPayload {
    toasts: Vec<ToastView>,
    expanded: bool,
    theme: String,
    border_width: u16,
    position: NotificationPosition,
}

#[derive(Debug)]
struct UiState {
    store: SharedStore,
    expanded: Mutex<bool>,
    window_visible: Mutex<bool>,
    window_size: Mutex<(i32, i32)>,
    config: NotifyConfig,
}

impl UiState {
    fn new(config: NotifyConfig) -> Self {
        Self {
            store: SharedStore::default(),
            expanded: Mutex::new(false),
            window_visible: Mutex::new(false),
            window_size: Mutex::new((0, 0)),
            config,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = config_path_from_args(&args).and_then(|config_path| {
        if args.iter().any(|arg| arg == "--self-check") {
            self_check(config_path.as_deref())
        } else if args.get(1).map(String::as_str) == Some("emit")
            && args.iter().any(|arg| arg == "--stdin")
        {
            emit_stdin(config_path.as_deref())
        } else if args.get(1).map(String::as_str) == Some("emit")
            && args.get(2).map(String::as_str) == Some("--json")
        {
            emit_json(
                args.get(3).cloned().unwrap_or_default(),
                config_path.as_deref(),
            )
        } else if args.get(1).map(String::as_str) == Some("daemon") {
            if std::env::var_os("NONSTOP_NOTIFY_DAEMONIZED").is_some() {
                run_daemon(config_path.as_deref())
            } else {
                ipc::spawn_daemon(config_path.as_deref())
                    .map(|_| ())
                    .map_err(Into::into)
            }
        } else {
            Ok(())
        }
    });

    if let Err(error) = result {
        if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() {
            eprintln!("nonstop-notify: {error}");
        }
        std::process::exit(1);
    }
}

fn self_check(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    parse_event(r#"{"event":"test.selfCheck","toastId":"self-check","progress":2}"#)?;
    validate_notification_sound(config.sound_path.as_deref())?;
    assert_eq!(
        build_url(DASHBOARD_BASE, "/runs/manual").as_deref(),
        Some("http://127.0.0.1:4137/runs/manual")
    );
    println!("nonstop-notify self-check ok");
    Ok(())
}

fn emit_stdin(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    emit_json(input, config_path)
}

fn emit_json(input: String, config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    if input.trim().is_empty() {
        return Ok(());
    }

    ipc::append_event_json(&input)?;
    if !daemon_heartbeat_is_fresh() {
        let _ = ipc::spawn_daemon(config_path);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
struct DaemonLock {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(not(target_os = "windows"))]
struct DaemonLock {
    path: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
impl Drop for DaemonLock {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(target_os = "windows")]
fn acquire_daemon_lock() -> Result<Option<DaemonLock>, Box<dyn std::error::Error>> {
    acquire_windows_daemon_lock("Local\\nonstop-notify-daemon")
}

#[cfg(target_os = "windows")]
fn acquire_windows_daemon_lock(
    name: &str,
) -> Result<Option<DaemonLock>, Box<dyn std::error::Error>> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let wide_name = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error().into());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return Ok(None);
    }
    Ok(Some(DaemonLock { handle }))
}

#[cfg(not(target_os = "windows"))]
fn acquire_daemon_lock() -> Result<Option<DaemonLock>, Box<dyn std::error::Error>> {
    let path = daemon_lock_path();
    match create_daemon_lock(&path)? {
        Some(lock) => Ok(Some(lock)),
        None if !daemon_heartbeat_is_fresh() => {
            let _ = std::fs::remove_file(&path);
            create_daemon_lock(&path)
        }
        None => Ok(None),
    }
}

#[cfg(not(target_os = "windows"))]
fn create_daemon_lock(
    path: &std::path::Path,
) -> Result<Option<DaemonLock>, Box<dyn std::error::Error>> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            let _ = writeln!(file, "{}", std::process::id());
            Ok(Some(DaemonLock {
                path: path.to_path_buf(),
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(target_os = "windows"))]
fn daemon_lock_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("nonstop-notify-daemon.lock");
    path
}

fn daemon_heartbeat_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("nonstop-notify-daemon.heartbeat");
    path
}

fn daemon_heartbeat_is_fresh() -> bool {
    std::fs::metadata(daemon_heartbeat_path())
        .and_then(|metadata| metadata.modified())
        .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
        .map(|age| age < Duration::from_secs(3))
        .unwrap_or(false)
}

fn touch_daemon_heartbeat() {
    let _ = std::fs::write(daemon_heartbeat_path(), b"ok");
}

fn run_daemon(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let Some(_daemon_lock) = acquire_daemon_lock()? else {
        return Ok(());
    };
    let config = load_config(config_path)?;
    tauri::Builder::default()
        .manage(UiState::new(config))
        .invoke_handler(tauri::generate_handler![
            close_toast,
            open_route,
            set_expanded,
            report_layout,
            request_state
        ])
        .setup(|app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Nonstop Notify")
                .inner_size(WINDOW_WIDTH, 1.0)
                .position(10000.0, 10000.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .shadow(false)
                .build()?;
            let handle = app.handle().clone();
            hide_from_taskbar(&handle);
            park_window(&handle, 1.0, 1.0);
            start_event_listener(handle.clone());
            start_prune_timer(handle);
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}

#[tauri::command]
fn close_toast(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    state
        .store
        .lock()
        .map_err(|_| "toast store lock poisoned".to_string())?
        .dismiss(&id);
    emit_state(&app)
}

#[tauri::command]
fn open_route(route: String) -> Result<(), String> {
    if let Some(url) = build_url(DASHBOARD_BASE, &route) {
        open::that(url).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    let state = app.state::<UiState>();
    *state
        .expanded
        .lock()
        .map_err(|_| "expanded lock poisoned".to_string())? = expanded;
    emit_state(&app)
}

#[tauri::command]
fn request_state(app: AppHandle) -> Result<(), String> {
    emit_state(&app)
}

#[tauri::command]
fn report_layout(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let width = width.clamp(WINDOW_WIDTH, WINDOW_WIDTH);
    let height = height.clamp(1.0, MAX_WINDOW_HEIGHT);
    resize_to_content(&app, width, height)
}

fn start_event_listener(app: AppHandle) {
    std::thread::spawn(move || {
        for event_json in ipc::drain_queued_events().unwrap_or_default() {
            apply_event(&app, &event_json);
        }
        let listener_app = app.clone();
        if let Err(error) =
            ipc::listen_events(move |event_json| apply_event(&listener_app, &event_json))
        {
            if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() {
                eprintln!("nonstop-notify ipc: {error}");
            }
        }
    });
}

fn start_prune_timer(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        touch_daemon_heartbeat();
        for event_json in ipc::drain_queued_events().unwrap_or_default() {
            apply_event(&app, &event_json);
        }
    });
}

fn apply_event(app: &AppHandle, event_json: &str) {
    match parse_event(event_json) {
        Ok(event) => {
            play_notification_sound(app.state::<UiState>().config.sound_path.clone());
            let state = app.state::<UiState>();
            if let Ok(mut store) = state.store.lock() {
                store.upsert(event);
            }
            let _ = emit_state(app);
        }
        Err(error) if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() => {
            eprintln!("nonstop-notify event: {error}");
        }
        Err(_) => {}
    }
}

fn validate_notification_sound(
    sound_path: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = sound_path {
        let bytes = read_custom_notification_sound(path)?;
        rodio::Decoder::try_from(Cursor::new(bytes)).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "failed to decode notification sound {}: {error}",
                    path.display()
                ),
            )
        })?;
    } else {
        rodio::Decoder::try_from(Cursor::new(NOTIFICATION_SOUND))?;
    }
    Ok(())
}

fn read_custom_notification_sound(path: &Path) -> Result<Vec<u8>, io::Error> {
    std::fs::read(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to read notification sound {}: {error}",
                path.display()
            ),
        )
    })
}

fn runtime_notification_sound_from_dir(directory: Option<&Path>) -> Option<Vec<u8>> {
    let directory = directory?;
    let path = directory.join("notify-ring.mp3");
    let sound = match std::fs::read(&path) {
        Ok(sound) => sound,
        Err(error) => {
            if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() {
                eprintln!("nonstop-notify sound fallback {}: {error}", path.display());
            }
            return None;
        }
    };
    if let Err(error) = rodio::Decoder::try_from(Cursor::new(sound.clone())) {
        if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() {
            eprintln!("nonstop-notify sound fallback {}: {error}", path.display());
        }
        return None;
    }
    Some(sound)
}

fn runtime_notification_sound_directory() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn play_notification_sound(sound_path: Option<PathBuf>) {
    let runtime_directory = runtime_notification_sound_directory();
    std::thread::spawn(move || {
        let result =
            play_notification_sound_inner(sound_path.as_deref(), runtime_directory.as_deref());
        if let Err(error) = result {
            if std::env::var_os("NONSTOP_NOTIFY_DEBUG").is_some() {
                eprintln!("nonstop-notify sound: {error}");
            }
        }
    });
}

fn play_notification_sound_inner(
    sound_path: Option<&Path>,
    runtime_directory: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = sound_path {
        let bytes = read_custom_notification_sound(path)?;
        return play_decoded_notification_sound(bytes, Some(path));
    }
    if let Some(bytes) = runtime_notification_sound_from_dir(runtime_directory) {
        let source_path = runtime_directory.map(|directory| directory.join("notify-ring.mp3"));
        return play_decoded_notification_sound(bytes, source_path.as_deref());
    }
    play_default_notification_sound()
}

fn play_decoded_notification_sound(
    bytes: Vec<u8>,
    source_path: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = rodio::DeviceSinkBuilder::open_default_sink()?;
    let player =
        rodio::play(stream.mixer(), Cursor::new(bytes)).map_err(|error| match source_path {
            Some(path) => io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "failed to decode notification sound {}: {error}",
                    path.display()
                ),
            ),
            None => io::Error::new(io::ErrorKind::InvalidData, error.to_string()),
        })?;
    player.sleep_until_end();
    Ok(())
}

#[cfg(target_os = "windows")]
fn play_default_notification_sound() -> Result<(), Box<dyn std::error::Error>> {
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION;

    if unsafe { MessageBeep(MB_ICONINFORMATION) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn play_default_notification_sound() -> Result<(), Box<dyn std::error::Error>> {
    play_decoded_notification_sound(NOTIFICATION_SOUND.to_vec(), None)
}

fn emit_state(app: &AppHandle) -> Result<(), String> {
    let payload = current_payload(app)?;
    let should_show = !payload.toasts.is_empty();
    let should_seed_layout = if should_show {
        let state = app.state::<UiState>();
        let visible = state
            .window_visible
            .lock()
            .map_err(|_| "window visible lock poisoned".to_string())?;
        !*visible
    } else {
        false
    };
    if should_seed_layout {
        resize_to_content(
            app,
            WINDOW_WIDTH,
            stacked_height(payload.toasts.len(), payload.expanded),
        )?;
    }
    update_window_visibility(app, should_show)?;
    if let Some(window) = app.get_webview_window("main") {
        let script = format!(
            "window.__NONSTOP_SET_TOASTS && window.__NONSTOP_SET_TOASTS({});",
            serde_json::to_string(&payload).map_err(|error| error.to_string())?
        );
        window.eval(&script).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn stacked_height(count: usize, expanded: bool) -> f64 {
    let count = count.clamp(1, 5);
    if expanded {
        (56.0 + count as f64 * 108.0).clamp(180.0, MAX_WINDOW_HEIGHT)
    } else if count > 1 {
        190.0
    } else {
        150.0
    }
}

fn current_payload(app: &AppHandle) -> Result<ToastPayload, String> {
    let state = app.state::<UiState>();
    let toasts = state
        .store
        .lock()
        .map_err(|_| "toast store lock poisoned".to_string())?
        .visible_views();
    let expanded = *state
        .expanded
        .lock()
        .map_err(|_| "expanded lock poisoned".to_string())?;
    Ok(ToastPayload {
        toasts,
        expanded,
        theme: system_theme(),
        border_width: state.config.border_width,
        position: state.config.position,
    })
}

fn system_theme() -> String {
    if system_prefers_dark() {
        "dark".into()
    } else {
        "light".into()
    }
}

#[cfg(target_os = "windows")]
fn system_prefers_dark() -> bool {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme".encode_utf16().chain(Some(0)).collect();
    let mut value = 1u32;
    let mut value_size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut value_size,
        )
    };
    prefers_dark_from_registry_value((status == 0).then_some(value))
}

#[cfg(not(target_os = "windows"))]
fn system_prefers_dark() -> bool {
    false
}

#[cfg(any(target_os = "windows", test))]
fn prefers_dark_from_registry_value(value: Option<u32>) -> bool {
    value == Some(0)
}

#[cfg(test)]
mod theme_tests {
    use super::{prefers_dark_from_registry_value, top_right_x};

    #[test]
    fn primary_monitor_position_accounts_for_scale_factor() {
        assert_eq!(top_right_x(0, 1920, 430.0, 1.5, 30), 1245);
    }

    #[test]
    fn windows_theme_registry_value_zero_means_dark() {
        assert!(prefers_dark_from_registry_value(Some(0)));
        assert!(!prefers_dark_from_registry_value(Some(1)));
        assert!(!prefers_dark_from_registry_value(None));
    }
}

fn update_window_visibility(app: &AppHandle, should_show: bool) -> Result<(), String> {
    let state = app.state::<UiState>();
    let mut visible = state
        .window_visible
        .lock()
        .map_err(|_| "window visible lock poisoned".to_string())?;
    if *visible == should_show {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("main") {
        if should_show {
            window.show().map_err(|error| error.to_string())?;
        } else {
            window.hide().map_err(|error| error.to_string())?;
        }
    }
    *visible = should_show;
    Ok(())
}

fn resize_to_content(app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
    if app.get_webview_window("main").is_none() {
        return Ok(());
    }
    let width_px = width.round() as i32;
    let height_px = height.round() as i32;
    let state = app.state::<UiState>();
    let mut last_size = state
        .window_size
        .lock()
        .map_err(|_| "window size lock poisoned".to_string())?;
    if *last_size == (width_px, height_px) {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        if *last_size == (0, 0) {
            position_window(app, WINDOW_WIDTH, MAX_WINDOW_HEIGHT)?;
        }
        set_content_region(app, height)?;
    }
    #[cfg(not(target_os = "windows"))]
    position_window(app, width, height)?;
    *last_size = (width_px, height_px);
    Ok(())
}

fn park_window(app: &AppHandle, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(Size::Logical(tauri::LogicalSize { width, height }));
        let _ = window.set_position(Position::Physical(PhysicalPosition { x: 10000, y: 10000 }));
    }
}

fn top_right_x(
    area_x: i32,
    area_width: u32,
    logical_window_width: f64,
    scale_factor: f64,
    offset_right: u16,
) -> i32 {
    let physical_window_width = (logical_window_width * scale_factor).round() as i32;
    (area_x + area_width as i32 - physical_window_width - i32::from(offset_right)).max(area_x)
}

fn window_position(
    position: NotificationPosition,
    area_x: i32,
    area_y: i32,
    area_width: u32,
    area_height: u32,
    logical_width: f64,
    logical_height: f64,
    scale_factor: f64,
    offset_left: u16,
    offset_right: u16,
) -> (i32, i32) {
    let physical_height = (logical_height * scale_factor).round() as i32;
    let left = area_x + i32::from(offset_left);
    let right = top_right_x(
        area_x,
        area_width,
        logical_width,
        scale_factor,
        offset_right,
    );
    let top = area_y + 30;
    let bottom = (area_y + area_height as i32 - physical_height - 30).max(area_y);
    match position {
        NotificationPosition::TopLeft => (left, top),
        NotificationPosition::TopRight => (right, top),
        NotificationPosition::BottomLeft => (left, bottom),
        NotificationPosition::BottomRight => (right, bottom),
    }
}

fn content_region_bounds(
    position: NotificationPosition,
    window_width: i32,
    window_height: i32,
    content_height: i32,
) -> (i32, i32, i32, i32) {
    let content_height = content_height.clamp(1, window_height);
    let top = match position {
        NotificationPosition::BottomLeft | NotificationPosition::BottomRight => {
            window_height - content_height
        }
        NotificationPosition::TopLeft | NotificationPosition::TopRight => 0,
    };
    (0, top, window_width, top + content_height)
}

#[cfg(target_os = "windows")]
fn set_content_region(app: &AppHandle, logical_height: f64) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, SetWindowRgn};

    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    let window_width = (WINDOW_WIDTH * scale_factor).round() as i32;
    let window_height = (MAX_WINDOW_HEIGHT * scale_factor).round() as i32;
    let content_height = (logical_height * scale_factor).round() as i32;
    let position = app.state::<UiState>().config.position;
    let (left, top, right, bottom) =
        content_region_bounds(position, window_width, window_height, content_height);
    let region = unsafe { CreateRectRgn(left, top, right, bottom) };
    if region.is_null() {
        return Err(io::Error::last_os_error().to_string());
    }
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let result = unsafe { SetWindowRgn(hwnd.0 as _, region, 1) };
    if result == 0 {
        unsafe { DeleteObject(region as _) };
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(())
}

fn position_window(app: &AppHandle, logical_width: f64, logical_height: f64) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let monitor = window
        .primary_monitor()
        .map_err(|error| error.to_string())?;
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale_factor = monitor.scale_factor();
    let state = app.state::<UiState>();
    let config = &state.config;
    let (x, y) = window_position(
        config.position,
        area.position.x,
        area.position.y,
        area.size.width,
        area.size.height,
        logical_width,
        logical_height,
        scale_factor,
        config.offset_left,
        config.offset_right,
    );
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
        };
        let hwnd = window.hwnd().map_err(|error| error.to_string())?;
        let physical_width = (logical_width * scale_factor).round() as i32;
        let physical_height = (logical_height * scale_factor).round() as i32;
        let result = unsafe {
            SetWindowPos(
                hwnd.0 as HWND,
                std::ptr::null_mut(),
                x,
                y,
                physical_width,
                physical_height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_size(Size::Logical(tauri::LogicalSize {
                width: logical_width,
                height: logical_height,
            }))
            .map_err(|error| error.to_string())?;
        window
            .set_position(Position::Physical(PhysicalPosition { x, y }))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod position_tests {
    use super::{
        content_region_bounds, parse_config, window_position, NotificationPosition, NotifyConfig,
    };
    use std::path::PathBuf;

    #[test]
    fn missing_config_defaults_to_bottom_left() {
        let config = NotifyConfig::default();
        assert_eq!(config.position, NotificationPosition::BottomLeft);
        assert_eq!(config.offset_left, 30);
        assert_eq!(config.offset_right, 30);
        assert_eq!(config.border_width, 1);
        assert_eq!(config.sound_path, None);
    }

    #[test]
    fn position_serializes_for_ui_payload() {
        assert_eq!(
            r#""bottom-right""#,
            serde_json::to_string(&NotificationPosition::BottomRight).unwrap()
        );
    }

    #[test]
    fn config_parses_supported_position() {
        let config = parse_config(
            r#"{"position":"top-right","offsetLeft":12,"offsetRight":18,"borderWidth":2}"#,
        )
        .unwrap();
        assert_eq!(config.position, NotificationPosition::TopRight);
        assert_eq!(config.offset_left, 12);
        assert_eq!(config.offset_right, 18);
        assert_eq!(config.border_width, 2);
    }

    #[test]
    fn config_parses_custom_sound_path() {
        let config = parse_config(r#"{"soundPath":"C:\\sounds\\notify.mp3"}"#).unwrap();
        assert_eq!(
            config.sound_path,
            Some(PathBuf::from(r"C:\sounds\notify.mp3"))
        );
    }

    #[test]
    fn bottom_content_region_stays_anchored_to_fixed_canvas() {
        assert_eq!(
            (0, 560, 430, 760),
            content_region_bounds(NotificationPosition::BottomLeft, 430, 760, 200)
        );
        assert_eq!(
            (0, 0, 430, 200),
            content_region_bounds(NotificationPosition::TopRight, 430, 760, 200)
        );
        assert_eq!(
            (0, 0, 430, 760),
            content_region_bounds(NotificationPosition::BottomRight, 430, 760, 900)
        );
    }

    #[test]
    fn window_position_uses_work_area_and_scale() {
        assert_eq!(
            (12, 630),
            window_position(
                NotificationPosition::BottomLeft,
                0,
                0,
                1920,
                1080,
                430.0,
                280.0,
                1.5,
                12,
                18
            )
        );
        assert_eq!(
            (1257, 30),
            window_position(
                NotificationPosition::TopRight,
                0,
                0,
                1920,
                1080,
                430.0,
                280.0,
                1.5,
                12,
                18
            )
        );
    }
}

#[cfg(target_os = "windows")]
fn hide_from_taskbar(app: &AppHandle) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                let hwnd = hwnd.0 as HWND;
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    (ex_style & !(WS_EX_APPWINDOW as isize)) | WS_EX_TOOLWINDOW as isize,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_from_taskbar(_: &AppHandle) {}

#[cfg(all(test, target_os = "windows"))]
mod daemon_lock_tests {
    use super::acquire_windows_daemon_lock;

    #[test]
    fn named_mutex_allows_only_one_daemon_owner() {
        let name = format!("Local\\nonstop-notify-test-{}", std::process::id());
        let first = acquire_windows_daemon_lock(&name).unwrap().unwrap();
        assert!(acquire_windows_daemon_lock(&name).unwrap().is_none());
        drop(first);
        assert!(acquire_windows_daemon_lock(&name).unwrap().is_some());
    }
}

#[cfg(test)]
mod notification_sound_tests {
    use super::*;

    fn temp_sound_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nonstop-notify-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn adjacent_runtime_notification_sound_is_preferred() {
        let directory = temp_sound_dir("runtime-sound");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("notify-ring.mp3"), NOTIFICATION_SOUND).unwrap();

        let sound = runtime_notification_sound_from_dir(Some(&directory));

        assert_eq!(sound.as_deref(), Some(NOTIFICATION_SOUND));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_adjacent_runtime_notification_sound_falls_back() {
        let directory = temp_sound_dir("invalid-runtime-sound");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("notify-ring.mp3"), b"not an audio file").unwrap();

        let sound = runtime_notification_sound_from_dir(Some(&directory));

        assert!(sound.is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn bundled_samples() -> Vec<i16> {
        NOTIFICATION_SOUND[44..]
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect()
    }

    #[test]
    fn bundled_notification_sound_is_decodable() {
        assert!(validate_notification_sound(None).is_ok());
    }

    #[test]
    fn bundled_notification_sound_is_soft_and_low() {
        let samples = bundled_samples();
        let peak = samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap();
        let zero_crossings = samples
            .windows(2)
            .filter(|pair| (pair[0] < 0 && pair[1] >= 0) || (pair[0] >= 0 && pair[1] < 0))
            .count();
        let duration_seconds = samples.len() as f32 / 44_100.0;
        let average_frequency = zero_crossings as f32 / duration_seconds / 2.0;

        assert!(peak <= 4_100, "peak amplitude is too loud: {peak}");
        assert!(
            average_frequency < 650.0,
            "average frequency is too high: {average_frequency} Hz"
        );
    }

    #[test]
    fn custom_notification_sound_is_decodable() {
        let path = Path::new(concat!(env!("OUT_DIR"), "/notification.wav"));
        assert!(validate_notification_sound(Some(path)).is_ok());
    }

    #[test]
    fn missing_custom_notification_sound_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "nonstop-notify-missing-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!path.exists());
        let error = validate_notification_sound(Some(&path)).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
    }
}
