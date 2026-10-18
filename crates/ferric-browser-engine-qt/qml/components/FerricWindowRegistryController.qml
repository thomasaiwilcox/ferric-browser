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

    function switcherEntryLive(entry) {
        return !!entry && !!entry.ui && !!entry.host && !!entry.profile
                && entry.host.runtimeReleased !== true && entry.host.runtimeDisposing !== true
                && entry.ui.profile_bootstrap_pending !== true
    }

    function switcherInvocation() {
        var root = controller.browserWindow
        var token = root.switcherOwnerToken
        if (!token || !root.switcherUi || !root.switcherHost || !root.switcherProfile) return null
        return root.liveWindowEntries().find(function(entry) {
            return controller.switcherEntryLive(entry) && entry.ui === root.switcherUi
                    && entry.host === root.switcherHost && entry.profile === root.switcherProfile
                    && String(entry.ui.window_token) === token
        }) || null
    }

    function switcherEntryTransient(entry) {
        return !!entry.privateProfile || !!entry.ephemeralProfile || !!entry.profile.offTheRecord
    }

    function switcherEntries() {
        var invocation = controller.switcherInvocation()
        if (!invocation) return []
        var transient = controller.switcherEntryTransient(invocation)
        return controller.browserWindow.liveWindowEntries().filter(function(entry) {
            if (!controller.switcherEntryLive(entry)) return false
            if (!transient) return !controller.switcherEntryTransient(entry)
            return controller.switcherEntryTransient(entry)
                    && entry.profile === invocation.profile
                    && !!entry.ephemeralProfile === !!invocation.ephemeralProfile
                    && (!invocation.ephemeralProfile
                        || entry.ephemeralInvocationToken === invocation.ephemeralInvocationToken)
        })
    }

    function switcherOwner(result) {
        var token = result && result.owner_token ? String(result.owner_token) : ""
        if (!token) return null
        return controller.switcherEntries().find(function(entry) {
            return String(entry.ui.window_token) === token
        }) || null
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
