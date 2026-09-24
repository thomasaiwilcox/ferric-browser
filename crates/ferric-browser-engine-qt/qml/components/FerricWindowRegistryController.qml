import QtQuick

// Owns transient profile tokens associated with registered windows. The
// registry itself remains on the composition root because browser-window
// factories consume it, but token lifetime is isolated here.
QtObject {
    id: controller

    required property var browserWindow
    required property var browserUi
    property var ephemeralProfileOwners: ({})

    function retainEphemeralProfileOwner(profile, token) {
        var requested = String(token || "")
        if (!profile || requested.length === 0) {
            return false
        }
        var owners = controller.ephemeralProfileOwners || ({})
        var current = owners[requested]
        if (current && current.profile && current.profile !== profile
                && Number(current.count || 0) > 0) {
            return false
        }
        owners[requested] = {
            profile: profile,
            count: Number(current && current.count || 0) + 1
        }
        controller.ephemeralProfileOwners = owners
        return true
    }

    function releaseEphemeralProfileOwner(profile, token) {
        var requested = String(token || "")
        if (!profile || requested.length === 0) {
            return false
        }
        var owners = controller.ephemeralProfileOwners || ({})
        var current = owners[requested]
        if (!current || current.profile !== profile) {
            return false
        }
        var count = Number(current.count || 0) - 1
        if (count > 0) {
            owners[requested] = { profile: profile, count: count }
        } else {
            delete owners[requested]
        }
        controller.ephemeralProfileOwners = owners
        return true
    }

    function switcherOwner(result) {
        var ownerToken = result && result.owner_token ? String(result.owner_token) : ""
        var entries = controller.browserWindow.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i] && entries[i].ui
                    && String(entries[i].ui.window_token) === ownerToken) {
                return entries[i]
            }
        }
        return { host: controller.browserWindow, ui: controller.browserUi }
    }

    function ephemeralProfileForToken(token) {
        var requested = String(token || "")
        if (requested.length === 0) {
            return null
        }
        var owner = (controller.ephemeralProfileOwners || ({}))[requested]
        if (owner && owner.profile && Number(owner.count || 0) > 0) {
            return owner.profile
        }
        var entries = controller.browserWindow.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.ephemeralProfile
                    && entry.profile
                    && String(entry.ephemeralInvocationToken || "") === requested) {
                return entry.profile
            }
        }
        return null
    }

    function ephemeralTokenForUi(ui) {
        var entries = controller.browserWindow.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.ui === ui && entry.ephemeralProfile) {
                return String(entry.ephemeralInvocationToken || "")
            }
        }
        return ""
    }
}
