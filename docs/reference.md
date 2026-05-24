# Reflex Reference

---

## Signals

All built-in signals follow `domain::event` naming. Connect to them with `reflex.signal.connect()`.

### Window signals
| Signal | Callback args | Notes |
|---|---|---|
| `window::opened` | `win` | Fires when any window appears |
| `window::closed` | `win` | Fires when any window closes |
| `window::focused` | `win` | Window gained focus |
| `window::unfocused` | `win` | Window lost focus |
| `window::moved` | `win, x, y` | Window was moved |
| `window::resized` | `win, w, h` | Window was resized |
| `window::title_changed` | `win, title` | Window title changed |
| `window::minimized` | `win` | |
| `window::maximized` | `win` | |
| `window::restored` | `win` | Un-minimized or un-maximized |

### Screen signals
| Signal | Callback args | Notes |
|---|---|---|
| `screen::changed` | | Resolution changed |
| `screen::connected` | | Monitor plugged in |
| `screen::disconnected` | | Monitor unplugged |

### Mouse signals
| Signal | Callback args | Notes |
|---|---|---|
| `mouse::moved` | `x, y` | Fires as cursor moves |
| `mouse::down` | `button, x, y` | Any button pressed |
| `mouse::up` | `button, x, y` | Any button released |
| `mouse::scrolled` | `delta, x, y` | `delta` is -1 or 1 |

### Key signals
| Signal | Callback args | Notes |
|---|---|---|
| `key::down` | `key` | Any key pressed |
| `key::up` | `key` | Any key released |

### Process signals
| Signal | Callback args | Notes |
|---|---|---|
| `process::spawned` | `name, pid` | Any process started |
| `process::exited` | `name, pid, code` | Any process exited |

### Reflex signals
| Signal | Callback args | Notes |
|---|---|---|
| `reflex::started` | | Script has started |
| `reflex::exiting` | | Script is about to exit |

---

## Key Names

### Modifiers
| Name | Key |
|---|---|
| `ctrl` | Control |
| `shift` | Shift |
| `alt` | Alt / Option |
| `win` | Windows / Super / Command |
| `altgr` | AltGr (Right Alt) |

### Common Keys
| Name | Key |
|---|---|
| `enter` | Enter / Return |
| `space` | Space |
| `tab` | Tab |
| `backspace` | Backspace |
| `delete` | Delete |
| `escape` | Escape |
| `insert` | Insert |
| `home` | Home |
| `end` | End |
| `pageup` | Page Up |
| `pagedown` | Page Down |

### Arrow Keys
`up`, `down`, `left`, `right`

### Function Keys
`f1` through `f24`

### Numpad
| Name | Key |
|---|---|
| `num0`–`num9` | Numpad digits |
| `num_add` | Numpad + |
| `num_sub` | Numpad - |
| `num_mul` | Numpad * |
| `num_div` | Numpad / |
| `num_decimal` | Numpad . |
| `num_enter` | Numpad Enter |
| `numlock` | Num Lock |

### Media Keys
| Name | Key |
|---|---|
| `media_play` | Play / Pause |
| `media_stop` | Stop |
| `media_next` | Next Track |
| `media_prev` | Previous Track |
| `vol_up` | Volume Up |
| `vol_down` | Volume Down |
| `mute` | Mute |

### Misc
| Name | Key |
|---|---|
| `capslock` | Caps Lock |
| `scrolllock` | Scroll Lock |
| `pause` | Pause / Break |
| `printscreen` | Print Screen |
| `menu` | Application / Menu key |
| `sleep` | Sleep key |

---

## Mouse Button Names

Used in `mouse.click()`, `mouse.down()`, `mouse.up()`, and in signal callbacks.

| Name | Button |
|---|---|
| `left` | Left click |
| `right` | Right click |
| `middle` | Middle click / scroll wheel click |
| `mouse4` | Back (thumb button 1) |
| `mouse5` | Forward (thumb button 2) |
| `mouse6`–`mouse9` | Extra buttons (hardware dependent) |

---

## Combos

Combos are expressed as `+`-separated key names, modifiers first:

```lua
"ctrl+c"
"ctrl+shift+t"
"alt+f4"
"win+left"
"ctrl+alt+delete"
```

Order within modifiers doesn't matter — `ctrl+shift` and `shift+ctrl` are the same.

---

## Pattern Matching

`reflex.window.find()` and flat window functions accept partial, case-insensitive title matches. You can also pass a Lua pattern:

```lua
reflex.window.find("notepad")           -- matches "Notepad", "Notepad++" etc.
reflex.window.find("%.lua$")            -- Lua pattern: title ending in .lua
reflex.window.find(function(win)        -- predicate function for full control
    return win:title():match("%.lua$") and win:exists()
end)
```
