use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::runtime::RuntimeState;
use crate::window::{WindowHandle, WindowSnapshot};
use mlua::{Function, Lua, Table, UserData, UserDataMethods, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(crate) fn register_lua(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let window = lua.create_table().map_err(lua_err)?;

    let st = state.clone();
    window
        .set(
            "list",
            lua.create_function(move |lua, ()| {
                let controller = st.borrow().host().windows;
                let snapshot = controller.snapshot().map_err(mlua::Error::external)?;
                window_list(lua, snapshot.windows)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    window
        .set(
            "find",
            lua.create_function(move |lua, selector: Value| {
                validate_selector(lua, &selector)?;
                let controller = st.borrow().host().windows;
                let snapshot = controller.snapshot().map_err(mlua::Error::external)?;
                find_window(lua, &selector, snapshot.windows)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    window
        .set(
            "exists",
            lua.create_function(move |lua, selector: Value| {
                validate_selector(lua, &selector)?;
                let controller = st.borrow().host().windows;
                let snapshot = controller.snapshot().map_err(mlua::Error::external)?;
                Ok(find_handle(lua, &selector, snapshot.windows)?.is_some())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    window
        .set(
            "wait",
            lua.create_function(move |lua, (selector, timeout): (Value, Option<f64>)| {
                validate_selector(lua, &selector)?;
                let timeout = parse_timeout(timeout)?;
                let controller = st.borrow().host().windows;
                wait_for_window(lua, &selector, controller.as_ref(), timeout)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("window", window).map_err(lua_err)
}

#[derive(Clone)]
pub(crate) struct WindowEntry(pub(crate) WindowHandle);

impl UserData for WindowEntry {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("id", |_, this, ()| Ok(this.0.id()));
        methods.add_method("title", |_, this, ()| Ok(this.0.title()));
        methods.add_method("app_id", |_, this, ()| Ok(this.0.app_id()));
        methods.add_method("exists", |_, this, ()| Ok(this.0.exists()));
    }
}

pub(crate) fn window_value(lua: &Lua, window: WindowHandle) -> mlua::Result<Value> {
    lua.create_userdata(WindowEntry(window))
        .map(Value::UserData)
}

fn window_list(lua: &Lua, windows: Vec<WindowHandle>) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(windows.len(), 0)?;
    for (index, window) in windows.into_iter().enumerate() {
        table.raw_set(index + 1, WindowEntry(window))?;
    }
    Ok(table)
}

fn find_window(lua: &Lua, selector: &Value, windows: Vec<WindowHandle>) -> mlua::Result<Value> {
    match find_handle(lua, selector, windows)? {
        Some(window) => window_value(lua, window),
        None => Ok(Value::Nil),
    }
}

fn find_handle(
    lua: &Lua,
    selector: &Value,
    windows: Vec<WindowHandle>,
) -> mlua::Result<Option<WindowHandle>> {
    for window in windows {
        if selector_matches(lua, selector, &window)? {
            return Ok(Some(window));
        }
    }
    Ok(None)
}

fn selector_matches(lua: &Lua, selector: &Value, window: &WindowHandle) -> mlua::Result<bool> {
    match selector {
        Value::String(pattern) => {
            let pattern = pattern.to_str()?.to_lowercase();
            let string: Table = lua.globals().get("string")?;
            let find: Function = string.get("find")?;
            let found: Option<usize> = find.call((window.title().to_lowercase(), pattern))?;
            Ok(found.is_some())
        }
        Value::Function(predicate) => {
            predicate.call(lua.create_userdata(WindowEntry(window.clone()))?)
        }
        _ => unreachable!("selector is validated before matching"),
    }
}

fn validate_selector(lua: &Lua, selector: &Value) -> mlua::Result<()> {
    match selector {
        Value::String(pattern) => {
            let string: Table = lua.globals().get("string")?;
            let find: Function = string.get("find")?;
            let _: Option<usize> = find.call(("", pattern.to_str()?.to_lowercase()))?;
            Ok(())
        }
        Value::Function(_) => Ok(()),
        other => Err(mlua::Error::external(LuaError::new(
            ErrorKind::Runtime,
            format!(
                "window selector must be a title pattern or predicate function, got {}",
                other.type_name()
            ),
        ))),
    }
}

fn parse_timeout(timeout: Option<f64>) -> mlua::Result<Option<Duration>> {
    let Some(timeout) = timeout else {
        return Ok(None);
    };
    if !timeout.is_finite() || timeout < 0.0 {
        return Err(mlua::Error::external(LuaError::new(
            ErrorKind::Runtime,
            "reflex.window.wait timeout must be a finite, non-negative number of seconds",
        )));
    }
    Duration::try_from_secs_f64(timeout).map(Some).map_err(|_| {
        mlua::Error::external(LuaError::new(
            ErrorKind::Runtime,
            "reflex.window.wait timeout is too large",
        ))
    })
}

fn wait_for_window(
    lua: &Lua,
    selector: &Value,
    controller: &dyn crate::window::WindowController,
    timeout: Option<Duration>,
) -> mlua::Result<Value> {
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    if timeout.is_some() && deadline.is_none() {
        return Err(mlua::Error::external(LuaError::new(
            ErrorKind::Runtime,
            "reflex.window.wait timeout is too large",
        )));
    }

    loop {
        let WindowSnapshot {
            windows,
            generation,
        } = controller.snapshot().map_err(mlua::Error::external)?;
        if let Some(window) = find_handle(lua, selector, windows)? {
            return window_value(lua, window);
        }

        let remaining = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(Value::Nil);
                }
                Some(remaining)
            }
            None => None,
        };
        if !controller
            .wait_for_change(generation, remaining)
            .map_err(mlua::Error::external)?
        {
            return Ok(Value::Nil);
        }
    }
}

fn lua_err(error: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::check_host;
    use crate::lua::{Runtime, RuntimeConfig};
    use crate::window::{WindowController, WindowEvent, WindowSnapshot};
    use std::sync::{Arc, Mutex};

    struct FakeWindows {
        windows: Vec<WindowHandle>,
        events: Mutex<Vec<WindowEvent>>,
    }

    impl WindowController for FakeWindows {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn snapshot(&self) -> Result<WindowSnapshot, LuaError> {
            Ok(WindowSnapshot {
                windows: self.windows.clone(),
                generation: 1,
            })
        }

        fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError> {
            Ok(std::mem::take(
                &mut *self
                    .events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            ))
        }

        fn wait_for_change(&self, _: u64, _: Option<Duration>) -> Result<bool, LuaError> {
            Ok(false)
        }
    }

    fn runtime_with_windows(windows: Vec<WindowHandle>, events: Vec<WindowEvent>) -> Runtime {
        let mut host = check_host();
        host.windows = Arc::new(FakeWindows {
            windows,
            events: Mutex::new(events),
        });
        Runtime::new(RuntimeConfig { host }).unwrap()
    }

    #[test]
    fn check_host_exposes_empty_side_effect_free_window_api() {
        let runtime = Runtime::new(RuntimeConfig { host: check_host() }).unwrap();
        runtime
            .run_str(
                r#"
                windows = reflex.window.list()
                found = reflex.window.find("anything")
                exists = reflex.window.exists("anything")
                waited = reflex.window.wait("anything")
                "#,
                "window-check-test",
            )
            .unwrap();

        let globals = runtime.lua().globals();
        assert_eq!(globals.get::<Table>("windows").unwrap().raw_len(), 0);
        assert_eq!(globals.get::<Value>("found").unwrap(), Value::Nil);
        assert!(!globals.get::<bool>("exists").unwrap());
        assert_eq!(globals.get::<Value>("waited").unwrap(), Value::Nil);
    }

    #[test]
    fn rejects_invalid_selector_and_timeout() {
        let runtime = Runtime::new(RuntimeConfig { host: check_host() }).unwrap();
        assert!(
            runtime
                .run_str("reflex.window.find(42)", "bad-selector")
                .is_err()
        );
        assert!(
            runtime
                .run_str("reflex.window.find('%')", "bad-pattern")
                .is_err()
        );
        assert!(
            runtime
                .run_str("reflex.window.wait('x', -1)", "bad-timeout")
                .is_err()
        );
    }

    #[test]
    fn lists_metadata_and_matches_patterns_and_predicates() {
        let terminal = WindowHandle::new(
            "fake:1".into(),
            "Alpha Terminal".into(),
            Some("org.example.Terminal".into()),
        );
        let editor = WindowHandle::new(
            "fake:2".into(),
            "notes.lua".into(),
            Some("org.example.Editor".into()),
        );
        let runtime = runtime_with_windows(vec![terminal, editor], Vec::new());
        runtime
            .run_str(
                r#"
                count = #reflex.window.list()
                terminal = reflex.window.find("terminal")
                lua_file = reflex.window.find("%.lua$")
                editor = reflex.window.find(function(win)
                  return win:app_id() == "org.example.Editor"
                end)
                metadata = {
                  terminal:id(),
                  terminal:title(),
                  terminal:app_id(),
                  terminal:exists(),
                }
                "#,
                "window-match-test",
            )
            .unwrap();

        let globals = runtime.lua().globals();
        assert_eq!(globals.get::<i64>("count").unwrap(), 2);
        let metadata = globals.get::<Table>("metadata").unwrap();
        assert_eq!(metadata.get::<String>(1).unwrap(), "fake:1");
        assert_eq!(metadata.get::<String>(2).unwrap(), "Alpha Terminal");
        assert_eq!(metadata.get::<String>(3).unwrap(), "org.example.Terminal");
        assert!(metadata.get::<bool>(4).unwrap());
        assert!(matches!(
            globals.get::<Value>("lua_file").unwrap(),
            Value::UserData(_)
        ));
        assert!(matches!(
            globals.get::<Value>("editor").unwrap(),
            Value::UserData(_)
        ));
    }

    #[test]
    fn dispatches_normalized_window_signals() {
        let window = WindowHandle::new("fake:3".into(), "New title".into(), Some("app".into()));
        let runtime = runtime_with_windows(
            vec![window.clone()],
            vec![
                WindowEvent::Opened(window.clone()),
                WindowEvent::TitleChanged {
                    window,
                    title: "New title".into(),
                },
            ],
        );
        runtime
            .run_str(
                r#"
                opened = nil
                changed = nil
                reflex.signal.connect("window::opened", function(win)
                  opened = win:title()
                end)
                reflex.signal.connect("window::title_changed", function(win, title)
                  changed = win:id() .. ":" .. title
                end)
                "#,
                "window-signal-test",
            )
            .unwrap();
        runtime.poll_windows().unwrap();

        let globals = runtime.lua().globals();
        assert_eq!(globals.get::<String>("opened").unwrap(), "New title");
        assert_eq!(
            globals.get::<String>("changed").unwrap(),
            "fake:3:New title"
        );
    }
}
