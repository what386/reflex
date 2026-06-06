-- Clipboard and notification smoke test.
-- This mutates the desktop clipboard and sends a notification.

local text = "reflex clipboard smoke"

reflex.clipboard.set(text)
local got = reflex.clipboard.get()
assert(got == text or got == text .. "\n", "clipboard get should return text that was set")

reflex.notify("Reflex smoke test", "clipboard_notify.lua completed")

reflex.notify({
    title = "Reflex notification options",
    body = "urgency, timeout, icon, and app_name parsed correctly",
    urgency = "low",
    timeout = 3000,
    icon = "dialog-information",
    app_name = "reflex",
})

reflex.clipboard.clear()
print("clipboard_notify: passed")
reflex.exit()
