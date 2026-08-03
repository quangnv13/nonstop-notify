use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use fs2::FileExt;
use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};

const SOCKET_NAME: &str = "nonstop-notify.sock";
const CONTROL_SOCKET_NAME: &str = "nonstop-notify-control.sock";

fn finish_queue_operation<T>(
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

pub fn listen_control<F>(on_stop: F) -> io::Result<()>
where
    F: Fn() + Send + 'static,
{
    let name = CONTROL_SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    for stream in listener.incoming() {
        let stream = stream?;
        let mut reader = BufReader::new(stream);
        let mut command = String::new();
        reader.read_line(&mut command)?;
        let stream = reader.get_mut();
        if is_stop_command(&command) {
            stream.write_all(b"{\"ok\":true}\n")?;
            stream.flush()?;
            on_stop();
        } else {
            stream.write_all(b"{\"ok\":false,\"error\":\"unknown command\"}\n")?;
            stream.flush()?;
        }
    }
    Ok(())
}

fn is_stop_command(command: &str) -> bool {
    command.trim() == "stop"
}

pub fn send_stop() -> io::Result<()> {
    let name = CONTROL_SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
    let mut stream = Stream::connect(name)?;
    stream.write_all(b"stop\n")?;
    stream.flush()?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    if response.trim() == "{\"ok\":true}" {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            response.trim().to_string(),
        ))
    }
}

#[cfg(test)]
mod control_tests {
    use super::is_stop_command;

    #[test]
    fn stop_command_contract_is_single_word() {
        assert!(is_stop_command("stop\n"));
        assert!(!is_stop_command("stop-now\n"));
        assert!(!is_stop_command("stop {\"force\":true}\n"));
    }
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
        .read(true)
        .append(true)
        .open(queue_path())?;
    file.lock_exclusive()?;
    let result = (|| {
        let line = format!("{}\n", input.trim());
        file.write_all(line.as_bytes())?;
        file.flush()
    })();
    let unlock_result = file.unlock();
    finish_queue_operation(result, unlock_result)
}

pub fn drain_queued_events() -> io::Result<Vec<String>> {
    let path = queue_path();
    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    file.lock_exclusive()?;
    let result = (|| {
        file.seek(SeekFrom::Start(0))?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        file.set_len(0)?;
        file.flush()?;
        Ok(input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    })();
    let unlock_result = file.unlock();
    finish_queue_operation(result, unlock_result)
}

#[cfg(test)]
mod tests {
    use super::{append_event_json, drain_queued_events};
    use fs2::FileExt;
    use std::fs::OpenOptions;
    use std::sync::{mpsc, Mutex, OnceLock};
    use std::thread;
    use std::time::Duration;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn reset_queue() {
        let _ = std::fs::remove_file(super::queue_path());
    }

    #[test]
    fn append_waits_for_an_existing_queue_lock() {
        let _test_guard = test_guard();
        reset_queue();
        append_event_json(r#"{"event":"seed"}"#).unwrap();
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(super::queue_path())
            .unwrap();
        lock_file.lock_exclusive().unwrap();

        let (done_tx, done_rx) = mpsc::channel();
        let writer = thread::spawn(move || {
            done_tx
                .send(append_event_json(r#"{"event":"queued"}"#))
                .unwrap();
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());

        lock_file.unlock().unwrap();
        assert!(done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_ok());
        writer.join().unwrap();
        assert_eq!(drain_queued_events().unwrap().len(), 2);
        reset_queue();
    }

    #[test]
    fn drain_waits_for_an_existing_queue_lock() {
        let _test_guard = test_guard();
        reset_queue();
        append_event_json(r#"{"event":"queued"}"#).unwrap();
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(super::queue_path())
            .unwrap();
        lock_file.lock_exclusive().unwrap();

        let (done_tx, done_rx) = mpsc::channel();
        let drainer = thread::spawn(move || {
            done_tx.send(drain_queued_events()).unwrap();
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());

        lock_file.unlock().unwrap();
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        drainer.join().unwrap();
        reset_queue();
    }
}
