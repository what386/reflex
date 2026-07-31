# reflex

`reflex` is a Lua-driven input automation tool for Linux. You write small Lua scripts that register hotkeys, remaps, timers, clipboard actions, process helpers, window observers, and other automation rules. The companion daemon, `reflexd`, owns input handling while scripts are loaded and unloaded.

## Install

The repository provides shell scripts for building and installing the release binaries.

### Fresh install

```sh
./scripts/install.sh
```

This script:

1. Builds the `reflex` GUI/CLI binary and `reflexd`.
2. Installs them to `/usr/local/bin`.
3. Installs the systemd unit for `reflexd`.
4. Enables and restarts the daemon.

### Update an existing install

```sh
./scripts/update.sh
```

This script rebuilds the release binaries, replaces the installed files, reloads systemd, and restarts `reflexd`.

### Requirements

- `cargo`
- `install`
- `systemctl`
- `sudo`

The install scripts also expect the bundled systemd unit at `crates/reflexd/reflexd.service`.

## Quick Start

1. Start or install `reflexd`.
2. Write a Lua script.
3. Run it with `reflex run script.lua`.

Example:

```lua
reflex.bind("ctrl+t", function()
  reflex.key.type("Hello from reflex")
  reflex.key.send("enter")
end)
```

Run it:

```sh
reflex run my_script.lua
```

If you want to validate a script without connecting to the daemon, use:

```sh
reflex check my_script.lua
```

## CLI

```sh
reflex run script.lua
reflex run -d script.lua
reflex gui
reflex list
reflex stop <id|script>
reflex status
reflex check script.lua
reflex keys
```

- `gui` opens the script launcher and running-script dashboard.
- `run` loads a script into `reflexd`.
- `run -d` starts the script in the background.
- `list`, `stop`, and `status` talk to the daemon.
- `check` performs a dry run and validates key names and combos.
- `keys` prints available key names.

## Writing Lua scripts

Scripts can use the global `reflex` object without imports.

### Bind keys

```lua
reflex.bind("ctrl+shift+t", function()
  reflex.key.type("Text from a hotkey")
end)
```

You can also provide separate press and release handlers:

```lua
reflex.bind("ctrl+u", {
  down = function()
    print("pressed")
  end,
  up = function()
    print("released")
  end,
})
```

### Remap keys

```lua
reflex.hotkey("capslock", "ctrl")
reflex.hotkey("back", "forward")
```

### Send input

```lua
reflex.key.type("Hello, World!")
reflex.key.send("ctrl+c")
reflex.key.down("shift")
reflex.key.up("shift")

reflex.mouse.move(100, 200)
reflex.mouse.click("left")
reflex.mouse.scroll(-1)
```

### Timers, clipboard, and signals

```lua
reflex.timer.after(1000, function()
  reflex.notify("reflex", "Timer fired")
end)

reflex.clipboard.set("copied text")
local value = reflex.clipboard.get()

reflex.signal.connect("reflex::started", function()
  print("script loaded")
end)
```

### Observe windows

```lua
for _, win in ipairs(reflex.window.list()) do
  print(win:id(), win:title(), win:app_id())
end

local editor = reflex.window.find(function(win)
  return win:app_id() == "org.gnome.TextEditor"
end)

reflex.signal.connect("window::opened", function(win)
  print("opened", win:title())
end)
```

Window observation works with EWMH window managers on X11 and with supported
foreign-toplevel interfaces on Wayland:

| Session | Backend |
|---|---|
| X11 | EWMH `_NET_CLIENT_LIST` |
| KDE Plasma Wayland | `org_kde_plasma_window_management` |
| wlroots and other compatible compositors | `ext_foreign_toplevel_list_v1`, falling back to `zwlr_foreign_toplevel_manager_v1` |
| GNOME Wayland | `org.gnome.Shell.Introspect.GetWindows` |

GNOME restricts its introspection API. Reflex does not weaken that restriction:
the session must already authorize `GetWindows` (on current upstream GNOME
Shell this requires unsafe mode). If GNOME returns `AccessDenied`, Reflex
reports the requirement instead of changing the session. Enabling unsafe mode
has broader privacy and security implications because it relaxes Shell
restrictions for local session clients.

When `WAYLAND_DISPLAY` is set, Reflex does not fall back to X11 because that
would expose only XWayland windows. For diagnostics and nested sessions, set
`REFLEX_WINDOW_BACKEND` to `auto`, `wayland`, `x11`, `kde`, `gnome`, `ext`, or
`wlr`.

## API Highlights

Common namespaces include:

- `reflex.bind`
- `reflex.hotkey`
- `reflex.notify`
- `reflex.key`
- `reflex.mouse`
- `reflex.clipboard`
- `reflex.timer`
- `reflex.process`
- `reflex.window`
- `reflex.str`
- `reflex.table`
- `reflex.signal`

For more complete reference material, see [`docs/lua_api.md`](docs/lua_api.md).

## Notes

- Key names are lowercase strings such as `ctrl`, `shift`, `alt`, `win`, `enter`, and `space`.
- Combos are joined with `+`, for example `ctrl+shift+t`.
- Clipboard support is text-only.
- `reflex.process` is handled by the local runner, not `reflexd`.
- `reflex.window` is handled by the local desktop session, not `reflexd`.
- `reflexd` is required for input bindings and hotkeys to work.

## Development

Common project commands are defined in `Justfile`:

```sh
just lint
just test
just run script.lua
just start-daemon
```
