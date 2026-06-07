-- Signals and timers smoke test. Exits on its own.

local function assert_eq(actual, expected, label)
    if actual ~= expected then
        error(label .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
    end
end

local custom_count = 0
local removed_count = 0

local function removed_handler()
    removed_count = removed_count + 1
end

reflex.signal.connect("custom::event", function(a, b)
    custom_count = custom_count + 1
    assert_eq(a, "alpha", "first signal argument")
    assert_eq(b, 42, "second signal argument")
end)

reflex.signal.connect("custom::removed", removed_handler)
reflex.signal.disconnect("custom::removed", removed_handler)

reflex.signal.emit("custom::event", "alpha", 42)
reflex.signal.emit("custom::removed")

assert_eq(custom_count, 1, "custom signal count")
assert_eq(removed_count, 0, "disconnected signal count")

local ticks = 0
local ticker = reflex.timer.new(20, function()
    ticks = ticks + 1
    print("signals_timers: repeating tick " .. ticks)
end)

ticker:start()

reflex.timer.after(70, function()
    ticker:pause()
    assert(ticks > 0, "repeating timer should have fired before pause")
    ticker:resume()
end)

reflex.timer.after(130, function()
    ticker:clear()
    assert(ticks > 0, "repeating timer should have fired before clear")
    print("signals_timers: passed")
    reflex.exit()
end)
