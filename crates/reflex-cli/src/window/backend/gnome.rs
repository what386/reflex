use super::super::store::backend_error;
use super::super::{WindowData, WindowStore};
use crate::lua::LuaError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

const DESTINATION: &str = "org.gnome.Shell.Introspect";
const PATH: &str = "/org/gnome/Shell/Introspect";
const INTERFACE: &str = "org.gnome.Shell.Introspect";

pub(super) fn start(store: Arc<WindowStore>) -> Result<(), LuaError> {
    let connection = Connection::session().map_err(|error| {
        backend_error(format!("failed to connect to the session D-Bus: {error}"))
    })?;
    let initial = fetch_windows(&connection)?;
    store.replace(initial);
    store.mark_ready();

    spawn_poller(connection.clone(), store.clone())?;
    spawn_signal_listener(connection, store);
    Ok(())
}

fn spawn_poller(connection: Connection, store: Arc<WindowStore>) -> Result<(), LuaError> {
    std::thread::Builder::new()
        .name("reflex-window-gnome-poll".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                match fetch_windows(&connection) {
                    Ok(windows) => store.replace(windows),
                    Err(error) => {
                        store.fail(error);
                        break;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| backend_error(format!("failed to spawn GNOME window poller: {error}")))
}

fn spawn_signal_listener(connection: Connection, store: Arc<WindowStore>) {
    let _ = std::thread::Builder::new()
        .name("reflex-window-gnome-signals".into())
        .spawn(move || {
            let Ok(proxy) = introspect_proxy(&connection) else {
                return;
            };
            let Ok(mut signals) = proxy.receive_signal("WindowsChanged") else {
                return;
            };
            while signals.next().is_some() {
                match fetch_windows(&connection) {
                    Ok(windows) => store.replace(windows),
                    Err(error) => {
                        store.fail(error);
                        break;
                    }
                }
            }
        });
}

fn fetch_windows(connection: &Connection) -> Result<Vec<WindowData>, LuaError> {
    let proxy = introspect_proxy(connection)?;
    let windows: HashMap<u64, HashMap<String, OwnedValue>> = proxy
        .call("GetWindows", &())
        .map_err(map_introspect_error)?;
    let mut windows = windows
        .into_iter()
        .map(|(id, properties)| WindowData {
            id: format!("gnome:{id}"),
            title: string_property(&properties, "title").unwrap_or_default(),
            app_id: string_property(&properties, "app-id")
                .or_else(|| string_property(&properties, "wm-class")),
            exists: true,
        })
        .collect::<Vec<_>>();
    windows.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(windows)
}

fn introspect_proxy(connection: &Connection) -> Result<Proxy<'static>, LuaError> {
    Proxy::new_owned(
        connection.clone(),
        DESTINATION.to_string(),
        PATH.to_string(),
        INTERFACE.to_string(),
    )
    .map_err(map_introspect_error)
}

fn string_property(properties: &HashMap<String, OwnedValue>, name: &str) -> Option<String> {
    properties
        .get(name)
        .and_then(|value| <&str>::try_from(value).ok())
        .map(ToOwned::to_owned)
        .filter(|value| !value.is_empty())
}

fn map_introspect_error(error: zbus::Error) -> LuaError {
    let detail = error.to_string();
    if detail.contains("AccessDenied") || detail.contains("not allowed") {
        backend_error(
            "GNOME denied org.gnome.Shell.Introspect.GetWindows; Reflex requires Shell introspection to be authorized (on current GNOME Shell, enable unsafe mode). Reflex will not change this security setting automatically",
        )
    } else {
        backend_error(format!(
            "GNOME Shell introspection is unavailable: {detail}"
        ))
    }
}
