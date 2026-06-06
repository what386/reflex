-- String and table stdlib smoke test. Exits on its own.

local function assert_eq(actual, expected, label)
    if actual ~= expected then
        error(label .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
    end
end

local function assert_table_eq(actual, expected, label)
    assert_eq(#actual, #expected, label .. " length")
    for i = 1, #expected do
        assert_eq(actual[i], expected[i], label .. "[" .. i .. "]")
    end
end

assert_eq(reflex.str.trim("  hello  "), "hello", "trim")
assert_table_eq(reflex.str.split("a,b,c", ","), { "a", "b", "c" }, "split")
assert(reflex.str.starts_with("reflex", "ref"), "starts_with")
assert(reflex.str.ends_with("reflex", "lex"), "ends_with")
assert_eq(reflex.str.join({ "a", "b", "c" }, "-"), "a-b-c", "join")

local merged = reflex.table.merge({ a = 1 }, { b = 2 })
assert_eq(merged.a, 1, "merge a")
assert_eq(merged.b, 2, "merge b")

local deep = reflex.table.deep_merge({ a = { b = 1 }, keep = true }, { a = { c = 2 } })
assert_eq(deep.a.b, 1, "deep_merge nested b")
assert_eq(deep.a.c, 2, "deep_merge nested c")
assert_eq(deep.keep, true, "deep_merge keep")

assert(reflex.table.contains({ "x", "y" }, "y"), "contains")

local keys = reflex.table.keys({ alpha = 1, beta = 2 })
table.sort(keys)
assert_table_eq(keys, { "alpha", "beta" }, "keys")

local mapped = reflex.table.map({ 1, 2, 3 }, function(value)
    return value * 2
end)
assert_table_eq(mapped, { 2, 4, 6 }, "map")

local filtered = reflex.table.filter({ 1, 2, 3, 4 }, function(value)
    return value % 2 == 0
end)
assert_table_eq(filtered, { 2, 4 }, "filter")

print("stdlib: passed")
reflex.exit()
