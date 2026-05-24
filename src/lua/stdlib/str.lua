reflex.str = reflex.str or {}

reflex.str.trim = function(s)
    if type(s) ~= "string" then
        error("s must be a string", 2)
    end
    return (s:gsub("^%s*(.-)%s*$", "%1"))
end

reflex.str.split = function(s, sep)
    if type(s) ~= "string" then
        error("s must be a string", 2)
    end
    if type(sep) ~= "string" then
        error("sep must be a string", 2)
    end

    local out = {}
    if sep == "" then
        for i = 1, #s do
            out[#out + 1] = s:sub(i, i)
        end
        return out
    end

    local start = 1
    while true do
        local i, j = s:find(sep, start, true)
        if not i then
            out[#out + 1] = s:sub(start)
            break
        end
        out[#out + 1] = s:sub(start, i - 1)
        start = j + 1
    end
    return out
end

reflex.str.starts_with = function(s, prefix)
    if type(s) ~= "string" then
        error("s must be a string", 2)
    end
    if type(prefix) ~= "string" then
        error("prefix must be a string", 2)
    end
    return s:sub(1, #prefix) == prefix
end

reflex.str.ends_with = function(s, suffix)
    if type(s) ~= "string" then
        error("s must be a string", 2)
    end
    if type(suffix) ~= "string" then
        error("suffix must be a string", 2)
    end
    if #suffix == 0 then
        return true
    end
    return s:sub(-#suffix) == suffix
end

reflex.str.join = function(t, sep)
    if type(t) ~= "table" then
        error("t must be a table", 2)
    end
    if type(sep) ~= "string" then
        error("sep must be a string", 2)
    end

    local parts = {}
    for i, value in ipairs(t) do
        parts[i] = tostring(value)
    end
    return table.concat(parts, sep)
end
