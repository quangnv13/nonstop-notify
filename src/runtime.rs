use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const HEARTBEAT_STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationPosition {
    TopLeft,
    TopRight,
    #[default]
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct NotifyConfig {
    pub position: NotificationPosition,
    pub offset_left: u16,
    pub offset_right: u16,
    pub border_width: u16,
    pub sound_path: Option<PathBuf>,
    pub log_rotation_hours: u64,
    pub log_retention_days: u64,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            position: NotificationPosition::default(),
            offset_left: 30,
            offset_right: 30,
            border_width: 1,
            sound_path: None,
            log_rotation_hours: 24,
            log_retention_days: 7,
        }
    }
}

pub fn parse_config(input: &str) -> Result<NotifyConfig, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn validate_config(config: &NotifyConfig) -> io::Result<()> {
    if config.log_rotation_hours == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "logRotationHours must be a positive integer",
        ));
    }
    if config.log_retention_days == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "logRetentionDays must be a positive integer",
        ));
    }
    Ok(())
}

pub fn default_data_dir() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|path| path.join("AppData").join("Local"))
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
        return Ok(base.join("Nonstop Notify"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        Ok(base.join("nonstop-notify"))
    }
}

pub fn default_config_path() -> io::Result<PathBuf> {
    Ok(default_data_dir()?.join("config.json"))
}

pub fn selected_config_path(explicit: Option<&Path>) -> io::Result<PathBuf> {
    match explicit {
        Some(path) => Ok(path.to_path_buf()),
        None => default_config_path(),
    }
}

pub fn load_config(
    explicit: Option<&Path>,
) -> Result<(NotifyConfig, PathBuf), Box<dyn std::error::Error>> {
    let path = selected_config_path(explicit)?;
    load_config_file(&path, explicit.is_none())
}

fn load_config_file(
    path: &Path,
    missing_defaults: bool,
) -> Result<(NotifyConfig, PathBuf), Box<dyn std::error::Error>> {
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if missing_defaults && error.kind() == io::ErrorKind::NotFound => {
            return Ok((NotifyConfig::default(), path.to_path_buf()));
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("failed to read notify config {}: {error}", path.display()),
            )
            .into())
        }
    };
    let config = parse_config(&input).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid notify config {}: {error}", path.display()),
        )
    })?;
    validate_config(&config)?;
    Ok((config, path.to_path_buf()))
}

pub fn set_config_value(config: &mut NotifyConfig, key: &str, value: &str) -> io::Result<()> {
    match key {
        "position" => {
            config.position = match value {
                "top-left" => NotificationPosition::TopLeft,
                "top-right" => NotificationPosition::TopRight,
                "bottom-left" => NotificationPosition::BottomLeft,
                "bottom-right" => NotificationPosition::BottomRight,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "position must be top-left, top-right, bottom-left, or bottom-right",
                    ))
                }
            };
        }
        "offsetLeft" => config.offset_left = parse_u16(key, value)?,
        "offsetRight" => config.offset_right = parse_u16(key, value)?,
        "borderWidth" => config.border_width = parse_u16(key, value)?,
        "soundPath" => {
            config.sound_path = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
        "logRotationHours" => config.log_rotation_hours = parse_positive_u64(key, value)?,
        "logRetentionDays" => config.log_retention_days = parse_positive_u64(key, value)?,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported config key: {key}"),
            ))
        }
    }
    validate_config(config)
}

fn parse_u16(key: &str, value: &str) -> io::Result<u16> {
    value.parse::<u16>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{key} must be an unsigned 16-bit integer"),
        )
    })
}

fn parse_positive_u64(key: &str, value: &str) -> io::Result<u64> {
    let parsed = value.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{key} must be a positive integer"),
        )
    })?;
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{key} must be a positive integer"),
        ));
    }
    Ok(parsed)
}

pub fn write_config(path: &Path, config: &NotifyConfig) -> Result<(), Box<dyn std::error::Error>> {
    validate_config(config)?;
    let bytes = serde_json::to_vec_pretty(config)?;
    atomic_write(path, &bytes)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub state_path: PathBuf,
    pub log_dir: PathBuf,
    pub current_log_path: PathBuf,
    pub log_lock_path: PathBuf,
}

impl RuntimePaths {
    pub fn new(data_dir: PathBuf) -> Self {
        let state_path = data_dir.join("runtime.json");
        let log_dir = data_dir.join("logs");
        let current_log_path = log_dir.join("current.log");
        let log_lock_path = log_dir.join(".lock");
        Self {
            state_path,
            log_dir,
            current_log_path,
            log_lock_path,
        }
    }
}

pub fn runtime_paths() -> io::Result<RuntimePaths> {
    Ok(RuntimePaths::new(default_data_dir()?))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub pid: u32,
    pub heartbeat_at_ms: u64,
}

pub fn write_runtime_state(paths: &RuntimePaths, pid: u32) -> io::Result<()> {
    let state = RuntimeState {
        pid,
        heartbeat_at_ms: now_ms(),
    };
    let bytes = serde_json::to_vec(&state).map_err(io::Error::other)?;
    atomic_write(&paths.state_path, &bytes)
}

pub fn heartbeat(paths: &RuntimePaths, pid: u32) -> io::Result<()> {
    write_runtime_state(paths, pid)
}

pub fn read_runtime_state(paths: &RuntimePaths) -> io::Result<Option<RuntimeState>> {
    let mut input = String::new();
    match File::open(&paths.state_path) {
        Ok(mut file) => file.read_to_string(&mut input)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    serde_json::from_str(&input)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn remove_runtime_state(paths: &RuntimePaths) -> io::Result<()> {
    match fs::remove_file(&paths.state_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn heartbeat_age_ms(state: &RuntimeState) -> u64 {
    now_ms().saturating_sub(state.heartbeat_at_ms)
}

pub fn append_log(
    paths: &RuntimePaths,
    rotation_hours: u64,
    retention_days: u64,
    entry: &Value,
) -> io::Result<()> {
    fs::create_dir_all(&paths.log_dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&paths.log_lock_path)?;
    lock.lock_exclusive()?;
    let result = (|| {
        rotate_current_log(paths, rotation_hours)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.current_log_path)?;
        serde_json::to_writer(&mut file, entry).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        file.flush()
    })();
    let result = result.and_then(|_| cleanup_logs(paths, retention_days));
    let unlock_result = lock.unlock();
    finish_locked_operation(result, unlock_result)
}

pub fn latest_log(paths: &RuntimePaths) -> io::Result<Option<Vec<u8>>> {
    let entries = match fs::read_dir(&paths.log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|value| value.to_str()) == Some("log")).then_some(path)
        })
        .filter_map(|path| {
            fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .map(|modified| (modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| *modified);
    files.pop().map(|(_, path)| fs::read(path)).transpose()
}

fn rotate_current_log(paths: &RuntimePaths, rotation_hours: u64) -> io::Result<()> {
    let metadata = match fs::metadata(&paths.current_log_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.len() == 0 {
        return Ok(());
    }
    let age = metadata
        .modified()
        .and_then(|modified| modified.elapsed().map_err(io::Error::other))?;
    if age < Duration::from_secs(rotation_hours.saturating_mul(3600)) {
        return Ok(());
    }
    let timestamp = unix_seconds();
    let mut archive = paths.log_dir.join(format!("{timestamp}.log"));
    if archive.exists() {
        archive = paths
            .log_dir
            .join(format!("{timestamp}-{}.log", std::process::id()));
    }
    fs::rename(&paths.current_log_path, archive)
}

fn cleanup_logs(paths: &RuntimePaths, retention_days: u64) -> io::Result<()> {
    let cutoff = Duration::from_secs(retention_days.saturating_mul(24 * 3600));
    let entries = match fs::read_dir(&paths.log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }
        let age = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .and_then(|modified| modified.elapsed().map_err(io::Error::other));
        if age.map(|value| value >= cutoff).unwrap_or(false) {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn finish_locked_operation<T>(
    result: io::Result<T>,
    unlock_result: io::Result<()>,
) -> io::Result<T> {
    match result {
        Ok(value) => unlock_result.map(|_| value),
        Err(error) => {
            let _ = unlock_result;
            Err(error)
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temporary = path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    replace_file(&temporary, path)
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nonstop-notify-runtime-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn defaults_and_config_set_validation() {
        let mut config = NotifyConfig::default();
        assert_eq!(config.log_rotation_hours, 24);
        assert_eq!(config.log_retention_days, 7);
        set_config_value(&mut config, "position", "top-right").unwrap();
        set_config_value(&mut config, "soundPath", "none").unwrap();
        assert_eq!(config.position, NotificationPosition::TopRight);
        assert_eq!(config.sound_path, None);
        assert!(set_config_value(&mut config, "logRotationHours", "0").is_err());
    }

    #[test]
    fn runtime_state_reports_stale_heartbeat() {
        let state = RuntimeState {
            pid: 42,
            heartbeat_at_ms: now_ms().saturating_sub(10_000),
        };
        assert!(heartbeat_age_ms(&state) >= 10_000);
    }

    #[test]
    fn runtime_state_round_trip_and_remove_work() {
        let directory = temp_dir("state");
        let paths = RuntimePaths::new(directory.clone());
        write_runtime_state(&paths, 42).unwrap();
        let state = read_runtime_state(&paths).unwrap().unwrap();
        assert_eq!(state.pid, 42);
        remove_runtime_state(&paths).unwrap();
        assert!(read_runtime_state(&paths).unwrap().is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn config_write_is_atomic_and_preserves_new_fields() {
        let directory = temp_dir("config");
        let path = directory.join("config.json");
        let config = NotifyConfig::default();
        write_config(&path, &config).unwrap();
        let saved = parse_config(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved, config);
        assert!(!directory
            .join(format!(".config.json.tmp-{}", std::process::id()))
            .exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_default_config_falls_back_without_creating_file() {
        let directory = temp_dir("missing-config");
        let path = directory.join("config.json");
        fs::create_dir_all(&directory).unwrap();
        let (config, selected) = load_config_file(&path, true).unwrap();
        assert_eq!(config, NotifyConfig::default());
        assert_eq!(selected, path);
        assert!(!selected.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_config_path_has_precedence() {
        let directory = temp_dir("explicit-config");
        let explicit = directory.join("explicit.json");
        assert_eq!(selected_config_path(Some(&explicit)).unwrap(), explicit);
        fs::create_dir_all(directory).unwrap();
        fs::remove_dir_all(explicit.parent().unwrap()).unwrap();
    }

    #[test]
    fn log_append_and_latest_log_work() {
        let directory = temp_dir("logs");
        let paths = RuntimePaths::new(directory.clone());
        append_log(&paths, 24, 7, &serde_json::json!({"type":"test"})).unwrap();
        let latest = latest_log(&paths).unwrap().unwrap();
        assert_eq!(String::from_utf8(latest).unwrap(), "{\"type\":\"test\"}\n");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn config_write_round_trips_without_leaving_temp_file() {
        let directory = temp_dir("config");
        let path = directory.join("config.json");
        let mut config = NotifyConfig::default();
        config.position = NotificationPosition::BottomRight;
        write_config(&path, &config).unwrap();
        assert_eq!(
            parse_config(&fs::read_to_string(&path).unwrap()).unwrap(),
            config
        );
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rotation_and_retention_use_modified_time() {
        let directory = temp_dir("rotation");
        let paths = RuntimePaths::new(directory.clone());
        append_log(&paths, 24, 7, &serde_json::json!({"type":"old"})).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(25 * 3600);
        OpenOptions::new()
            .write(true)
            .open(&paths.current_log_path)
            .unwrap()
            .set_modified(old_time)
            .unwrap();
        append_log(&paths, 24, 7, &serde_json::json!({"type":"new"})).unwrap();
        assert!(paths
            .log_dir
            .read_dir()
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() != "current.log"));

        let stale = paths.log_dir.join("stale.log");
        fs::write(&stale, b"stale\n").unwrap();
        OpenOptions::new()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(8 * 24 * 3600))
            .unwrap();
        cleanup_logs(&paths, 7).unwrap();
        assert!(!stale.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
