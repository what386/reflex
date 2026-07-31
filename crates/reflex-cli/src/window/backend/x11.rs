use super::super::WindowStore;
use super::super::store::backend_error;
use crate::lua::LuaError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

struct Atoms {
    client_list: Atom,
    supporting_wm_check: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
    wm_name: Atom,
    wm_class: Atom,
}

pub(super) fn start(store: Arc<WindowStore>) -> Result<(), LuaError> {
    let (connection, screen) = x11rb::connect(None)
        .map_err(|error| backend_error(format!("failed to connect to X11: {error}")))?;
    let root = connection
        .setup()
        .roots
        .get(screen)
        .ok_or_else(|| backend_error(format!("X11 screen {screen} is unavailable")))?
        .root;
    let atoms = intern_atoms(&connection)?;
    validate_ewmh(&connection, root, &atoms)?;

    connection
        .change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new()
                .event_mask(EventMask::PROPERTY_CHANGE | EventMask::SUBSTRUCTURE_NOTIFY),
        )
        .map_err(x11_error)?
        .check()
        .map_err(x11_error)?;

    let mut active = HashMap::new();
    let mut next_generation = 1;
    reconcile(
        &connection,
        root,
        &atoms,
        &store,
        &mut active,
        &mut next_generation,
    )?;
    connection.flush().map_err(x11_error)?;
    store.mark_ready();

    std::thread::Builder::new()
        .name("reflex-window-x11".into())
        .spawn(move || {
            if let Err(error) = event_loop(
                connection,
                root,
                atoms,
                store.clone(),
                active,
                next_generation,
            ) {
                store.fail(error);
            }
        })
        .map_err(|error| backend_error(format!("failed to spawn X11 window worker: {error}")))?;
    Ok(())
}

fn event_loop(
    connection: RustConnection,
    root: Window,
    atoms: Atoms,
    store: Arc<WindowStore>,
    mut active: HashMap<Window, String>,
    mut next_generation: u64,
) -> Result<(), LuaError> {
    loop {
        match connection.wait_for_event().map_err(x11_error)? {
            Event::PropertyNotify(event)
                if event.window == root && event.atom == atoms.client_list =>
            {
                reconcile(
                    &connection,
                    root,
                    &atoms,
                    &store,
                    &mut active,
                    &mut next_generation,
                )?;
            }
            Event::PropertyNotify(event)
                if active.contains_key(&event.window)
                    && matches!(
                        event.atom,
                        atom if atom == atoms.net_wm_name
                            || atom == atoms.wm_name
                            || atom == atoms.wm_class
                    ) =>
            {
                let id = active
                    .get(&event.window)
                    .expect("active X11 window should have an ID");
                upsert_window(&connection, &atoms, &store, event.window, id)?;
            }
            Event::DestroyNotify(event) => {
                if let Some(id) = active.remove(&event.window) {
                    store.close(&id);
                }
            }
            _ => {}
        }
    }
}

fn reconcile(
    connection: &RustConnection,
    root: Window,
    atoms: &Atoms,
    store: &WindowStore,
    active: &mut HashMap<Window, String>,
    next_generation: &mut u64,
) -> Result<(), LuaError> {
    let reply = connection
        .get_property(
            false,
            root,
            atoms.client_list,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )
        .map_err(x11_error)?
        .reply()
        .map_err(x11_error)?;
    let listed = reply
        .value32()
        .ok_or_else(|| backend_error("X11 _NET_CLIENT_LIST has an invalid type"))?
        .collect::<Vec<_>>();
    let listed_set = listed.iter().copied().collect::<HashSet<_>>();

    let added = listed
        .iter()
        .copied()
        .filter(|window| !active.contains_key(window))
        .collect::<Vec<_>>();
    for window in added {
        connection
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new()
                    .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY),
            )
            .map_err(x11_error)?
            .check()
            .map_err(x11_error)?;
        let id = window_id(window, *next_generation);
        *next_generation = next_generation.wrapping_add(1);
        upsert_window(connection, atoms, store, window, &id)?;
        active.insert(window, id);
    }
    let stale = active
        .keys()
        .filter(|window| !listed_set.contains(window))
        .copied()
        .collect::<Vec<_>>();
    for window in stale {
        if let Some(id) = active.remove(&window) {
            store.close(&id);
        }
    }
    connection.flush().map_err(x11_error)
}

fn upsert_window(
    connection: &RustConnection,
    atoms: &Atoms,
    store: &WindowStore,
    window: Window,
    id: &str,
) -> Result<(), LuaError> {
    let title = read_text_property(connection, window, atoms.net_wm_name, atoms.utf8_string)?
        .or(read_text_property(
            connection,
            window,
            atoms.wm_name,
            AtomEnum::ANY.into(),
        )?)
        .unwrap_or_default();
    let app_id = read_text_property(connection, window, atoms.wm_class, AtomEnum::STRING.into())?
        .and_then(|value| parse_wm_class(&value));
    store.upsert(id.to_string(), title, app_id);
    Ok(())
}

fn read_text_property(
    connection: &RustConnection,
    window: Window,
    property: Atom,
    property_type: Atom,
) -> Result<Option<String>, LuaError> {
    let reply = connection
        .get_property(false, window, property, property_type, 0, u32::MAX)
        .map_err(x11_error)?
        .reply()
        .map_err(x11_error)?;
    if reply.value.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&reply.value)
            .trim_end_matches('\0')
            .to_string(),
    ))
}

fn validate_ewmh(connection: &RustConnection, root: Window, atoms: &Atoms) -> Result<(), LuaError> {
    let reply = connection
        .get_property(
            false,
            root,
            atoms.supporting_wm_check,
            AtomEnum::WINDOW,
            0,
            1,
        )
        .map_err(x11_error)?
        .reply()
        .map_err(x11_error)?;
    let Some(wm_window) = reply.value32().and_then(|mut values| values.next()) else {
        return Err(backend_error(
            "the active X11 window manager does not advertise EWMH _NET_SUPPORTING_WM_CHECK",
        ));
    };
    let child_reply = connection
        .get_property(
            false,
            wm_window,
            atoms.supporting_wm_check,
            AtomEnum::WINDOW,
            0,
            1,
        )
        .map_err(x11_error)?
        .reply()
        .map_err(x11_error)?;
    if child_reply.value32().and_then(|mut values| values.next()) != Some(wm_window) {
        return Err(backend_error(
            "the active X11 window manager has an invalid EWMH _NET_SUPPORTING_WM_CHECK",
        ));
    }
    Ok(())
}

fn intern_atoms(connection: &RustConnection) -> Result<Atoms, LuaError> {
    Ok(Atoms {
        client_list: intern(connection, b"_NET_CLIENT_LIST")?,
        supporting_wm_check: intern(connection, b"_NET_SUPPORTING_WM_CHECK")?,
        net_wm_name: intern(connection, b"_NET_WM_NAME")?,
        utf8_string: intern(connection, b"UTF8_STRING")?,
        wm_name: AtomEnum::WM_NAME.into(),
        wm_class: AtomEnum::WM_CLASS.into(),
    })
}

fn intern(connection: &RustConnection, name: &[u8]) -> Result<Atom, LuaError> {
    connection
        .intern_atom(false, name)
        .map_err(x11_error)?
        .reply()
        .map(|reply| reply.atom)
        .map_err(x11_error)
}

fn window_id(window: Window, generation: u64) -> String {
    format!("x11:{generation}:0x{window:08x}")
}

fn parse_wm_class(value: &str) -> Option<String> {
    let parts = value
        .split('\0')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts
        .get(1)
        .or_else(|| parts.first())
        .map(|part| (*part).to_string())
}

fn x11_error(error: impl std::fmt::Display) -> LuaError {
    backend_error(format!("X11 window backend failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wm_class_prefers_the_class_over_the_instance() {
        assert_eq!(
            parse_wm_class("firefox\0Firefox\0").as_deref(),
            Some("Firefox")
        );
        assert_eq!(parse_wm_class("kitty\0").as_deref(), Some("kitty"));
        assert_eq!(parse_wm_class("\0"), None);
    }

    #[test]
    fn x11_ids_are_opaque_and_fixed_width() {
        assert_eq!(window_id(42, 7), "x11:7:0x0000002a");
    }
}
