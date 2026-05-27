use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::runtime::RuntimeState;
use mlua::{Function, Lua, Table, Value, Variadic};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct CallbackEntry {
    ptr: *const c_void,
    callback: Function,
}

#[derive(Default)]
pub(crate) struct SignalState {
    callbacks: HashMap<String, Vec<CallbackEntry>>,
}

impl SignalState {
    pub(crate) fn connect(&mut self, name: String, callback: Function) {
        let ptr = callback.to_pointer();
        self.callbacks
            .entry(name)
            .or_default()
            .push(CallbackEntry { ptr, callback });
    }

    pub(crate) fn disconnect(&mut self, name: &str, callback: &Function) {
        let target = callback.to_pointer();
        if let Some(callbacks) = self.callbacks.get_mut(name) {
            callbacks.retain(|entry| !ptr::eq(entry.ptr, target));
        }
    }

    pub(crate) fn callbacks_for(&self, name: &str) -> Vec<CallbackEntry> {
        self.callbacks.get(name).cloned().unwrap_or_default()
    }
}

pub(crate) fn register_lua(
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
                st.borrow_mut().signals.connect(name, callback);
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
                st.borrow_mut().signals.disconnect(&name, &callback);
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
                emit(&st, &name, args.into_iter().collect())
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("signal", signal).map_err(lua_err)
}

pub(crate) fn emit(
    state: &Rc<RefCell<RuntimeState>>,
    name: &str,
    args: Vec<Value>,
) -> mlua::Result<()> {
    let callbacks = state.borrow().signals.callbacks_for(name);
    for entry in callbacks {
        entry
            .callback
            .call::<()>(mlua::MultiValue::from_vec(args.clone()))?;
    }
    Ok(())
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}
