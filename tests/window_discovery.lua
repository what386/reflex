local windows = reflex.window.list()
print("window_discovery: found " .. #windows .. " window(s)")

for index, window in ipairs(windows) do
  print("window_discovery: [" .. index .. "] " .. window:id()
    .. " title=" .. window:title()
    .. " app_id=" .. tostring(window:app_id())
    .. " exists=" .. tostring(window:exists()))
end

local terminal = reflex.window.find("terminal")
print("window_discovery: title match='terminal' -> " .. tostring(terminal ~= nil))
if terminal then
  print("window_discovery: matched " .. terminal:id() .. " titled '" .. terminal:title() .. "'")
end

local editor = reflex.window.find(function(window)
  return window:app_id() ~= nil and window:app_id():lower():find("editor") ~= nil
end)
print("window_discovery: predicate app_id contains 'editor' -> " .. tostring(editor ~= nil))
if editor then
  print("window_discovery: predicate matched " .. editor:id())
end

print("window_discovery: exists('%.lua$') -> " .. tostring(reflex.window.exists("%.lua$")))
print("window_discovery: exists('missing') -> " .. tostring(reflex.window.exists("missing")))
print("window_discovery: wait('missing', 0) -> " .. tostring(reflex.window.wait("missing", 0)))

reflex.exit()
