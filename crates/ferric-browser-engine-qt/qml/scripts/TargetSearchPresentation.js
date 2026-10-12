.pragma library

function identity(candidate) {
    if (!candidate || typeof candidate !== "object") return ""
    var related = candidate.related_element_id === null
            || typeof candidate.related_element_id === "undefined"
            ? null : Number(candidate.related_element_id)
    return JSON.stringify([
        String(candidate.frame_path || ""), Number(candidate.element_id || 0), related
    ])
}

function meaning(candidate) {
    var source = JSON.stringify([
        String(candidate.kind || ""), candidate.href === null ? "" : String(candidate.href || ""),
        candidate.related_element_id === null
                || typeof candidate.related_element_id === "undefined"
            ? null : Number(candidate.related_element_id)
    ])
    var hash = 2166136261
    for (var index = 0; index < source.length; ++index) {
        hash ^= source.charCodeAt(index)
        hash = Math.imul(hash, 16777619)
    }
    return (hash >>> 0).toString(16)
}

function rows(hints, labels) {
    var ordered = []
    if (!Array.isArray(hints) || !Array.isArray(labels)) return ordered
    for (var labelIndex = 0; labelIndex < labels.length; ++labelIndex) {
        var label = String(labels[labelIndex])
        for (var hintIndex = 0; hintIndex < hints.length; ++hintIndex) {
            var hint = hints[hintIndex]
            if (String(hint.label) !== label) continue
            var candidate = hint.candidate || hint
            ordered.push({
                label: label,
                kind: String(candidate.kind || "target"),
                text: String(candidate.text || "Visible target"),
                identity: identity(candidate),
                meaning: meaning(candidate)
            })
            break
        }
    }
    return ordered
}

function reconcile(currentRows, selection) {
    var next = {
        identity: selection && selection.identity || "",
        meaning: selection && selection.meaning || "",
        initialized: !!(selection && selection.initialized)
    }
    if (!Array.isArray(currentRows)) currentRows = []
    if (!next.initialized && currentRows.length > 0) {
        next.identity = currentRows[0].identity
        next.meaning = currentRows[0].meaning
        next.initialized = true
        return next
    }
    if (next.identity.length > 0) {
        for (var index = 0; index < currentRows.length; ++index) {
            if (currentRows[index].identity === next.identity
                    && currentRows[index].meaning === next.meaning) {
                return next
            }
        }
    }
    next.identity = ""
    next.meaning = ""
    return next
}

function select(currentRows, selectionIdentity) {
    if (!Array.isArray(currentRows)) return null
    for (var index = 0; index < currentRows.length; ++index) {
        if (currentRows[index].identity === selectionIdentity) {
            return {
                identity: currentRows[index].identity,
                meaning: currentRows[index].meaning,
                initialized: true
            }
        }
    }
    return null
}

function move(currentRows, selection, delta) {
    if (!Array.isArray(currentRows) || currentRows.length === 0) return null
    var current = -1
    for (var index = 0; index < currentRows.length; ++index) {
        if (currentRows[index].identity === selection.identity
                && currentRows[index].meaning === selection.meaning) {
            current = index
            break
        }
    }
    var next = current < 0
            ? (delta < 0 ? currentRows.length - 1 : 0)
            : (current + delta + currentRows.length) % currentRows.length
    return select(currentRows, currentRows[next].identity)
}

function selectedRow(currentRows, selection) {
    if (!Array.isArray(currentRows)) return null
    for (var index = 0; index < currentRows.length; ++index) {
        if (currentRows[index].identity === selection.identity
                && currentRows[index].meaning === selection.meaning) {
            return currentRows[index]
        }
    }
    return null
}
