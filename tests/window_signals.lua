reflex.signal.connect("window::opened", function(window)
  print("window_signals: opened id=" .. window:id()
    .. " title='" .. window:title() .. "' app_id=" .. tostring(window:app_id()))
end)

reflex.signal.connect("window::title_changed", function(window, title)
  print("window_signals: title_changed id=" .. window:id() .. " title='" .. title .. "'")
end)

reflex.signal.connect("window::closed", function(window)
  print("window_signals: closed id=" .. window:id()
    .. " exists=" .. tostring(window:exists()))
end)

print("window_signals: listening for opened, title_changed, and closed events")
