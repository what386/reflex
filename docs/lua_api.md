# Reflex API Reference

Reflex scripts are plain Lua files. The `reflex` global is available everywhere; no imports are needed.

Input handling is owned by `reflexd`. Start `reflexd` first, then run one or more scripts with `reflex run script.lua`. Each script registers its own rules with the daemon; when a script exits, its rules are removed. A sample systemd unit is available at `crates/reflexd/reflexd.service`; it assumes `reflexd` is installed at `/usr/local/bin/reflexd`.

## CLI

```sh
reflex run script.lua
reflex run -d script.lua
reflex list
reflex stop <id|script>
reflex status
reflex check script.lua
reflex keys
```

`run -d` starts a script in the background and returns after it registers with `reflexd`. `list`, `stop`, and `status` talk to `reflexd`. `check` loads a script with a dry-run host, so it does not connect to `reflexd` or perform host side effects. It also validates key names and combos passed to `bind`, `hotkey`, `key.send`, `key.down`, and `key.up`.

## reflex.signal

Owns built-in and user-defined signals. Signals use `domain::event` names.

```lua
reflex.signal.connect("reflex::started", function() end)
reflex.signal.connect("reflex::exiting", function() end)
reflex.signal.emit("myapp::ready")
reflex.signal.emit("myapp::status", { code = 200 })
reflex.signal.disconnect("myapp::status", fn)
```

## Root

```lua
reflex.bind("ctrl+t", function() end)        -- key combo -> function
reflex.bind("ctrl+u", {                      -- separate press/release handlers
  down = function() end,
  up = function() end,
})
reflex.bind("ctrl+back", function() end)     -- keyboard + mouse button combo
reflex.hotkey("capslock", "ctrl")           -- key becomes another key
reflex.hotkey("back", "forward")            -- mouse button remap
reflex.notify("Title", "Body")              -- desktop notification
reflex.sleep(500)                            -- pause for ms
reflex.exit()                                -- request clean shutdown
```

`reflex.notify()` sends a desktop notification through the Freedesktop notification service.

```lua
reflex.notify({
  title = "Build finished",
  body = "reflex compiled successfully",
  urgency = "normal",                        -- "low", "normal", "critical"
  timeout = 5000,                            -- ms; 0 = never, negative = server default
  icon = "dialog-information",
  app_name = "reflex",
})
```

## reflex.key

```lua
reflex.key.type("Hello, World!")
reflex.key.send("ctrl+c")
reflex.key.down("shift")
reflex.key.up("shift")
```

`reflex.key.send("H")` sends the physical `h` key and warns with a `shift+h` hint. Use explicit combos such as `reflex.key.send("shift+h")` for capitals, or `reflex.key.type("Hello")` for text.

## reflex.mouse

```lua
reflex.mouse.move(100, 200)
reflex.mouse.move(50, 50, "rel")
reflex.mouse.click("left")
reflex.mouse.click("left", 300, 400)
reflex.mouse.down("left")
reflex.mouse.up("left")
reflex.mouse.scroll(-1)                      -- positive = up
```

## reflex.clipboard

Clipboard APIs are text-only. On Linux, Reflex uses `wl-copy`/`wl-paste`, `xclip`, or `xsel`, depending on the current session and installed tools.

```lua
reflex.clipboard.set("Hello, World!")
local text = reflex.clipboard.get()
reflex.clipboard.clear()
```

## reflex.timer

```lua
local t = reflex.timer.new(1000, function()
  print("tick")
end)

t:start()
t:pause()
t:resume()
t:clear()

reflex.timer.after(2000, function()
  print("done")
end)
```

## reflex.process

Process APIs are handled by the local runner, not `reflexd`. `find()` and `pkill()` match process names from `/proc`, including the executable basename from the command line.

```lua
local pid = reflex.process.spawn("kitty")
reflex.process.find("kitty")                 -- pid or nil
reflex.process.kill(pid)
reflex.process.pkill("kitty")                -- number of matched processes signaled
```

## reflex.window

Window APIs are handled by the local runner. They observe windows but do not
focus, close, move, minimize, or otherwise control them.

```lua
local windows = reflex.window.list()         -- array of Window objects
local win = reflex.window.find("terminal")   -- case-insensitive Lua title pattern
local found = reflex.window.exists("%.lua$")
local waited = reflex.window.wait("Build", 5) -- timeout in seconds; nil on timeout

local by_app = reflex.window.find(function(candidate)
  return candidate:app_id() == "org.mozilla.firefox"
end)

if win then
  print(win:id())                            -- opaque mapped-lifetime ID
  print(win:title())
  print(win:app_id())                        -- string or nil
  print(win:exists())                        -- false after window::closed
end
```

`wait()` waits indefinitely when no timeout is supplied. A timeout of `0`
performs an immediate lookup. Negative, infinite, and NaN timeouts are errors.
Under `reflex check`, window lists are empty and waits return immediately.

List ordering and the first result returned by `find()` are backend-defined.
String selectors match titles. Use a predicate to match `app_id()` or other
metadata.

```lua
reflex.signal.connect("window::opened", function(win) end)
reflex.signal.connect("window::closed", function(win) end)
reflex.signal.connect("window::title_changed", function(win, title) end)
```

Windows present when observation starts form the initial baseline and do not
emit `window::opened`. A handle passed to `window::closed` keeps its cached ID,
title, and app ID, while `exists()` returns `false`.

Supported backends are EWMH X11, KDE Plasma's native Wayland window-management
protocol, standard or wlroots foreign-toplevel protocols, and GNOME Shell's
Introspect D-Bus API. Wayland sessions never silently fall back to X11.

GNOME must authorize
`org.gnome.Shell.Introspect.GetWindows`; current upstream Shell normally
requires unsafe mode. Reflex does not enable it automatically. The backend can
be selected explicitly for diagnostics with
`REFLEX_WINDOW_BACKEND=auto|wayland|x11|kde|gnome|ext|wlr`.

## reflex.str

```lua
reflex.str.trim("  hi  ")
reflex.str.split("a,b", ",")
reflex.str.starts_with("hello", "he")
reflex.str.ends_with("hello", "lo")
reflex.str.join({ "a", "b" }, "-")
```

## reflex.table

```lua
reflex.table.merge({ a = 1 }, { b = 2 })
reflex.table.deep_merge({ a = { b = 1 } }, { a = { c = 2 } })
reflex.table.contains({ "a", "b" }, "b")
reflex.table.keys({ a = 1, b = 2 })
reflex.table.map({ 1, 2 }, function(value) return value * 2 end)
reflex.table.filter({ 1, 2, 3 }, function(value) return value > 1 end)
```

## Notes

- Key names are lowercase strings: `"ctrl"`, `"shift"`, `"alt"`, `"win"`, `"enter"`, `"space"`, `"f1"`-`"f12"`, etc.
- Combos are joined with `+`: `"ctrl+shift+t"`.
- Mouse buttons: `"left"`, `"right"`, `"middle"`, `"back"`, `"forward"`.
- Mouse-button binds and hotkeys use `"mouse_left"`, `"mouse_right"`, `"mouse_middle"`, `"back"`, and `"forward"`. In binds, `"left"` and `"right"` are arrow keys. `"back"` and `"forward"` match both common Linux thumb-button code families.
