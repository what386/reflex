-- reflex test script
-- tests most of the api surface. read the msgboxes as you go.

-- =========================================
-- signals
-- =========================================

reflex.signal.connect("reflex::started", function()
    reflex.msgbox("reflex started successfully.")
end)

reflex.signal.connect("reflex::exiting", function()
    reflex.msgbox("reflex is exiting. goodbye.")
end)

reflex.signal.connect("window::focused", function(win)
    print("window focused: " .. win:title())
end)

reflex.signal.connect("key::down", function(key)
    print("key down: " .. key)
end)

-- =========================================
-- hotkeys and hotstrings (remaps)
-- =========================================

-- capslock becomes ctrl
reflex.hotkey("capslock", "ctrl")

-- btw expands automatically
reflex.hotstring("btw", "by the way")

-- =========================================
-- bind (key combos -> functions)
-- =========================================

-- ctrl+alt+t: open a terminal
reflex.bind("ctrl+alt+t", function()
    reflex.process.spawn("xterm")
end)

-- ctrl+alt+n: open notepad (windows) or gedit (linux)
reflex.bind("ctrl+alt+n", function()
    if reflex.process.exists("explorer.exe") then
        reflex.process.spawn("notepad.exe")
    else
        reflex.process.spawn("gedit")
    end
end)

-- ctrl+alt+c: copy clipboard contents to a msgbox
reflex.bind("ctrl+alt+c", function()
    local contents = reflex.clipboard.get()
    if contents then
        reflex.msgbox("clipboard: " .. contents)
    else
        reflex.msgbox("clipboard is empty.")
    end
end)

-- ctrl+alt+w: report focused window
reflex.bind("ctrl+alt+w", function()
    local title = reflex.window.focused()
    reflex.msgbox("focused window: " .. (title or "none"))
end)

-- =========================================
-- bindstring (typed strings -> functions)
-- =========================================

-- typing "reflextest" triggers a message
reflex.bindstring("reflextest", function()
    reflex.msgbox("bindstring works!")
end)

-- =========================================
-- mouse
-- =========================================

-- ctrl+alt+m: print current mouse position
reflex.bind("ctrl+alt+m", function()
    local x, y = reflex.mouse.pos()
    reflex.msgbox("mouse is at: " .. x .. ", " .. y)
end)

-- ctrl+alt+click: move mouse to 100,100 and click
reflex.bind("ctrl+alt+f1", function()
    reflex.mouse.move(100, 100)
    reflex.sleep(100)
    reflex.mouse.click("left")
end)

-- =========================================
-- timers
-- =========================================

-- fires once after 3 seconds
reflex.timer.once(3000, function()
    reflex.msgbox("timer.once fired after 3 seconds.")
end)

-- repeating timer, announced every 10 seconds
local tick_count = 0
local ticker = reflex.timer.new(10000, function()
    tick_count = tick_count + 1
    print("ticker fired " .. tick_count .. " time(s)")
end)
ticker:start()

-- ctrl+alt+p: pause/resume the ticker
local ticking = true
reflex.bind("ctrl+alt+p", function()
    if ticking then
        ticker:pause()
        reflex.msgbox("ticker paused.")
    else
        ticker:resume()
        reflex.msgbox("ticker resumed.")
    end
    ticking = not ticking
end)

-- =========================================
-- window management
-- =========================================

-- ctrl+alt+z: minimize focused window
reflex.bind("ctrl+alt+z", function()
    local title = reflex.window.focused()
    if title then
        reflex.window.minimize(title)
    end
end)

-- =========================================
-- process
-- =========================================

-- ctrl+alt+x: kill notepad/gedit if running
reflex.bind("ctrl+alt+x", function()
    local target = "notepad.exe"
    local pid = reflex.process.find(target)
    if pid then
        reflex.process.kill(pid)
        reflex.msgbox("killed " .. target)
    else
        reflex.msgbox(target .. " is not running.")
    end
end)

-- =========================================
-- escape hatch
-- =========================================

-- ctrl+alt+q: cleanly exit reflex
reflex.bind("ctrl+alt+q", function()
    ticker:clear()
    reflex.msgbox("exiting reflex.")
    reflex.exit()
end)
