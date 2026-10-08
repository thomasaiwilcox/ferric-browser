// VB-060: activate only a typed line resolved by the source application.
// Native profile/view authority is rechecked by Rust and the QML lease.
function activate(parentUi, target, entries, ownerForToken, validateChild, activateHost) {
    if (!parentUi || !target || typeof target !== "string")
        return false
    var fields = target.split("\t")
    if (fields.length !== 2 || !fields[1])
        return false
    try {
        if (fields[0] === "parent") {
            var found = null
            for (var i = 0; i < entries.length; ++i) {
                var entry = entries[i]
                if (!entry || entry.ui !== parentUi
                        || entry.coreWindowId !== fields[1])
                    continue
                if (found || !entry.host || !entry.profile || !entry.view
                        || entry.view.profile !== entry.profile)
                    return false
                found = entry
            }
            return found ? !!activateHost(found.host, parentUi) : false
        }
        if (fields[0] !== "child"
                || !/^branch:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(fields[1]))
            return false
        var owner = ownerForToken(fields[1])
        if (!owner || !owner.host || !owner.profile || !owner.view
                || owner.host.windowBranchToken !== fields[1]
                || !owner.host.windowCleanBranchPublished
                || !owner.host.windowCleanBranchLineagePublished
                || owner.host.windowBranchProfile !== owner.profile
                || owner.host.windowBranchInitialView !== owner.view
                || owner.view.profile !== owner.profile
                || !owner.host.windowBranchUi
                || !validateChild(owner.host.windowBranchUi, fields[1]))
            return false
        return !!activateHost(owner.host, owner.host.windowBranchUi)
    } catch (error) {
        return false
    }
}
