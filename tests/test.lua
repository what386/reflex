-- reflex v1 smoke script

reflex.signal.connect("reflex::started", function()
    print("reflex started")
end)

reflex.signal.connect("reflex::exiting", function()
    print("reflex exiting")
end)

-- remaps
reflex.hotkey("capslock", "ctrl")
reflex.hotkey("t", "y")

-- binds
reflex.bind("ctrl+alt+t", function()
    reflex.key.send("u")
end)

reflex.bind("ctrl+alt+n", function()
    reflex.process.spawn("gedit")
end)

reflex.bind("ctrl+alt+f1", function()
    reflex.mouse.move(100, 100)
    reflex.sleep(100)
    reflex.mouse.click("left")
end)

local tick_count = 0
local ticker = reflex.timer.new(10000, function()
    tick_count = tick_count + 1
    print("ticker fired " .. tick_count .. " time(s)")
end)
ticker:start()

local ticking = true
reflex.bind("ctrl+alt+p", function()
    if ticking then
        ticker:pause()
        print("ticker paused")
    else
        ticker:resume()
        print("ticker resumed")
    end
    ticking = not ticking
end)

reflex.bind("ctrl+alt+x", function()
    local target = "gedit"
    local pid = reflex.process.find(target)
    if pid then
        reflex.process.kill(pid)
        print("killed " .. target)
    else
        print(target .. " is not running")
    end
end)

reflex.bind("ctrl+alt+q", function()
    ticker:clear()
    reflex.exit()
end)
