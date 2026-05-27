# Reflex API Reference

Reflex scripts are plain Lua files. The `reflex` global is available everywhere; no imports are needed.

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
reflex.hotkey("capslock", "ctrl")           -- key becomes another key
reflex.sleep(500)                            -- pause for ms
reflex.exit()                                -- request clean shutdown
```

## reflex.key

```lua
reflex.key.type("Hello, World!")
reflex.key.send("ctrl+c")
reflex.key.down("shift")
reflex.key.up("shift")
```

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

## reflex.timer

```lua
local t = reflex.timer.new(1000, function()
  print("tick")
end)

t:start()
t:pause()
t:resume()
t:clear()

reflex.timer.once(2000, function()
  print("done")
end)
```

## reflex.process

```lua
reflex.process.spawn("kitty")
reflex.process.spawn("cmd.exe", "/c", "echo", "hi")
reflex.process.find("kitty")                 -- pid or nil
reflex.process.kill(pid)
reflex.process.pkill("kitty")
```

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
- V1 intentionally does not include clipboard, msgbox, path, or window APIs.
