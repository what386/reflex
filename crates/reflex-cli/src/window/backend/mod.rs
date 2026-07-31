mod gnome;
mod wayland;
mod x11;

use super::WindowStore;
use super::store::backend_error;
use crate::lua::LuaError;
use std::env;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendChoice {
    X11,
    Wayland,
    Ext,
    Wlr,
    Kde,
    Gnome,
}

pub(super) fn start(store: Arc<WindowStore>) -> Result<(), LuaError> {
    match select_backend(
        env::var("REFLEX_WINDOW_BACKEND").ok().as_deref(),
        env::var_os("WAYLAND_DISPLAY").is_some(),
        env::var_os("DISPLAY").is_some(),
    )? {
        BackendChoice::X11 => x11::start(store),
        BackendChoice::Gnome => gnome::start(store),
        BackendChoice::Wayland => match wayland::start(store.clone(), wayland::Mode::Auto) {
            Err(error) if error.kind == crate::lua::ErrorKind::Unsupported => gnome::start(store)
                .map_err(|gnome_error| {
                    backend_error(format!(
                        "{}; GNOME fallback also failed: {}",
                        error.msg, gnome_error.msg
                    ))
                }),
            result => result,
        },
        BackendChoice::Ext => wayland::start(store, wayland::Mode::Ext),
        BackendChoice::Wlr => wayland::start(store, wayland::Mode::Wlr),
        BackendChoice::Kde => wayland::start(store, wayland::Mode::Kde),
    }
}

fn select_backend(
    requested: Option<&str>,
    wayland: bool,
    x11: bool,
) -> Result<BackendChoice, LuaError> {
    match requested
        .unwrap_or("auto")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "auto" if wayland => Ok(BackendChoice::Wayland),
        "auto" if x11 => Ok(BackendChoice::X11),
        "auto" => Err(backend_error(
            "reflex.window requires WAYLAND_DISPLAY or DISPLAY",
        )),
        "wayland" => Ok(BackendChoice::Wayland),
        "x11" => Ok(BackendChoice::X11),
        "ext" => Ok(BackendChoice::Ext),
        "wlr" => Ok(BackendChoice::Wlr),
        "kde" => Ok(BackendChoice::Kde),
        "gnome" => Ok(BackendChoice::Gnome),
        other => Err(backend_error(format!(
            "invalid REFLEX_WINDOW_BACKEND {other:?}; expected auto, wayland, x11, ext, wlr, kde, or gnome"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_wayland_over_x11() {
        assert_eq!(
            select_backend(None, true, true).unwrap(),
            BackendChoice::Wayland
        );
        assert_eq!(
            select_backend(None, false, true).unwrap(),
            BackendChoice::X11
        );
    }

    #[test]
    fn auto_does_not_select_x11_without_a_display() {
        assert!(select_backend(None, false, false).is_err());
    }

    #[test]
    fn explicit_backends_override_session_detection() {
        assert_eq!(
            select_backend(Some("gnome"), false, false).unwrap(),
            BackendChoice::Gnome
        );
        assert_eq!(
            select_backend(Some("x11"), true, false).unwrap(),
            BackendChoice::X11
        );
    }
}
