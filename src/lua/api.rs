use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::runtime::{CallbackEntry, RuntimeState, TimerEntry};
use crate::lua::types::MouseMoveMode;
use mlua::{Function, Lua, Table, Value, Variadic};
use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;
use std::time::Duration;

pub(crate) fn register_api(lua: &Lua, state: Rc<RefCell<RuntimeState>>) -> Result<(), LuaError> {
    let reflex = lua.create_table().map_err(lua_err)?;
    lua.globals()
        .set("reflex", reflex.clone())
        .map_err(lua_err)?;

    register_root(lua, &reflex, state.clone())?;
    register_signal(lua, &reflex, state.clone())?;
    register_key(lua, &reflex, state.clone())?;
    register_mouse(lua, &reflex, state.clone())?;
    register_timer(lua, &reflex, state.clone())?;
    register_process(lua, &reflex, state)?;
    register_stdlib(lua)?;
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

fn register_signal(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let signal = lua.create_table().map_err(lua_err)?;
    let st = state.clone();
    signal
        .set(
            "connect",
            lua.create_function(move |_, (name, callback): (String, Function)| {
                let ptr = callback.to_pointer();
                st.borrow_mut()
                    .signals
                    .entry(name)
                    .or_default()
                    .push(CallbackEntry { ptr, callback });
                Ok(())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state.clone();
    signal
        .set(
            "disconnect",
            lua.create_function(move |_, (name, callback): (String, Function)| {
                let target = callback.to_pointer();
                if let Some(callbacks) = st.borrow_mut().signals.get_mut(&name) {
                    callbacks.retain(|entry| !ptr::eq(entry.ptr, target));
                }
                Ok(())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    signal
        .set(
            "emit",
            lua.create_function(move |_, (name, args): (String, Variadic<Value>)| {
                emit_signal(&st, &name, args.into_iter().collect())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("signal", signal).map_err(lua_err)
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

fn register_timer(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let timer = lua.create_table().map_err(lua_err)?;
    let st = state.clone();
    timer
        .set(
            "once",
            lua.create_function(move |_, (ms, callback): (u64, Function)| {
                st.borrow_mut().add_timer(ms, callback, false, true)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    timer
        .set(
            "new",
            lua.create_function(move |lua, (ms, callback): (u64, Function)| {
                let id = st.borrow_mut().add_timer(ms, callback, true, false)?;
                lua.create_userdata(TimerEntry {
                    id,
                    state: st.clone(),
                })
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("timer", timer).map_err(lua_err)
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

fn register_stdlib(lua: &Lua) -> Result<(), LuaError> {
    for (name, script) in [
        ("table.lua", include_str!("stdlib/table.lua")),
        ("str.lua", include_str!("stdlib/str.lua")),
    ] {
        lua.load(script).set_name(name).exec().map_err(lua_err)?;
    }
    Ok(())
}

pub(crate) fn emit_signal(
    state: &Rc<RefCell<RuntimeState>>,
    name: &str,
    args: Vec<Value>,
) -> mlua::Result<()> {
    let callbacks = state
        .borrow()
        .signals
        .get(name)
        .cloned()
        .unwrap_or_default();
    for entry in callbacks {
        entry
            .callback
            .call::<()>(mlua::MultiValue::from_vec(args.clone()))?;
    }
    Ok(())
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
