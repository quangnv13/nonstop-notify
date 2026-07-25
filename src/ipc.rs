use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions};

const SOCKET_NAME: &str = "nonstop-notify.sock";

pub fn listen_events<F>(mut on_event: F) -> io::Result<()>
where
    F: FnMut(String) + Send + 'static,
{
    let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut input = String::new();
        stream.read_to_string(&mut input)?;
        for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
            on_event(line.to_string());
        }
    }
    Ok(())
}

pub fn spawn_daemon(config_path: Option<&Path>) -> io::Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("daemon");
    if let Some(path) = config_path {
        command.arg("--config").arg(path);
    }
    command
        .env("NONSTOP_NOTIFY_DAEMONIZED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut command);
    command.spawn()
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000 | 0x0000_0008);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_: &mut Command) {}

fn queue_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("nonstop-notify-events.jsonl");
    path
}

pub fn append_event_json(input: &str) -> io::Result<()> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(queue_path())?;
    file.write_all(input.trim().as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}

pub fn drain_queued_events() -> io::Result<Vec<String>> {
    let path = queue_path();
    let input = match std::fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let _ = std::fs::remove_file(&path);
    Ok(input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}
