reflex.path = reflex.path or {}

local sep = (reflex.project and type(reflex.project.dir) == "string"
    and reflex.project.dir:find("\\", 1, true)) and "\\" or "/"

local function trim_segment(value, is_first, is_last)
    local out = value
    if not is_first then
        out = out:gsub("^[\\/]+", "")
    end
    if not is_last then
        out = out:gsub("[\\/]+$", "")
    end
    return out
end

reflex.path.join = function(...)
    local parts = { ... }
    if #parts == 0 then
        return ""
    end

    local out = {}
    for i, part in ipairs(parts) do
        if type(part) ~= "string" then
            error("path segment must be a string", 2)
        end
        if part ~= "" then
            out[#out + 1] = trim_segment(part, i == 1, i == #parts)
        end
    end
    return table.concat(out, sep)
end

reflex.path.basename = function(path)
    if type(path) ~= "string" then
        error("path must be a string", 2)
    end
    local clean = path:gsub("[\\/]+$", "")
    local name = clean:match("([^\\/]+)$")
    return name or ""
end

reflex.path.stem = function(path)
    local base = reflex.path.basename(path)
    local idx = base:match("^.*()%.")
    if not idx or idx == 1 then
        return base
    end
    return base:sub(1, idx - 1)
end

reflex.path.ext = function(path)
    local base = reflex.path.basename(path)
    local idx = base:match("^.*()%.")
    if not idx or idx == 1 or idx == #base then
        return ""
    end
    return base:sub(idx + 1)
end
