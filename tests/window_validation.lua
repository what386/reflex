local ok, err = pcall(function()
  reflex.window.find(42)
end)
print("window_validation: find(42) accepted=" .. tostring(ok) .. " result=" .. tostring(err))

ok, err = pcall(function()
  reflex.window.find("%")
end)
print("window_validation: find('%') accepted=" .. tostring(ok) .. " result=" .. tostring(err))

ok, err = pcall(function()
  reflex.window.wait("anything", -1)
end)
print("window_validation: wait('anything', -1) accepted=" .. tostring(ok) .. " result=" .. tostring(err))

reflex.exit()
