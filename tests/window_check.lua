local windows = reflex.window.list()
print("window_check: list returned " .. #windows .. " window(s)")
print("window_check: find('anything') -> " .. tostring(reflex.window.find("anything")))
print("window_check: exists('anything') -> " .. tostring(reflex.window.exists("anything")))
print("window_check: wait('anything', 0) -> " .. tostring(reflex.window.wait("anything", 0)))

reflex.exit()
