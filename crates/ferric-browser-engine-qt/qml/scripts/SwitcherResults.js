.pragma library

// Length-prefixed scalar keys are local UI bookkeeping, never bridge payloads.
function key(parts) {
    return parts.map(function(value) {
        var text = String(value)
        return text.length + ":" + text
    }).join("")
}
function resultKey(value, index) {
    return key([String(value.owner_token || ""), String(value.kind || ""),
        value.id === undefined ? "row:" + key([value.label || "", index]) : "id:" + String(value.id)])
}
function activationKey(value, index) {
    return key([resultKey(value, index), value.generation === undefined ? "" : value.generation])
}
function samePresentation(left, right) {
    var fields = ["kind", "id", "generation", "owner_token", "label", "secondary",
        "profile", "workspace", "rank", "recency"]
    for (var i = 0; i < fields.length; ++i)
        if (left[fields[i]] !== right[fields[i]]) return false
    return true
}

function identity(row) {
    var kind = String(row.kind || "")
    if (kind === "command" || kind === "action" || kind === "context")
        return key([kind, String(row.id)])
    var catalog = kind === "bookmark" || kind === "quickmark" || kind === "history"
        || kind === "session" || kind === "download"
    return key([kind, String(catalog ? row.profile : row.owner_token), String(row.id)])
}

function compare(left, right) {
    return Number(right.rank || 0) - Number(left.rank || 0)
        || Number(right.recency || 0) - Number(left.recency || 0)
        || String(left.kind || "").localeCompare(String(right.kind || ""))
        || String(left.id || "").localeCompare(String(right.id || ""))
        || String(left.owner_token || "").localeCompare(String(right.owner_token || ""))
}

function publish(current, previous, pendingOwners, maximum) {
    // A source batch replaces only the owners it has actually queried. Keep
    // the same-query rows of other live owners until their replacement arrives.
    var values = current.concat(previous.filter(function(row) {
        return pendingOwners.indexOf(String(row.owner_token)) >= 0
    }))
    var seen = Object.create(null)
    var unique = []
    for (var i = 0; i < values.length; ++i) {
        var key = identity(values[i])
        if (seen[key]) continue
        seen[key] = true
        unique.push(values[i])
    }
    return unique.sort(compare).slice(0, Math.max(0, maximum))
}
