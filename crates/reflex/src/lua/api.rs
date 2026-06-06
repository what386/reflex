use crate::host::{NotificationOptions, NotificationUrgency};
use crate::lua::components::{signal, timer};
use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::runtime::RuntimeState;
use crate::lua::stdlib;
use crate::lua::types::MouseMoveMode;
use mlua::{Function, Lua, Table, Value, Variadic};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub(crate) fn register_api(lua: &Lua, state: Rc<RefCell<RuntimeState>>) -> Result<(), LuaError> {
    let reflex = lua.create_table().map_err(lua_err)?;
    lua.globals()
        .set("reflex", reflex.clone())
        .map_err(lua_err)?;

    register_root(lua, &reflex, state.clone())?;
    signal::register_lua(lua, &reflex, state.clone())?;
    register_key(lua, &reflex, state.clone())?;
    register_mouse(lua, &reflex, state.clone())?;
    register_clipboard(lua, &reflex, state.clone())?;
    timer::register_lua(lua, &reflex, state.clone())?;
    register_process(lua, &reflex, state)?;
    stdlib::register(lua)?;
    Ok(())
}

fn register_root(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let st = state.clone();
    reflex
        .set(
            "bind",
            lua.create_function(move |_, (combo, callback): (String, Function)| {
                st.borrow()
                    .host()
                    .remapping
                    .register_bind(&combo)
                    .map_err(mlua::Error::external)?;
                st.borrow_mut().bindings.push((combo, callback));
                Ok(())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    reflex
        .set(
            "hotkey",
            lua.create_function(move |_, (from, to): (String, String)| {
                st.borrow()
                    .host()
                    .remapping
                    .remap_key(&from, &to)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex
        .set(
            "sleep",
            lua.create_function(|_, ms: u64| {
                std::thread::sleep(Duration::from_millis(ms));
                Ok(())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    reflex
        .set(
            "notify",
            lua.create_function(move |_, args: Variadic<Value>| {
                let options = notification_options(args).map_err(mlua::Error::external)?;
                st.borrow()
                    .host()
                    .notifications
                    .notify(&options)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    reflex
        .set(
            "exit",
            lua.create_function(move |_, ()| {
                st.borrow_mut().should_exit = true;
                Ok(())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)
}

fn register_key(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let key = lua.create_table().map_err(lua_err)?;
    let st = state.clone();
    key.set(
        "type",
        lua.create_function(move |_, text: String| {
            st.borrow()
                .host()
                .input
                .key_send(&text)
                .map_err(mlua::Error::external)
        })
        .map_err(lua_err)?,
    )
    .map_err(lua_err)?;
    let st = state.clone();
    key.set(
        "send",
        lua.create_function(move |_, combo: String| {
            st.borrow()
                .host()
                .input
                .key_tap(&combo)
                .map_err(mlua::Error::external)
        })
        .map_err(lua_err)?,
    )
    .map_err(lua_err)?;
    let st = state.clone();
    key.set(
        "down",
        lua.create_function(move |_, name: String| {
            st.borrow()
                .host()
                .input
                .key_down(&name)
                .map_err(mlua::Error::external)
        })
        .map_err(lua_err)?,
    )
    .map_err(lua_err)?;
    let st = state;
    key.set(
        "up",
        lua.create_function(move |_, name: String| {
            st.borrow()
                .host()
                .input
                .key_up(&name)
                .map_err(mlua::Error::external)
        })
        .map_err(lua_err)?,
    )
    .map_err(lua_err)?;
    reflex.set("key", key).map_err(lua_err)
}

fn register_mouse(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let mouse = lua.create_table().map_err(lua_err)?;
    let st = state.clone();
    mouse
        .set(
            "move",
            lua.create_function(move |_, (x, y, mode): (i32, i32, Option<String>)| {
                let mode = match mode.as_deref() {
                    Some("rel") | Some("relative") => MouseMoveMode::Relative,
                    Some("abs") | Some("absolute") | None => MouseMoveMode::Absolute,
                    Some(other) => {
                        return Err(mlua::Error::external(LuaError::new(
                            ErrorKind::Runtime,
                            format!("unsupported mouse move mode: {other}"),
                        )));
                    }
                };
                st.borrow()
                    .host()
                    .input
                    .mouse_move(x, y, mode)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    mouse
        .set(
            "click",
            lua.create_function(
                move |_, (button, x, y): (String, Option<i32>, Option<i32>)| {
                    st.borrow()
                        .host()
                        .input
                        .mouse_click(&button, x, y)
                        .map_err(mlua::Error::external)
                },
            )
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    mouse
        .set(
            "down",
            lua.create_function(move |_, button: String| {
                st.borrow()
                    .host()
                    .input
                    .mouse_down(&button)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    let st = state.clone();
    mouse
        .set(
            "up",
            lua.create_function(move |_, button: String| {
                st.borrow()
                    .host()
                    .input
                    .mouse_up(&button)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    let st = state;
    mouse
        .set(
            "scroll",
            lua.create_function(move |_, delta: i32| {
                st.borrow()
                    .host()
                    .input
                    .mouse_scroll(delta)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    reflex.set("mouse", mouse).map_err(lua_err)
}

fn register_clipboard(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let clipboard = lua.create_table().map_err(lua_err)?;

    let st = state.clone();
    clipboard
        .set(
            "get",
            lua.create_function(move |_, ()| {
                st.borrow()
                    .host()
                    .clipboard
                    .get()
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    clipboard
        .set(
            "set",
            lua.create_function(move |_, text: String| {
                st.borrow()
                    .host()
                    .clipboard
                    .set(&text)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    clipboard
        .set(
            "clear",
            lua.create_function(move |_, ()| {
                st.borrow()
                    .host()
                    .clipboard
                    .clear()
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("clipboard", clipboard).map_err(lua_err)
}

fn register_process(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let process = lua.create_table().map_err(lua_err)?;
    let st = state.clone();
    process
        .set(
            "spawn",
            lua.create_function(move |_, args: Variadic<String>| {
                let argv = args.into_iter().collect::<Vec<_>>();
                let Some((program, rest)) = argv.split_first() else {
                    return Err(mlua::Error::external(LuaError::new(
                        ErrorKind::Runtime,
                        "reflex.process.spawn requires a program",
                    )));
                };
                st.borrow()
                    .host()
                    .process
                    .spawn(program, rest)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    process
        .set(
            "find",
            lua.create_function(move |lua, name: String| {
                option_u32(
                    lua,
                    st.borrow()
                        .host()
                        .process
                        .find(&name)
                        .map_err(mlua::Error::external)?,
                )
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    let st = state.clone();
    process
        .set(
            "kill",
            lua.create_function(move |_, pid: u32| {
                st.borrow()
                    .host()
                    .process
                    .kill(pid)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    let st = state;
    process
        .set(
            "pkill",
            lua.create_function(move |_, name: String| {
                st.borrow()
                    .host()
                    .process
                    .pkill(&name)
                    .map_err(mlua::Error::external)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;
    reflex.set("process", process).map_err(lua_err)
}

fn notification_options(args: Variadic<Value>) -> Result<NotificationOptions, LuaError> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [Value::String(title)] => Ok(NotificationOptions {
            title: title.to_str().map_err(lua_err)?.to_string(),
            body: None,
            app_name: None,
            icon: None,
            urgency: NotificationUrgency::Normal,
            timeout: None,
        }),
        [Value::String(title), Value::String(body)] => Ok(NotificationOptions {
            title: title.to_str().map_err(lua_err)?.to_string(),
            body: Some(body.to_str().map_err(lua_err)?.to_string()),
            app_name: None,
            icon: None,
            urgency: NotificationUrgency::Normal,
            timeout: None,
        }),
        [Value::Table(table)] => notification_options_from_table(table),
        _ => Err(LuaError::new(
            ErrorKind::Runtime,
            "reflex.notify expects a title string or options table",
        )),
    }
}

fn notification_options_from_table(table: &Table) -> Result<NotificationOptions, LuaError> {
    let title = match table.get::<Option<String>>("title").map_err(lua_err)? {
        Some(title) => title,
        None => table
            .get::<Option<String>>("summary")
            .map_err(lua_err)?
            .ok_or_else(|| {
                LuaError::new(ErrorKind::Runtime, "reflex.notify options require title")
            })?,
    };
    let urgency = table
        .get::<Option<String>>("urgency")
        .map_err(lua_err)?
        .map(|urgency| notification_urgency(&urgency))
        .transpose()?
        .unwrap_or(NotificationUrgency::Normal);

    Ok(NotificationOptions {
        title,
        body: match table.get::<Option<String>>("body").map_err(lua_err)? {
            Some(body) => Some(body),
            None => table.get::<Option<String>>("message").map_err(lua_err)?,
        },
        app_name: table.get::<Option<String>>("app_name").map_err(lua_err)?,
        icon: table.get::<Option<String>>("icon").map_err(lua_err)?,
        urgency,
        timeout: table.get::<Option<i32>>("timeout").map_err(lua_err)?,
    })
}

fn notification_urgency(urgency: &str) -> Result<NotificationUrgency, LuaError> {
    match urgency.trim().to_ascii_lowercase().as_str() {
        "low" => Ok(NotificationUrgency::Low),
        "normal" => Ok(NotificationUrgency::Normal),
        "critical" | "high" => Ok(NotificationUrgency::Critical),
        other => Err(LuaError::new(
            ErrorKind::Runtime,
            format!("unsupported notification urgency: {other}"),
        )),
    }
}

fn option_u32(_: &Lua, value: Option<u32>) -> mlua::Result<Value> {
    match value {
        Some(value) => Ok(Value::Integer(value.into())),
        None => Ok(Value::Nil),
    }
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notification_urgency_names() {
        assert_eq!(
            notification_urgency("low").unwrap(),
            NotificationUrgency::Low
        );
        assert_eq!(
            notification_urgency("normal").unwrap(),
            NotificationUrgency::Normal
        );
        assert_eq!(
            notification_urgency("critical").unwrap(),
            NotificationUrgency::Critical
        );
        assert_eq!(
            notification_urgency("high").unwrap(),
            NotificationUrgency::Critical
        );
        assert!(notification_urgency("urgent").is_err());
    }
}
