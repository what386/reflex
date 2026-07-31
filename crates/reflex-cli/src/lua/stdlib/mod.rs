use crate::lua::errors::{ErrorKind, LuaError};
use mlua::Lua;

const COMPONENTS: &[(&str, &str)] = &[
    ("table.lua", include_str!("table.lua")),
    ("str.lua", include_str!("str.lua")),
];

pub(crate) fn register(lua: &Lua) -> Result<(), LuaError> {
    for (name, script) in COMPONENTS {
        lua.load(*script).set_name(*name).exec().map_err(lua_err)?;
    }
    Ok(())
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}
