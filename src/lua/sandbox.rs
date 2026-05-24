use crate::lua::errors::{ErrorKind, LuaError};
use mlua::{Lua, Value};

pub fn configure_sandbox(lua: &Lua) -> Result<(), LuaError> {
    let globals = lua.globals();
    for name in [
        "require", "load", "loadfile", "dofile", "debug", "io", "package",
    ] {
        globals.set(name, Value::Nil).map_err(lua_err)?;
    }
    if let Ok(os_table) = globals.get::<mlua::Table>("os") {
        for name in [
            "execute",
            "exit",
            "remove",
            "rename",
            "tmpname",
            "setlocale",
        ] {
            os_table.set(name, Value::Nil).map_err(lua_err)?;
        }
    }
    Ok(())
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::SandboxViolation, err.to_string())
}
