.pragma library

var MAX_POPUP_WINDOWS = 64

// Read an own registry field through its descriptor so an accessor is invoked
// exactly once. Inherited fields are not registry authority.
function ownEntryValue(entry, key) {
    if (!entry || typeof entry !== "object")
        return undefined
    var descriptor = Object.getOwnPropertyDescriptor(entry, key)
    if (!descriptor)
        return undefined
    return typeof descriptor.get === "function"
            ? descriptor.get.call(entry) : descriptor.value
}

// Return an exact, bounded snapshot for process-shutdown traversal. A null
// result means ownership is unknown and callers must not treat it as empty.
function snapshotPopupHosts(rootWindow) {
    if (!rootWindow)
        return null

    try {
        var popups = rootWindow.popupWindowRegistry
        if (!Array.isArray(popups))
            return null

        var popupCount = popups.length
        if (popupCount > MAX_POPUP_WINDOWS)
            return null

        var hosts = []
        for (var i = 0; i < popupCount; ++i) {
            var entry = popups[i]
            var host = ownEntryValue(entry, "host")
            if (!host || hosts.indexOf(host) >= 0)
                return null
            hosts.push(host)
        }
        return hosts
    } catch (error) {
        return null
    }
}

// Return the minimal stable popup fields needed for cross-window routing.
// Unlike the shutdown snapshot this also inspects view and opener-UI
// references; callers must treat any unavailable field as unknown ownership.
function snapshotPopupEntriesFromRegistry(popups) {
    try {
        if (!Array.isArray(popups))
            return null

        var popupCount = popups.length
        if (popupCount > MAX_POPUP_WINDOWS)
            return null

        var entries = []
        var hosts = []
        for (var i = 0; i < popupCount; ++i) {
            var entry = popups[i]
            var host = ownEntryValue(entry, "host")
            if (!host || hosts.indexOf(host) >= 0)
                return null
            var view = ownEntryValue(entry, "view")
            var ui = ownEntryValue(entry, "ui")
            hosts.push(host)
            entries.push({ host: host, view: view || null, ui: ui || null })
        }
        return entries
    } catch (error) {
        return null
    }
}

function snapshotPopupEntries(rootWindow) {
    if (!rootWindow)
        return null

    try {
        return snapshotPopupEntriesFromRegistry(rootWindow.popupWindowRegistry)
    } catch (error) {
        return null
    }
}

// Refuse new popup ownership before allocation when the process-wide registry
// cannot be inspected safely or has exhausted the discovery bound.
function canRegisterPopupWindow(rootWindow) {
    if (!rootWindow)
        return false

    try {
        if (rootWindow.applicationShutdownInProgress === true
                || rootWindow.shutdownApproved === true)
            return false
        var popups = rootWindow.popupWindowRegistry
        if (!Array.isArray(popups) || popups.length >= MAX_POPUP_WINDOWS)
            return false

        var hosts = []
        for (var i = 0; i < popups.length; ++i) {
            var entry = popups[i]
            var host = ownEntryValue(entry, "host")
            if (!host || hosts.indexOf(host) >= 0)
                return false
            hosts.push(host)
        }
        return true
    } catch (error) {
        return false
    }
}

// Revalidate and publish the ownership record atomically after the popup
// object has been allocated. No property access on an externally supplied
// registry entry may escape this boundary: failure means the popup must not
// be opened and the restore owner remains retained.
function registerPopupWindow(rootWindow, hostWindow, openerUi, view) {
    if (!rootWindow || !hostWindow)
        return false

    try {
        if (rootWindow.applicationShutdownInProgress === true
                || rootWindow.shutdownApproved === true)
            return false
        var popups = rootWindow.popupWindowRegistry
        if (!Array.isArray(popups) || popups.length > MAX_POPUP_WINDOWS)
            return false

        var retained = []
        var hosts = []
        for (var i = 0; i < popups.length; ++i) {
            var entry = popups[i]
            var existingHost = ownEntryValue(entry, "host")
            if (!existingHost || hosts.indexOf(existingHost) >= 0)
                return false
            hosts.push(existingHost)
            if (existingHost !== hostWindow)
                retained.push(entry)
        }
        if (retained.length >= MAX_POPUP_WINDOWS)
            return false

        retained.push({
            host: hostWindow,
            ui: openerUi || rootWindow.primaryBrowserUi || null,
            view: view || null
        })
        rootWindow.popupWindowRegistry = retained
        return true
    } catch (error) {
        return false
    }
}

// Remove an exact popup owner only when the full bounded registry can be
// inspected and replaced. Any malformed entry or failed assignment leaves
// the old registry in place so shutdown remains fail-closed.
function unregisterPopupWindow(rootWindow, hostWindow) {
    if (!rootWindow || !hostWindow)
        return false

    try {
        var popups = rootWindow.popupWindowRegistry
        if (!Array.isArray(popups) || popups.length > MAX_POPUP_WINDOWS)
            return false

        var retained = []
        var hosts = []
        for (var i = 0; i < popups.length; ++i) {
            var entry = popups[i]
            var existingHost = ownEntryValue(entry, "host")
            if (!existingHost || hosts.indexOf(existingHost) >= 0)
                return false
            hosts.push(existingHost)
            if (existingHost !== hostWindow)
                retained.push(entry)
        }
        rootWindow.popupWindowRegistry = retained
        return true
    } catch (error) {
        return false
    }
}

// Keep an approved restore window alive while any process-wide popup entry
// still points back to that window's host or UI. Malformed registry state and
// property access failures are treated as outstanding ownership.
function hasOwnedPopupWindows(rootWindow, secondaryWindow, secondaryUi) {
    if (!rootWindow || !secondaryWindow || !secondaryUi)
        return true

    try {
        var popups = rootWindow.popupWindowRegistry
        if (!Array.isArray(popups) || popups.length > MAX_POPUP_WINDOWS)
            return true

        var hosts = []
        for (var i = 0; i < popups.length; ++i) {
            var entry = popups[i]
            var host = ownEntryValue(entry, "host")
            if (!host || hosts.indexOf(host) >= 0)
                return true
            hosts.push(host)
            var entryUi = ownEntryValue(entry, "ui")
            var permissionUi = host.popupPermissionUi
            if (host.popupOpenerHost === secondaryWindow
                    || entryUi === secondaryUi
                    || permissionUi === secondaryUi)
                return true
        }
        return false
    } catch (error) {
        return true
    }
}
