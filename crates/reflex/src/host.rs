use crate::lua::ErrorKind;
use crate::lua::LuaError;
pub use reflex_core::MouseMoveMode;
use std::env;
use std::ffi::OsStr;
use std::io::{ErrorKind as IoErrorKind, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

#[derive(Clone)]
pub struct Host {
    pub name: &'static str,
    pub remapping: Arc<dyn Remapper>,
    pub input: Arc<dyn InputController>,
    pub process: Arc<dyn ProcessController>,
    pub clipboard: Arc<dyn ClipboardController>,
}

pub trait Remapper: Send + Sync {
    fn name(&self) -> &'static str;
    fn register_bind(&self, combo: &str) -> Result<(), LuaError>;
    fn remap_key(&self, from: &str, to: &str) -> Result<(), LuaError>;
    fn drain_bind_events(&self) -> Result<Vec<String>, LuaError> {
        Ok(Vec::new())
    }
}

pub trait InputController: Send + Sync {
    fn name(&self) -> &'static str;
    fn key_send(&self, text: &str) -> Result<(), LuaError>;
    fn key_tap(&self, combo: &str) -> Result<(), LuaError>;
    fn key_down(&self, key: &str) -> Result<(), LuaError>;
    fn key_up(&self, key: &str) -> Result<(), LuaError>;
    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> Result<(), LuaError>;
    fn mouse_click(&self, button: &str, x: Option<i32>, y: Option<i32>) -> Result<(), LuaError>;
    fn mouse_down(&self, button: &str) -> Result<(), LuaError>;
    fn mouse_up(&self, button: &str) -> Result<(), LuaError>;
    fn mouse_scroll(&self, delta: i32) -> Result<(), LuaError>;
}

pub trait ProcessController: Send + Sync {
    fn name(&self) -> &'static str;
    fn spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError>;
    fn find(&self, name: &str) -> Result<Option<u32>, LuaError>;
    fn kill(&self, pid: u32) -> Result<(), LuaError>;
    fn pkill(&self, name: &str) -> Result<u32, LuaError>;
}

pub trait ClipboardController: Send + Sync {
    fn name(&self) -> &'static str;
    fn get(&self) -> Result<String, LuaError>;
    fn set(&self, text: &str) -> Result<(), LuaError>;
    fn clear(&self) -> Result<(), LuaError>;
}

pub fn default_host() -> Host {
    unsupported_host()
}

pub fn unsupported_host() -> Host {
    host("unsupported")
}

pub fn daemon_host() -> Result<Host, LuaError> {
    let daemon = Arc::new(crate::daemon::client::DaemonHost::connect_default()?);
    Ok(Host {
        name: "reflexd",
        remapping: daemon.clone(),
        input: daemon.clone(),
        process: Arc::new(LocalProcessController),
        clipboard: Arc::new(CommandClipboard),
    })
}

fn host(name: &'static str) -> Host {
    let unsupported = Arc::new(UnsupportedController { host: name });
    Host {
        name,
        remapping: unsupported.clone(),
        input: unsupported.clone(),
        process: Arc::new(LocalProcessController),
        clipboard: Arc::new(CommandClipboard),
    }
}

struct UnsupportedController {
    host: &'static str,
}

impl UnsupportedController {
    fn unsupported(&self, operation: &str) -> LuaError {
        LuaError::unsupported_for_host(operation, self.host)
    }
}

impl Remapper for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn register_bind(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.bind"))
    }

    fn remap_key(&self, _: &str, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.hotkey"))
    }
}

impl InputController for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn key_send(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.type"))
    }

    fn key_tap(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.send"))
    }

    fn key_down(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.down"))
    }

    fn key_up(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.up"))
    }

    fn mouse_move(&self, _: i32, _: i32, _: MouseMoveMode) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.move"))
    }

    fn mouse_click(&self, _: &str, _: Option<i32>, _: Option<i32>) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.click"))
    }

    fn mouse_down(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.down"))
    }

    fn mouse_up(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.up"))
    }

    fn mouse_scroll(&self, _: i32) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.scroll"))
    }
}

impl ProcessController for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn spawn(&self, _: &str, _: &[String]) -> Result<u32, LuaError> {
        Err(self.unsupported("reflex.process.spawn"))
    }

    fn find(&self, _: &str) -> Result<Option<u32>, LuaError> {
        Err(self.unsupported("reflex.process.find"))
    }

    fn kill(&self, _: u32) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.process.kill"))
    }

    fn pkill(&self, _: &str) -> Result<u32, LuaError> {
        Err(self.unsupported("reflex.process.pkill"))
    }
}

struct LocalProcessController;

impl ProcessController for LocalProcessController {
    fn name(&self) -> &'static str {
        "local"
    }

    fn spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError> {
        Command::new(program)
            .args(args)
            .spawn()
            .map(|child| child.id())
            .map_err(|err| process_err(format!("failed to spawn {program}: {err}")))
    }

    fn find(&self, name: &str) -> Result<Option<u32>, LuaError> {
        Ok(find_processes(name, "reflex.process.find")?
            .into_iter()
            .next())
    }

    fn kill(&self, pid: u32) -> Result<(), LuaError> {
        kill_pid(pid)
    }

    fn pkill(&self, name: &str) -> Result<u32, LuaError> {
        let pids = find_processes(name, "reflex.process.pkill")?;
        let mut killed = 0;
        for pid in pids {
            kill_pid(pid)?;
            killed += 1;
        }
        Ok(killed)
    }
}

fn find_processes(name: &str, operation: &str) -> Result<Vec<u32>, LuaError> {
    let query = name.trim();
    if query.is_empty() {
        return Err(LuaError::new(
            ErrorKind::Runtime,
            format!("{operation} requires a process name"),
        ));
    }

    let mut pids = Vec::new();
    let entries = std::fs::read_dir("/proc")
        .map_err(|err| process_err(format!("failed to read /proc: {err}")))?;

    for entry in entries {
        let entry =
            entry.map_err(|err| process_err(format!("failed to read /proc entry: {err}")))?;
        let file_name = entry.file_name();
        let Some(pid) = parse_pid(&file_name) else {
            continue;
        };
        if process_matches(pid, query) {
            pids.push(pid);
        }
    }

    pids.sort_unstable();
    Ok(pids)
}

fn parse_pid(file_name: &OsStr) -> Option<u32> {
    file_name.to_str()?.parse().ok()
}

fn process_matches(pid: u32, query: &str) -> bool {
    read_process_comm(pid)
        .as_deref()
        .is_some_and(|comm| comm == query)
        || read_process_cmdline(pid)
            .iter()
            .any(|arg| process_arg_matches(arg, query))
}

fn read_process_comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn read_process_cmdline(pid: u32) -> Vec<String> {
    let Ok(bytes) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return Vec::new();
    };

    bytes
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .filter_map(|arg| String::from_utf8(arg.to_vec()).ok())
        .collect()
}

fn process_arg_matches(arg: &str, query: &str) -> bool {
    arg == query
        || PathBuf::from(arg)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == query)
}

fn kill_pid(pid: u32) -> Result<(), LuaError> {
    let output = Command::new("kill")
        .arg(pid.to_string())
        .output()
        .map_err(|err| match err.kind() {
            IoErrorKind::NotFound => process_err("failed to run kill: command not found"),
            _ => process_err(format!("failed to run kill: {err}")),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(command_failed("kill", &output.stderr))
    }
}

fn process_err(message: impl Into<String>) -> LuaError {
    LuaError::new(ErrorKind::Host, message)
}

struct CommandClipboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBackend {
    WlClipboard,
    Xclip,
    Xsel,
}

impl ClipboardBackend {
    fn command(self) -> &'static str {
        match self {
            Self::WlClipboard => "wl-copy",
            Self::Xclip => "xclip",
            Self::Xsel => "xsel",
        }
    }

    fn get_command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::WlClipboard => ("wl-paste", &[]),
            Self::Xclip => ("xclip", &["-selection", "clipboard", "-out"]),
            Self::Xsel => ("xsel", &["--clipboard", "--output"]),
        }
    }

    fn set_command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::WlClipboard => ("wl-copy", &[]),
            Self::Xclip => ("xclip", &["-selection", "clipboard"]),
            Self::Xsel => ("xsel", &["--clipboard", "--input"]),
        }
    }
}

impl ClipboardController for CommandClipboard {
    fn name(&self) -> &'static str {
        "command"
    }

    fn get(&self) -> Result<String, LuaError> {
        let backend = detect_clipboard_backend()?;
        let (program, args) = backend.get_command();
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|err| clipboard_err(format!("failed to run {program}: {err}")))?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|err| {
                clipboard_err(format!(
                    "{program} returned non-UTF-8 clipboard data: {err}"
                ))
            })
        } else {
            Err(command_failed(program, &output.stderr))
        }
    }

    fn set(&self, text: &str) -> Result<(), LuaError> {
        let backend = detect_clipboard_backend()?;
        let (program, args) = backend.set_command();
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| clipboard_err(format!("failed to run {program}: {err}")))?;

        child
            .stdin
            .as_mut()
            .expect("clipboard command stdin should be piped")
            .write_all(text.as_bytes())
            .map_err(|err| {
                clipboard_err(format!(
                    "failed to write clipboard data to {program}: {err}"
                ))
            })?;

        let output = child
            .wait_with_output()
            .map_err(|err| clipboard_err(format!("failed to wait for {program}: {err}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(command_failed(program, &output.stderr))
        }
    }

    fn clear(&self) -> Result<(), LuaError> {
        self.set("")
    }
}

fn detect_clipboard_backend() -> Result<ClipboardBackend, LuaError> {
    select_clipboard_backend(
        env::var_os("WAYLAND_DISPLAY").is_some(),
        env::var_os("DISPLAY").is_some(),
        command_exists,
    )
    .ok_or_else(|| {
        clipboard_err("no supported clipboard command found; install wl-clipboard, xclip, or xsel")
    })
}

fn select_clipboard_backend(
    wayland: bool,
    x11: bool,
    command_exists: impl Fn(&str) -> bool,
) -> Option<ClipboardBackend> {
    let candidates: &[ClipboardBackend] = match (wayland, x11) {
        (true, true) => &[
            ClipboardBackend::WlClipboard,
            ClipboardBackend::Xclip,
            ClipboardBackend::Xsel,
        ],
        (true, false) => &[
            ClipboardBackend::WlClipboard,
            ClipboardBackend::Xclip,
            ClipboardBackend::Xsel,
        ],
        (false, true) => &[
            ClipboardBackend::Xclip,
            ClipboardBackend::Xsel,
            ClipboardBackend::WlClipboard,
        ],
        (false, false) => &[
            ClipboardBackend::WlClipboard,
            ClipboardBackend::Xclip,
            ClipboardBackend::Xsel,
        ],
    };

    candidates.iter().copied().find(|backend| match backend {
        ClipboardBackend::WlClipboard => {
            command_exists(ClipboardBackend::WlClipboard.command()) && command_exists("wl-paste")
        }
        backend => command_exists(backend.command()),
    })
}

fn command_exists(program: &str) -> bool {
    let path = PathBuf::from(program);
    if path.components().count() > 1 {
        return path.is_file();
    }

    env::var_os("PATH")
        .is_some_and(|path| env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

fn command_failed(program: &str, stderr: &[u8]) -> LuaError {
    let stderr = String::from_utf8_lossy(stderr);
    let detail = stderr.trim();
    if detail.is_empty() {
        host_err(format!("{program} failed"))
    } else {
        host_err(format!("{program} failed: {detail}"))
    }
}

fn host_err(message: impl Into<String>) -> LuaError {
    LuaError::new(ErrorKind::Host, message)
}

fn clipboard_err(message: impl Into<String>) -> LuaError {
    host_err(message)
}

#[cfg(test)]
mod tests {
    use super::{
        ClipboardBackend, ErrorKind, find_processes, process_arg_matches, select_clipboard_backend,
    };

    #[test]
    fn prefers_wayland_when_available() {
        let backend = select_clipboard_backend(true, true, |command| {
            matches!(command, "wl-copy" | "wl-paste" | "xclip")
        });
        assert_eq!(backend, Some(ClipboardBackend::WlClipboard));
    }

    #[test]
    fn falls_back_to_xclip_when_wayland_tools_are_missing() {
        let backend = select_clipboard_backend(true, true, |command| command == "xclip");
        assert_eq!(backend, Some(ClipboardBackend::Xclip));
    }

    #[test]
    fn prefers_x11_tools_on_x11_sessions() {
        let backend = select_clipboard_backend(false, true, |command| {
            matches!(command, "wl-copy" | "wl-paste" | "xsel")
        });
        assert_eq!(backend, Some(ClipboardBackend::Xsel));
    }

    #[test]
    fn returns_none_without_supported_commands() {
        let backend = select_clipboard_backend(true, true, |_| false);
        assert_eq!(backend, None);
    }

    #[test]
    fn process_arg_matches_exact_or_basename_only() {
        assert!(process_arg_matches("/usr/bin/kitty", "kitty"));
        assert!(process_arg_matches("firefox", "firefox"));
        assert!(!process_arg_matches("/usr/bin/firefox-helper", "firefox"));
        assert!(!process_arg_matches("--class=firefox", "firefox"));
    }

    #[test]
    fn process_lookup_rejects_empty_names() {
        let err = find_processes(" ", "reflex.process.find").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Runtime);
        assert_eq!(err.msg, "reflex.process.find requires a process name");
    }
}
