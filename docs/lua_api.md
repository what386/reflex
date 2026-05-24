# Reflex API Reference

Reflex scripts are plain Lua files. The `reflex` global is available everywhere — no imports needed.

---

## reflex.signal

Owns the entire event system — both built-in and user-defined signals. All signals use `domain::event` naming.

```lua
-- Listening
reflex.signal.connect("window::open", function(win) end)
reflex.signal.connect("window::close", function(win) end)
reflex.signal.connect("focus::change", function(win) end)
reflex.signal.connect("screen::change", function() end)

-- Stop listening
reflex.signal.disconnect("window::open", fn)

-- User-defined signals (emit and connect work the same way)
reflex.signal.emit("myapp::ready")
reflex.signal.emit("myapp::status", { code = 200 })   -- arbitrary data
reflex.signal.connect("myapp::status", function(data)
    print(data.code)
end)
```

Built-in signals are just pre-wired emitters — user signals and built-in signals are the same system.

---

## Root

The root namespace holds the things that *are* Reflex — triggering, remapping, and flow control.

```lua
-- Key bindings
reflex.bind("ctrl+t", function() end)        -- key combo → function
reflex.bindstring("btw", function() end)     -- typed string → function

-- Remaps
reflex.hotkey("capslock", "ctrl")            -- key becomes another key
reflex.hotstring("btw", "by the way")        -- typed string becomes another string

-- Utility
reflex.sleep(500)                            -- pause for ms
reflex.msgbox("Hello!")                      -- simple popup dialog
```

---

## reflex.key

Owns all keyboard input and output.

```lua
reflex.key.send("Hello, World!")             -- type a string
reflex.key.tap("ctrl+c")                     -- press a key or combo
reflex.key.down("shift")                     -- hold a key
reflex.key.up("shift")                       -- release a key
```

---

## reflex.mouse

Owns all mouse input and output.

```lua
reflex.mouse.move(100, 200)                  -- move to x, y
reflex.mouse.move(50, 50, "rel")             -- move relative to current position
reflex.mouse.click("left")                   -- click at current position
reflex.mouse.click("left", 300, 400)         -- click at x, y
reflex.mouse.down("left")                    -- hold mouse button
reflex.mouse.up("left")                      -- release mouse button
reflex.mouse.scroll(-1)                      -- scroll down (positive = up)
```

---

## reflex.window

Owns window discovery and management.

```lua
-- Flat (quick, name-based one-liners)
reflex.window.focus("Notepad")               -- focus a window by name
reflex.window.close("Notepad")
reflex.window.minimize("Notepad")
reflex.window.maximize("Notepad")
reflex.window.exists("Notepad")              -- returns bool
reflex.window.is_focused("Notepad")          -- returns bool
reflex.window.focused()                      -- returns title of currently focused window
reflex.window.wait("Notepad", 5)             -- block until exists (timeout in seconds)

-- Object (for complex or multi-step work)
local win = reflex.window.find("Notepad")    -- returns a window object (partial match ok)
win:focus()
win:minimize()
win:maximize()
win:restore()
win:close()
win:move(100, 100, 800, 600)                 -- x, y, width, height
win:title()                                  -- returns the window title string
win:exists()                                 -- returns bool
```

---

## reflex.clipboard

Owns the system clipboard.

```lua
reflex.clipboard.get()                       -- returns clipboard contents as string
reflex.clipboard.set("some text")            -- write to clipboard
reflex.clipboard.clear()                     -- empty the clipboard
```

---

## reflex.timer

Owns repeating and delayed execution. `new()` returns a timer object you control directly.

```lua
local t = reflex.timer.new(1000, function()  -- create a timer, call fn every 1000ms
  print("tick")
end)

t:start()                                    -- start the timer
t:pause()                                    -- pause without destroying
t:resume()                                   -- resume a paused timer
t:clear()                                    -- stop and destroy

reflex.timer.once(2000, function()           -- fire-and-forget, no object needed
  print("done")
end)
```

---

## reflex.process

Owns process spawning and inspection.

```lua
reflex.process.spawn("notepad.exe")          -- launch a program
reflex.process.spawn("cmd.exe", "/c echo hi")-- launch with args
reflex.process.find("notepad.exe")           -- returns pid, or nil if not found
reflex.process.kill(pid)                     -- terminate by pid (precise)
reflex.process.pkill("notepad.exe")          -- terminate all by name (broad)
```

---

## Notes

- Key names are lowercase strings: `"ctrl"`, `"shift"`, `"alt"`, `"win"`, `"enter"`, `"space"`, `"f1"`–`"f24"`, etc.
- Combos are joined with `+`: `"ctrl+shift+t"`
- Mouse buttons: `"left"`, `"right"`, `"middle"`
- `reflex.window.find()` matches partial titles, case-insensitive
- All blocking calls (like `reflex.window.wait`) accept an optional timeout in seconds
