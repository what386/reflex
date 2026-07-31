reflex.table = reflex.table or {}

local function is_table(value, name)
    if type(value) ~= "table" then
        error(name .. " must be a table", 2)
    end
end

reflex.table.merge = function(t1, t2)
    is_table(t1, "t1")
    is_table(t2, "t2")

    local out = {}
    for k, v in pairs(t1) do
        out[k] = v
    end
    for k, v in pairs(t2) do
        out[k] = v
    end
    return out
end

reflex.table.deep_merge = function(t1, t2)
    is_table(t1, "t1")
    is_table(t2, "t2")

    local out = {}
    for k, v in pairs(t1) do
        if type(v) == "table" then
            out[k] = reflex.table.deep_merge(v, {})
        else
            out[k] = v
        end
    end
    for k, v in pairs(t2) do
        if type(v) == "table" and type(out[k]) == "table" then
            out[k] = reflex.table.deep_merge(out[k], v)
        elseif type(v) == "table" then
            out[k] = reflex.table.deep_merge(v, {})
        else
            out[k] = v
        end
    end
    return out
end

reflex.table.contains = function(t, value)
    is_table(t, "t")
    for _, item in ipairs(t) do
        if item == value then
            return true
        end
    end
    return false
end

reflex.table.keys = function(t)
    is_table(t, "t")
    local out = {}
    for k, _ in pairs(t) do
        out[#out + 1] = k
    end
    return out
end

reflex.table.map = function(t, fn)
    is_table(t, "t")
    if type(fn) ~= "function" then
        error("fn must be a function", 2)
    end

    local out = {}
    for k, v in pairs(t) do
        out[k] = fn(v, k)
    end
    return out
end

reflex.table.filter = function(t, fn)
    is_table(t, "t")
    if type(fn) ~= "function" then
        error("fn must be a function", 2)
    end

    local out = {}
    local seen = {}

    for i, v in ipairs(t) do
        seen[i] = true
        if fn(v, i) then
            out[#out + 1] = v
        end
    end

    for k, v in pairs(t) do
        if not seen[k] and fn(v, k) then
            out[k] = v
        end
    end
    return out
end
