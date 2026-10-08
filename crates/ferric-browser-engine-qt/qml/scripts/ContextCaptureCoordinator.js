.pragma library

// The registry is the UI's live owner inventory; Rust rechecks every staged
// profile/context/window and requires every originating owner to confirm the
// immutable member before the source writes the profile-local registry.
function saveContextMembership(sourceUi, sourceEntry, entries) {
    if (!sourceUi || !sourceEntry || sourceEntry.ui !== sourceUi
            || sourceEntry.privateProfile || sourceEntry.ephemeralProfile) {
        if (sourceUi)
            sourceUi.status_text = "Context source window is unavailable"
        return false
    }
    var contextName = String(sourceUi.context_name || "")
    if (!contextName) {
        sourceUi.status_text = "Current window is not in a context"
        return false
    }
    var owners = []
    var seen = ({})
    var sourceFound = false
    for (var i = 0; i < entries.length; ++i) {
        var entry = entries[i]
        if (!entry || !entry.ui || !entry.host
                || String(entry.ui.context_name || "") !== contextName
                || String(entry.profileName || "")
                        !== String(sourceEntry.profileName || "")
                || entry.privateProfile || entry.ephemeralProfile) {
            continue
        }
        var id = String(entry.coreWindowId || "")
        var token = String(entry.windowToken || "")
        if (!id || !token || seen[id]
                || String(entry.ui.core_window_id || "") !== id
                || String(entry.ui.window_token || "") !== token) {
            sourceUi.status_text = "Context member owner is ambiguous or stale"
            return false
        }
        seen[id] = true
        sourceFound = sourceFound || entry.ui === sourceUi
        owners.push(entry)
    }
    if (!sourceFound || owners.length === 0 || owners.length > 256) {
        sourceUi.status_text = "Context source membership is unavailable"
        return false
    }
    var handles = []
    for (var memberIndex = 0; memberIndex < owners.length; ++memberIndex) {
        var handle = String(owners[memberIndex].ui.stage_context_window_membership(
                                contextName) || "")
        if (!handle) {
            sourceUi.status_text = "Context member save capture was refused"
            return false
        }
        handles.push(handle)
    }
    var planId = String(sourceUi.prepare_staged_context_save(
                            contextName, handles) || "")
    if (!planId)
        return false
    for (var confirmIndex = 0; confirmIndex < handles.length; ++confirmIndex) {
        if (!owners[confirmIndex].ui.confirm_staged_context_window_membership(
                    handles[confirmIndex])) {
            sourceUi.cancel_staged_context_save(planId)
            sourceUi.status_text = "Context member changed during save"
            return false
        }
    }
    if (!sourceUi.finish_staged_context_save(planId)) {
        sourceUi.cancel_staged_context_save(planId)
        sourceUi.status_text = "Context membership could not be saved"
        return false
    }
    sourceEntry.host.scheduleRuntimeWork(0)
    return true
}

// Only exact live owner handles cross QML. Saved members, structural captures,
// identity bindings, and the all-owner receipt check remain in Rust.
function commitSavedContext(sourceUi, sourceEntry, entries) {
    if (!sourceUi) {
        return false
    }
    var contextName = String(sourceUi.context_name || "")
    var contextRef = String(sourceUi.active_context_capture_ref(contextName) || "")
    var members = sourceUi.active_context_capture_members(contextName)
    var message = String(sourceUi.take_context_commit_message() || "")
    if (!contextName || !contextRef || !message || !members
            || members.length === 0 || members.length > 256) {
        sourceUi.status_text = "Saved context capture is unavailable"
        return false
    }
    if (!sourceEntry || sourceEntry.ui !== sourceUi
            || sourceEntry.privateProfile || sourceEntry.ephemeralProfile) {
        sourceUi.status_text = "Context source window is unavailable"
        return false
    }
    var owners = []
    for (var ordinal = 0; ordinal < members.length; ++ordinal) {
        var memberId = String(members[ordinal] || "")
        var owner = null
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (!entry || !entry.ui || !entry.host
                    || String(entry.coreWindowId || "") !== memberId) {
                continue
            }
            if (owner !== null) {
                sourceUi.status_text = "Context member owner is ambiguous"
                return false
            }
            owner = entry
        }
        if (!owner || String(owner.ui.core_window_id || "") !== memberId
                || String(owner.ui.window_token || "")
                        !== String(owner.windowToken || "")
                || owner.privateProfile || owner.ephemeralProfile
                || String(owner.profileName || "")
                        !== String(sourceEntry.profileName || "")) {
            sourceUi.status_text = "Context member window is unavailable"
            return false
        }
        owners.push(owner)
    }
    var handles = []
    for (var captureIndex = 0; captureIndex < owners.length; ++captureIndex) {
        var handle = String(owners[captureIndex].ui.stage_context_window_capture(
                                contextName, contextRef, captureIndex) || "")
        if (!handle) {
            sourceUi.status_text = "Context member capture was refused"
            return false
        }
        handles.push(handle)
    }
    var planId = String(sourceUi.prepare_staged_context_commit(handles, message) || "")
    if (!planId) {
        return false
    }
    for (var memberIndex = 0; memberIndex < handles.length; ++memberIndex) {
        if (!owners[memberIndex].ui.bind_staged_context_window_capture(
                    handles[memberIndex])) {
            sourceUi.cancel_staged_context_commit(planId)
            sourceUi.status_text = "Context member changed during capture"
            return false
        }
        owners[memberIndex].host.scheduleRuntimeWork(0)
    }
    var submitted = sourceUi.finish_staged_context_commit(planId)
    if (!submitted || submitted.length !== 2) {
        sourceUi.cancel_staged_context_commit(planId)
        sourceUi.status_text = "Context capture could not be published"
        return false
    }
    sourceEntry.host.scheduleRuntimeWork(0)
    return true
}
