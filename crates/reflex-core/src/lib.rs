pub mod protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMoveMode {
    Absolute,
    Relative,
}

pub const SOCKET_ENV: &str = "REFLEXD_SOCKET";

pub fn default_socket_path() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os(SOCKET_ENV) {
        return Ok(std::path::PathBuf::from(path));
    }

    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return Ok(std::path::PathBuf::from(runtime_dir).join("reflexd.sock"));
    }

    Err(format!(
        "{SOCKET_ENV} is not set and XDG_RUNTIME_DIR is unavailable"
    ))
}
