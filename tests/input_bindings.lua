-- Manual input smoke test.
-- Run reflexd first, then run:
--   cargo run --bin reflex -- tests/input_bindings.lua

print("input_bindings: registering remaps and binds")

reflex.signal.connect("reflex::started", function()
    print("input_bindings: started")
    print("  ctrl+alt+t: type text")
    print("  ctrl+alt+k: send ctrl+c")
    print("  ctrl+alt+m: move/click/scroll mouse")
    print("  ctrl+back: send back")
    print("  forward: send forward")
    print("  ctrl+alt+q: exit")
end)

reflex.signal.connect("reflex::exiting", function()
    print("input_bindings: exiting")
end)

reflex.hotkey("capslock", "ctrl")
reflex.hotkey("t", "y")
reflex.hotkey("back", "forward")

reflex.bind("ctrl+alt+t", function()
    reflex.key.type("Hello from input_bindings.lua")
    reflex.key.send("enter")
end)

reflex.bind("ctrl+alt+k", function()
    reflex.key.down("ctrl")
    reflex.key.send("c")
    reflex.key.up("ctrl")
end)

reflex.bind("ctrl+alt+m", function()
    reflex.mouse.move(40, 20, "rel")
    reflex.mouse.click("left")
    reflex.mouse.scroll(-1)
end)

reflex.bind("ctrl+back", function()
    reflex.key.type("mouse back combo")
    reflex.key.send("enter")
end)

reflex.bind("forward", function()
    reflex.key.type("mouse forward bind")
    reflex.key.send("enter")
end)

reflex.bind("ctrl+alt+q", function()
    reflex.exit()
end)
