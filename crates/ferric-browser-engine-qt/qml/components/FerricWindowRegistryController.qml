import QtQuick
import QtWebEngine

// Owns transient profile tokens associated with registered windows. The
// registry itself remains on the composition root because browser-window
// factories consume it, but token lifetime is isolated here.
QtObject {
    id: controller

    required property var browserWindow
    required property var browserUi
    property var ephemeralProfileOwners: ({})
    // Clean branches are exact native profile/view leases, separate from the
    // invocation-token lookup used by ordinary ephemeral windows. One branch
    // may never borrow another branch's or the parent's profile instance.
    property var cleanBranchOwners: ({})

    function retainCleanBranchOwner(token, host, profile, view) {
        var requested = String(token || "")
        if (!/^branch:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(requested)
                || !host || !profile || !view)
            return false
        try {
            if (host.windowBranchToken !== requested
                    || host.windowCleanBranch !== true
                    || profile === controller.browserWindow.browserProfile
                    || host.windowSharedProfile
                    || profile.offTheRecord !== true
                    || String(profile.storageName || "") !== ""
                    || profile.persistentCookiesPolicy
                       !== WebEngineProfile.NoPersistentCookies
                    || profile.httpCacheType !== WebEngineProfile.MemoryHttpCache
                    || profile.persistentPermissionsPolicy
                       !== WebEngineProfile.AskEveryTime
                    || profile.isPushServiceEnabled !== false
                    || view.profile !== profile)
                return false
            var owners = controller.cleanBranchOwners
            if (!owners || typeof owners !== "object" || Array.isArray(owners)
                    || Object.keys(owners).length >= 32 || owners[requested])
                return false
            for (var key in owners) {
                if (!Object.prototype.hasOwnProperty.call(owners, key))
                    continue
                var existing = owners[key]
                if (!existing || existing.profile === profile
                        || existing.host === host || existing.view === view)
                    return false
            }
            owners[requested] = { host: host, profile: profile, view: view }
            controller.cleanBranchOwners = owners
            return owners[requested].host === host
                    && owners[requested].profile === profile
                    && owners[requested].view === view
        } catch (error) {
            return false
        }
    }

    function cleanBranchOwner(token) {
        var requested = String(token || "")
        var owners = controller.cleanBranchOwners
        if (!owners || typeof owners !== "object" || Array.isArray(owners))
            return null
        if (!Object.prototype.hasOwnProperty.call(owners, requested))
            return null
        var owner = owners[requested]
        try {
            return owner && owner.host && owner.profile && owner.view
                    && owner.host.windowBranchToken === requested
                    && owner.host.windowCleanBranch === true
                    && !owner.host.windowSharedProfile
                    && owner.profile.offTheRecord === true
                    && owner.view.profile === owner.profile
                    ? owner : null
        } catch (error) {
            return null
        }
    }

    function releaseCleanBranchOwner(token, host, profile, view) {
        var requested = String(token || "")
        var owners = controller.cleanBranchOwners
        if (!owners || typeof owners !== "object" || Array.isArray(owners)
                || !Object.prototype.hasOwnProperty.call(owners, requested))
            return false
        var owner = owners[requested]
        if (!owner || owner.host !== host || owner.profile !== profile
                || owner.view !== view)
            return false
        delete owners[requested]
        controller.cleanBranchOwners = owners
        return true
    }

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
