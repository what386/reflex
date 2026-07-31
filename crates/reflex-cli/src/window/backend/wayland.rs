use super::super::WindowStore;
use super::super::store::backend_error;
use crate::lua::{ErrorKind, LuaError};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1 as ext_handle, ext_foreign_toplevel_list_v1 as ext_manager,
};
use wayland_protocols_plasma::plasma_window_management::client::{
    org_kde_plasma_window as kde_window, org_kde_plasma_window_management as kde_manager,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as wlr_handle, zwlr_foreign_toplevel_manager_v1 as wlr_manager,
};

const EXT_INTERFACE: &str = "ext_foreign_toplevel_list_v1";
const WLR_INTERFACE: &str = "zwlr_foreign_toplevel_manager_v1";
const KDE_INTERFACE: &str = "org_kde_plasma_window_management";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Auto,
    Ext,
    Wlr,
    Kde,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Ext,
    Wlr,
    Kde,
}

struct PendingWindow {
    id: Option<String>,
    title: String,
    app_id: Option<String>,
    announced: bool,
}

impl PendingWindow {
    fn new(id: Option<String>) -> Self {
        Self {
            id,
            title: String::new(),
            app_id: None,
            announced: false,
        }
    }
}

struct State {
    store: Arc<WindowStore>,
    _ext_manager: Option<ext_manager::ExtForeignToplevelListV1>,
    _wlr_manager: Option<wlr_manager::ZwlrForeignToplevelManagerV1>,
    _kde_manager: Option<kde_manager::OrgKdePlasmaWindowManagement>,
    pending: HashMap<ObjectId, PendingWindow>,
    next_id: u64,
}

impl State {
    fn new(store: Arc<WindowStore>) -> Self {
        Self {
            store,
            _ext_manager: None,
            _wlr_manager: None,
            _kde_manager: None,
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    fn local_id(&mut self, prefix: &str) -> String {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        format!("{prefix}:{id}")
    }

    fn commit(&mut self, object: &ObjectId) {
        let Some(window) = self.pending.get_mut(object) else {
            return;
        };
        let Some(id) = window.id.clone() else {
            return;
        };
        self.store
            .upsert(id, window.title.clone(), window.app_id.clone());
        window.announced = true;
    }

    fn close(&mut self, object: &ObjectId) {
        let Some(window) = self.pending.remove(object) else {
            return;
        };
        if let Some(id) = window.id {
            self.store.close(&id);
        }
    }

    fn update_title(&mut self, object: &ObjectId, title: String, immediate: bool) {
        let Some(window) = self.pending.get_mut(object) else {
            return;
        };
        window.title = title;
        if immediate && window.announced {
            self.commit(object);
        }
    }

    fn update_app_id(&mut self, object: &ObjectId, app_id: String, immediate: bool) {
        let Some(window) = self.pending.get_mut(object) else {
            return;
        };
        window.app_id = (!app_id.is_empty()).then_some(app_id);
        if immediate && window.announced {
            self.commit(object);
        }
    }
}

pub(super) fn start(store: Arc<WindowStore>, mode: Mode) -> Result<(), LuaError> {
    let connection = Connection::connect_to_env()
        .map_err(|error| backend_error(format!("failed to connect to Wayland: {error}")))?;
    let (globals, mut event_queue) =
        registry_queue_init::<State>(&connection).map_err(wayland_error)?;
    let available = globals.contents().clone_list();
    let protocol = select_protocol(
        mode,
        available.iter().map(|global| global.interface.as_str()),
    )?;
    let queue_handle = event_queue.handle();
    let mut state = State::new(store.clone());

    match protocol {
        Protocol::Kde => {
            state._kde_manager = Some(
                globals
                    .bind::<kde_manager::OrgKdePlasmaWindowManagement, _, _>(
                        &queue_handle,
                        1..=18,
                        (),
                    )
                    .map_err(wayland_error)?,
            );
        }
        Protocol::Ext => {
            state._ext_manager = Some(
                globals
                    .bind::<ext_manager::ExtForeignToplevelListV1, _, _>(&queue_handle, 1..=1, ())
                    .map_err(wayland_error)?,
            );
        }
        Protocol::Wlr => {
            state._wlr_manager = Some(
                globals
                    .bind::<wlr_manager::ZwlrForeignToplevelManagerV1, _, _>(
                        &queue_handle,
                        1..=3,
                        (),
                    )
                    .map_err(wayland_error)?,
            );
        }
    }

    // KDE creates per-window objects in response to the first batch of manager
    // events, so a second roundtrip is needed to receive their initial state.
    event_queue.roundtrip(&mut state).map_err(wayland_error)?;
    event_queue.roundtrip(&mut state).map_err(wayland_error)?;
    store.mark_ready();

    spawn_worker(connection, event_queue, state)?;
    Ok(())
}

fn spawn_worker(
    connection: Connection,
    mut event_queue: EventQueue<State>,
    mut state: State,
) -> Result<(), LuaError> {
    let store = state.store.clone();
    std::thread::Builder::new()
        .name("reflex-window-wayland".into())
        .spawn(move || {
            let _connection = connection;
            loop {
                if let Err(error) = event_queue.blocking_dispatch(&mut state) {
                    store.fail(wayland_error(error));
                    break;
                }
            }
        })
        .map_err(|error| {
            backend_error(format!("failed to spawn Wayland window worker: {error}"))
        })?;
    Ok(())
}

fn select_protocol<'a>(
    mode: Mode,
    interfaces: impl Iterator<Item = &'a str>,
) -> Result<Protocol, LuaError> {
    let interfaces = interfaces.collect::<Vec<_>>();
    let has = |name: &str| interfaces.contains(&name);
    let selected = match mode {
        Mode::Auto if has(KDE_INTERFACE) => Some(Protocol::Kde),
        Mode::Auto if has(EXT_INTERFACE) => Some(Protocol::Ext),
        Mode::Auto if has(WLR_INTERFACE) => Some(Protocol::Wlr),
        Mode::Kde if has(KDE_INTERFACE) => Some(Protocol::Kde),
        Mode::Ext if has(EXT_INTERFACE) => Some(Protocol::Ext),
        Mode::Wlr if has(WLR_INTERFACE) => Some(Protocol::Wlr),
        _ => None,
    };
    selected.ok_or_else(|| {
        LuaError::new(
            ErrorKind::Unsupported,
            format!(
                "the Wayland compositor does not advertise the requested window-list protocol ({mode:?}); expected {KDE_INTERFACE}, {EXT_INTERFACE}, or {WLR_INTERFACE}"
            ),
        )
    })
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_manager::ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_manager::ExtForeignToplevelListV1,
        event: ext_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_manager::Event::Toplevel { toplevel } = event {
            state
                .pending
                .insert(toplevel.id(), PendingWindow::new(None));
        }
    }

    event_created_child!(State, ext_manager::ExtForeignToplevelListV1, [
        ext_manager::EVT_TOPLEVEL_OPCODE => (ext_handle::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_handle::ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ext_handle::ExtForeignToplevelHandleV1,
        event: ext_handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let object = proxy.id();
        match event {
            ext_handle::Event::Identifier { identifier } => {
                if let Some(window) = state.pending.get_mut(&object) {
                    window.id = Some(format!("wayland-ext:{identifier}"));
                }
            }
            ext_handle::Event::Title { title } => {
                state.update_title(&object, title, false);
            }
            ext_handle::Event::AppId { app_id } => {
                state.update_app_id(&object, app_id, false);
            }
            ext_handle::Event::Done => {
                if state
                    .pending
                    .get(&object)
                    .is_some_and(|window| window.id.is_none())
                {
                    let id = state.local_id("wayland-ext");
                    state.pending.get_mut(&object).unwrap().id = Some(id);
                }
                state.commit(&object);
            }
            ext_handle::Event::Closed => {
                state.close(&object);
                proxy.destroy();
            }
            _ => {}
        }
    }
}

impl Dispatch<wlr_manager::ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &wlr_manager::ZwlrForeignToplevelManagerV1,
        event: wlr_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wlr_manager::Event::Toplevel { toplevel } = event {
            let id = state.local_id("wayland-wlr");
            state
                .pending
                .insert(toplevel.id(), PendingWindow::new(Some(id)));
        }
    }

    event_created_child!(State, wlr_manager::ZwlrForeignToplevelManagerV1, [
        wlr_manager::EVT_TOPLEVEL_OPCODE => (wlr_handle::ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<wlr_handle::ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &wlr_handle::ZwlrForeignToplevelHandleV1,
        event: wlr_handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let object = proxy.id();
        match event {
            wlr_handle::Event::Title { title } => {
                state.update_title(&object, title, false);
            }
            wlr_handle::Event::AppId { app_id } => {
                state.update_app_id(&object, app_id, false);
            }
            wlr_handle::Event::Done => state.commit(&object),
            wlr_handle::Event::Closed => {
                state.close(&object);
                proxy.destroy();
            }
            _ => {}
        }
    }
}

impl Dispatch<kde_manager::OrgKdePlasmaWindowManagement, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &kde_manager::OrgKdePlasmaWindowManagement,
        event: kde_manager::Event,
        _: &(),
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            kde_manager::Event::Window { id } if proxy.version() < 13 => {
                let window = proxy.get_window(id, queue_handle, ());
                let opaque_id = state.local_id(&format!("kde:{id}"));
                state
                    .pending
                    .insert(window.id(), PendingWindow::new(Some(opaque_id)));
            }
            kde_manager::Event::WindowWithUuid { id, uuid } => {
                let window = proxy.get_window_by_uuid(uuid.clone(), queue_handle, ());
                let opaque_id = if uuid.is_empty() {
                    state.local_id(&format!("kde:{id}"))
                } else {
                    format!("kde:{uuid}")
                };
                state
                    .pending
                    .insert(window.id(), PendingWindow::new(Some(opaque_id)));
            }
            _ => {}
        }
    }
}

impl Dispatch<kde_window::OrgKdePlasmaWindow, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &kde_window::OrgKdePlasmaWindow,
        event: kde_window::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let object = proxy.id();
        match event {
            kde_window::Event::TitleChanged { title } => {
                state.update_title(&object, title, true);
            }
            kde_window::Event::AppIdChanged { app_id } => {
                state.update_app_id(&object, app_id, true);
            }
            kde_window::Event::InitialState => state.commit(&object),
            kde_window::Event::Unmapped => {
                state.close(&object);
                if proxy.version() >= 4 {
                    proxy.destroy();
                }
            }
            _ => {}
        }
    }
}

fn wayland_error(error: impl std::fmt::Display) -> LuaError {
    backend_error(format!("Wayland window backend failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_kde_then_ext_then_wlr() {
        assert_eq!(
            select_protocol(Mode::Auto, [WLR_INTERFACE, EXT_INTERFACE].into_iter()).unwrap(),
            Protocol::Ext
        );
        assert_eq!(
            select_protocol(
                Mode::Auto,
                [EXT_INTERFACE, KDE_INTERFACE, WLR_INTERFACE].into_iter()
            )
            .unwrap(),
            Protocol::Kde
        );
        assert_eq!(
            select_protocol(Mode::Auto, [WLR_INTERFACE].into_iter()).unwrap(),
            Protocol::Wlr
        );
    }

    #[test]
    fn explicit_protocol_must_be_advertised() {
        assert!(select_protocol(Mode::Kde, [EXT_INTERFACE].into_iter()).is_err());
        assert_eq!(
            select_protocol(Mode::Wlr, [WLR_INTERFACE].into_iter()).unwrap(),
            Protocol::Wlr
        );
    }
}
