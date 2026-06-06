-- Process API smoke test. Spawns a short-lived helper and kills it.

local pid = reflex.process.spawn("sleep", "30")
assert(type(pid) == "number", "spawn should return a pid")
print("process: spawned sleep pid " .. pid)

local found = reflex.process.find("sleep")
assert(found ~= nil, "find should see a sleep process")
print("process: found sleep pid " .. found)

reflex.process.kill(pid)
print("process: killed spawned sleep")

local killed = reflex.process.pkill("__reflex_process_test_no_match__")
assert(killed == 0, "pkill on a missing process should return 0")

print("process: passed")
reflex.exit()
