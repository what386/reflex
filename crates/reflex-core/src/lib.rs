pub mod protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMoveMode {
    Absolute,
    Relative,
}

pub const SOCKET_ENV: &str = "REFLEXD_SOCKET";

pub fn default_socket_path() -> Result<std::path::PathBuf, String> {
    Ok(std::path::PathBuf::from("/run/reflexd.sock"))
}
