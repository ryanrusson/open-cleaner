//! Installing drivers needs administrator rights; scanning does not. Rather
//! than run the whole app elevated, the app relaunches its own exe elevated
//! (one UAC prompt) as a windowless helper and reads progress from it over a
//! named pipe as JSON lines.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::io::FromRawHandle;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_CANCELLED, ERROR_PIPE_LISTENING, HANDLE, WAIT_TIMEOUT,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::install;
use crate::models::InstallEvent;
use crate::INSTALL_HELPER_FLAG;

const PIPE_PREFIX: &str = r"\\.\pipe\driver-updater-";

/// Update IDs are GUIDs. Anything else never reaches a command line.
fn is_update_id(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// What may be passed to a helper on its command line: plain identifiers.
fn is_plain_arg(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn create_pipe() -> Result<(String, File), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!("{PIPE_PREFIX}{}-{nonce:x}", std::process::id());
    let handle = unsafe {
        CreateNamedPipeW(
            &HSTRING::from(&name),
            // FIRST_PIPE_INSTANCE: fail rather than join a pipe someone else
            // already created under this name.
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            64 * 1024,
            0,
            None,
        )
    };
    if handle.is_invalid() {
        return Err(format!("Could not set up the installer channel: {}", std::io::Error::last_os_error()));
    }
    Ok((name, unsafe { File::from_raw_handle(handle.0 as _) }))
}

/// Read lines until the helper closes its end. `helper_running` is asked
/// while no helper has connected yet, so a helper that dies early is noticed.
/// `on_line` may stop the run by returning an error.
fn read_lines(
    mut pipe: File,
    mut helper_running: impl FnMut() -> bool,
    on_line: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = match pipe.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.raw_os_error() == Some(ERROR_BROKEN_PIPE.0 as i32) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_LISTENING.0 as i32) => {
                if !helper_running() {
                    return Err("The helper closed before it could start.".into());
                }
                continue;
            }
            Err(e) => return Err(format!("Lost contact with the helper: {e}")),
        };
        pending.extend_from_slice(&chunk[..n]);
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            on_line(&line)?;
        }
    }
}

/// Turn raw helper lines into install events, noting whether it finished.
fn relay_event(
    line: &[u8],
    finished: &mut bool,
    on_event: &mut dyn FnMut(InstallEvent),
) -> Result<(), String> {
    match serde_json::from_slice::<InstallEvent>(line) {
        Ok(InstallEvent::Error { message }) => Err(message),
        Ok(event) => {
            *finished |= matches!(event, InstallEvent::Finished { .. });
            on_event(event);
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

const STOPPED_EARLY: &str =
    "The installer stopped unexpectedly. Run a new scan to see what was installed.";

struct Process(HANDLE);

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Relaunch this exe elevated as a windowless helper (one UAC prompt) with
/// `flag` and `args`, and hand each line it reports to `on_line`.
pub fn run_elevated(
    flag: &str,
    args: &[String],
    on_line: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if !flag.starts_with("--") || !is_plain_arg(&flag[2..]) || !args.iter().all(|a| is_plain_arg(a)) {
        return Err("Refusing to start the helper with unexpected arguments.".into());
    }
    let (pipe_name, pipe) = create_pipe()?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(format!("{flag} {pipe_name} {}", args.join(" ")));

    let mut launch = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        // "runas" is what raises the UAC prompt.
        lpVerb: w!("runas"),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut launch) }.map_err(|e| {
        if e.code() == ERROR_CANCELLED.to_hresult() {
            "Administrator permission was not granted.".to_string()
        } else {
            format!("Could not start the helper: {e}")
        }
    })?;
    let process = Process(launch.hProcess);

    read_lines(pipe, || unsafe { WaitForSingleObject(process.0, 200) } == WAIT_TIMEOUT, on_line)
}

/// App side of a driver install: launch the helper and relay its progress.
pub fn install_elevated(
    ids: &[String],
    on_event: &mut dyn FnMut(InstallEvent),
) -> Result<(), String> {
    if ids.is_empty() || !ids.iter().all(|id| is_update_id(id)) {
        return Err("No valid updates were selected.".into());
    }
    let mut finished = false;
    run_elevated(INSTALL_HELPER_FLAG, ids, &mut |line| relay_event(line, &mut finished, on_event))?;
    if finished {
        Ok(())
    } else {
        Err(STOPPED_EARLY.into())
    }
}

/// Helper side: connect back to the app that launched us.
pub fn open_pipe(pipe_name: &str) -> Option<File> {
    if !pipe_name.starts_with(PIPE_PREFIX) {
        return None;
    }
    OpenOptions::new().write(true).open(pipe_name).ok()
}

/// Helper side: `args` are everything after [`INSTALL_HELPER_FLAG`].
/// Returns the process exit code.
pub fn run_helper(args: &[String]) -> i32 {
    let Some((pipe_name, ids)) = args.split_first() else { return 2 };
    if ids.is_empty() || !ids.iter().all(|id| is_update_id(id)) {
        return 2;
    }
    let Some(mut pipe) = open_pipe(pipe_name) else { return 3 };
    let mut emit = |event: InstallEvent| {
        if let Ok(mut line) = serde_json::to_vec(&event) {
            line.push(b'\n');
            let _ = pipe.write_all(&line).and_then(|_| pipe.flush());
        }
    };
    match install::run(ids, &mut emit) {
        Ok(()) => 0,
        Err(message) => {
            emit(InstallEvent::Error { message });
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::InstallStage;

    fn send(pipe_name: String, events: Vec<InstallEvent>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let mut pipe = OpenOptions::new().write(true).open(pipe_name).unwrap();
            for event in events {
                let mut line = serde_json::to_vec(&event).unwrap();
                line.push(b'\n');
                pipe.write_all(&line).unwrap();
            }
        })
    }

    fn wait_briefly() -> bool {
        std::thread::sleep(std::time::Duration::from_millis(50));
        true
    }

    /// What `install_elevated` does once the helper is running.
    fn relay(pipe: File, running: impl FnMut() -> bool) -> Result<Vec<InstallEvent>, String> {
        let mut got = Vec::new();
        let mut finished = false;
        read_lines(pipe, running, &mut |line| {
            relay_event(line, &mut finished, &mut |e| got.push(e))
        })?;
        if finished {
            Ok(got)
        } else {
            Err(STOPPED_EARLY.into())
        }
    }

    #[test]
    fn relays_events_from_a_late_connecting_helper() {
        let (name, pipe) = create_pipe().unwrap();
        let sent = vec![
            InstallEvent::Update {
                update_id: "a".into(),
                stage: InstallStage::Installed,
                detail: String::new(),
            },
            InstallEvent::Finished { installed: 1, failed: 0, reboot_required: true },
        ];
        let helper = send(name, sent.clone());
        assert_eq!(relay(pipe, wait_briefly).unwrap(), sent);
        helper.join().unwrap();
    }

    #[test]
    fn helper_error_or_early_exit_is_an_error() {
        let (name, pipe) = create_pipe().unwrap();
        let helper = send(name, vec![InstallEvent::Error { message: "boom".into() }]);
        assert_eq!(relay(pipe, wait_briefly).unwrap_err(), "boom");
        helper.join().unwrap();

        // Helper connects and vanishes without finishing.
        let (name, pipe) = create_pipe().unwrap();
        let helper = send(name, vec![]);
        assert!(relay(pipe, wait_briefly).is_err());
        helper.join().unwrap();

        // Helper never connects at all.
        let (_name, pipe) = create_pipe().unwrap();
        assert!(relay(pipe, || false).is_err());
    }

    #[test]
    fn only_guids_count_as_update_ids() {
        assert!(is_update_id("3f2504e0-4f89-11d3-9a0c-0305e82c3301"));
        assert!(!is_update_id("3f2504e0-4f89-11d3-9a0c-0305e82c3301 & calc"));
        assert!(!is_update_id(""));
        assert!(is_plain_arg("windows-temp"));
        assert!(!is_plain_arg("a b"));
        assert!(!is_plain_arg("a\"b"));
        assert!(!is_plain_arg(""));
    }
}
