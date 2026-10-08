import QtQuick
import "../scripts/PopupWindowOwnership.js" as PopupWindowOwnership

// Strong, bounded ownership bridges createObject(null) and asynchronous profile
// bootstrap. A visible native window alone does not retain its QML wrapper.
QtObject {
    id: owner
    readonly property int maximumWindows: 32
    property var hostWindow: null
    // Every restore descendant shares the entry window's bounded ownership
    // budget, even when a restore window itself opens further restore windows.
    property var parentOwner: null
    property var windows: []
    signal discoveryChanged()

    function rootOwner() {
        var current = owner
        var seen = []
        for (var depth = 0; depth < 64; ++depth) {
            if (!current || seen.indexOf(current) >= 0)
                return null
            seen.push(current)
            if (!current.parentOwner)
                return current
            current = current.parentOwner
        }
        return null
    }

    function cancelRootShutdown() {
        var root = rootOwner()
        if (!root || !root.hostWindow
                || !root.hostWindow.applicationShutdownInProgress
                || typeof root.hostWindow.abortApplicationShutdown !== "function")
            return false
        root.hostWindow.abortApplicationShutdown()
        return true
    }

    function totalOwnedWindows(limit) {
        var bound = Math.max(1, Math.min(64, Number(limit) || 1))
        var pending = [owner]
        var seen = []
        var count = 0
        for (var i = 0; i < pending.length && i < bound; ++i) {
            var current = pending[i]
            if (!current || seen.indexOf(current) >= 0)
                return bound
            seen.push(current)
            var children = current.windows || []
            for (var j = 0; j < children.length; ++j) {
                var child = children[j]
                if (!child)
                    return bound
                ++count
                if (count >= bound)
                    return bound
                if (child.versioningWindowOwner)
                    pending.push(child.versioningWindowOwner)
            }
        }
        return i < pending.length ? bound : count
    }

    // Qt Quick page-save callbacks have no safe per-view origin in every
    // lifecycle state. Serialize Ferric-authorized MHTML saves over the
    // bounded same-profile owner tree and correlate the sole outstanding
    // request by its one-use private staging path.
    function activePageCaptureHost(profile) {
        var root = rootOwner()
        if (!root || !profile || !root.hostWindow)
            return undefined
        var pending = [root.hostWindow]
        var seen = []
        var match = null
        for (var i = 0; i < pending.length; ++i) {
            if (i >= root.maximumWindows + 1)
                return undefined
            var host = pending[i]
            if (!host || seen.indexOf(host) >= 0)
                return undefined
            seen.push(host)
            var capture = host.nativePageCaptureSave
            if (host.browserProfile === profile && capture
                    && capture.profile === profile) {
                if (match && match !== host)
                    return undefined
                match = host
            }
            var childOwner = host.versioningWindowOwner
            if (childOwner) {
                var children = childOwner.windows
                if (!Array.isArray(children)
                        || pending.length + children.length > root.maximumWindows + 1)
                    return undefined
                pending = pending.concat(children)
            }
        }
        return match
    }

    // An unpermitted Save Page callback has no safe view-origin fallback.
    // Before allowing it into the ordinary profile download manager, make
    // sure no same-profile tab is quarantined after an interrupted capture.
    // Unknown or malformed owner state fails closed for the whole profile.
    function profileHasPoisonedPageCaptureTab(profile) {
        var root = owner.rootOwner()
        if (!root || !profile || !root.hostWindow)
            return undefined
        var pending = [root.hostWindow]
        var seen = []
        var seenPopupHosts = []
        var popupCount = 0
        var maximumHosts = root.maximumWindows + 1

        function poisonedState(host, trackerRequired) {
            if (!host)
                return undefined
            try {
                if (host.browserProfile !== profile
                        && host.popupProfile !== profile
                        && host.windowWebEngineProfile !== profile)
                    return false
                var poisoned = host.nativePageCapturePoisonedTabIds
                if (poisoned === undefined || poisoned === null)
                    return trackerRequired === true ? undefined : false
                if (!Array.isArray(poisoned) || poisoned.length > 512)
                    return undefined
                return poisoned.length > 0
            } catch (error) {
                return undefined
            }
        }

        for (var i = 0; i < pending.length; ++i) {
            if (i >= maximumHosts)
                return undefined
            var host = pending[i]
            if (!host || seen.indexOf(host) >= 0)
                return undefined
            seen.push(host)
            var state = poisonedState(host, true)
            if (state !== false)
                return state

            var popups
            try {
                popups = host.popupWindowRegistry
            } catch (error) {
                return undefined
            }
            var popupEntries
            if (popups === undefined && host !== root.hostWindow) {
                popupEntries = []
            } else {
                popupEntries = PopupWindowOwnership.snapshotPopupEntriesFromRegistry(popups)
                if (popupEntries === null)
                    return undefined
            }
            if (popupCount + popupEntries.length > 64)
                return undefined
            popupCount += popupEntries.length
            for (var popupIndex = 0; popupIndex < popupEntries.length; ++popupIndex) {
                var popupHost = popupEntries[popupIndex].host
                if (seenPopupHosts.indexOf(popupHost) >= 0)
                    return undefined
                seenPopupHosts.push(popupHost)
                state = poisonedState(popupHost, false)
                if (state !== false)
                    return state
            }

            var childOwner = host.versioningWindowOwner
            if (childOwner) {
                var children = childOwner.windows
                if (!Array.isArray(children)
                        || pending.length + children.length > maximumHosts)
                    return undefined
                pending = pending.concat(children)
            }
        }
        return false
    }

    function profilePageCaptureAvailable(profile, requester) {
        var active = owner.activePageCaptureHost(profile)
        return active !== undefined && (active === null || active === requester)
    }

    function canCreate() {
        var root = rootOwner()
        return root !== null && windows.length < maximumWindows
                && root.totalOwnedWindows(root.maximumWindows) < root.maximumWindows
    }

    function retain(child) {
        var childOwner = null
        var visibleHandler = null
        var discoveryHandler = null
        var nestedDiscoveryHandler = null
        var visibleConnected = false
        var discoveryConnected = false
        var nestedDiscoveryConnected = false
        var published = false
        try {
            if (!child || windows.indexOf(child) >= 0 || !owner.canCreate())
                return false
            childOwner = child.versioningWindowOwner || null
            if (childOwner && childOwner.parentOwner !== owner)
                return false

            visibleHandler = function() {
                owner.releaseWhenApproved(child)
            }
            discoveryHandler = function() {
                owner.discoveryChanged()
                owner.releaseWhenApproved(child)
            }
            child.visibleChanged.connect(visibleHandler)
            visibleConnected = true
            child.versioningDiscoveryChanged.connect(discoveryHandler)
            discoveryConnected = true
            if (childOwner && childOwner.discoveryChanged) {
                nestedDiscoveryHandler = function() {
                    owner.discoveryChanged()
                    owner.releaseWhenApproved(child)
                }
                childOwner.discoveryChanged.connect(nestedDiscoveryHandler)
                nestedDiscoveryConnected = true
            }
            windows = windows.concat([child])
            published = true
            owner.discoveryChanged()
            return true
        } catch (error) {
            // Signal wiring and ownership publication are one transaction:
            // failed wiring must not leave a phantom child blocking shutdown.
            try {
                if (Array.isArray(windows) && windows.indexOf(child) >= 0)
                    windows = windows.filter(function(candidate) {
                        return candidate !== child
                    })
            } catch (cleanupError) {}
            try {
                if (nestedDiscoveryConnected)
                    childOwner.discoveryChanged.disconnect(nestedDiscoveryHandler)
            } catch (cleanupError) {}
            try {
                if (discoveryConnected)
                    child.versioningDiscoveryChanged.disconnect(discoveryHandler)
            } catch (cleanupError) {}
            try {
                if (visibleConnected)
                    child.visibleChanged.disconnect(visibleHandler)
            } catch (cleanupError) {}
            if (published) {
                try {
                    owner.discoveryChanged()
                } catch (cleanupError) {}
            }
            return false
        }
    }

    function releaseWhenApproved(child) {
        if (!child || child.visible || !child.shutdownApproved)
            return false
        if (typeof child.requestApprovedWindowRelease === "function")
            return child.requestApprovedWindowRelease()
        return owner.releaseClosed(child)
    }

    function releaseClosed(child) {
        if (!child || child.visible || !child.shutdownApproved
                || windows.indexOf(child) < 0
                || typeof child.hasActiveDownloads !== "function"
                || child.hasActiveDownloads()
                || (typeof child.hasOwnedPopupWindows === "function"
                    && child.hasOwnedPopupWindows() !== false)
                || (child.versioningWindowOwner
                    && child.versioningWindowOwner.totalOwnedWindows(1) > 0))
            return false
        windows = windows.filter(function(candidate) { return candidate !== child })
        owner.discoveryChanged()
        child.destroy()
        return true
    }

    // A hidden child can fail during asynchronous profile bootstrap before
    // it was ever visible. Its destruction must still release the strong
    // owner reference, including after a failed clean-branch publication.
    function forgetDestroyed(child) {
        if (!child || windows.indexOf(child) < 0)
            return false
        windows = windows.filter(function(candidate) { return candidate !== child })
        owner.discoveryChanged()
        return true
    }

    // A restore surface has its own reducer IDs. Only its process-unique owner
    // token can select it across surfaces; windowid-2 may exist more than once.
    function discoveryEntries(limit) {
        var remaining = Math.max(0, Math.min(64, Number(limit) || 0))
        var pending = windows.slice()
        var entries = []
        var seen = ({})
        for (var i = 0; i < pending.length && i < 64 && entries.length < remaining; ++i) {
            var child = pending[i]
            if (!child)
                continue
            if (child.versioningWindowOwner && pending.length < 64)
                pending = pending.concat(child.versioningWindowOwner.windows.slice(0, 64 - pending.length))
            if (child.shutdownApproved)
                continue
            var local = child.browserWindowRegistry || []
            for (var j = 0; j < local.length && j < 64 && entries.length < remaining; ++j) {
                var entry = local[j]
                var token = entry ? String(entry.windowToken || "") : ""
                if (entry && entry.host && entry.ui && token.length > 0 && !seen[token]) {
                    seen[token] = true
                    entries.push(entry)
                }
            }
        }
        return entries
    }

    function entryForOwnerToken(token) {
        var requested = String(token || "")
        var entries = discoveryEntries(64)
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i].windowToken === requested)
                return entries[i]
        }
        return null
    }

    function containsHost(host) {
        if (!host)
            return false
        var pending = windows.slice()
        var seen = []
        for (var i = 0; i < pending.length && i < maximumWindows; ++i) {
            var child = pending[i]
            if (!child || seen.indexOf(child) >= 0)
                return true // unknown ownership must not be double-dispatched
            if (child === host)
                return true
            seen.push(child)
            if (child.versioningWindowOwner)
                pending = pending.concat(child.versioningWindowOwner.windows || [])
        }
        return i < pending.length // an over-budget tree also fails closed
    }

    function requestCloseAll() {
        var current = windows.slice()
        for (var i = 0; i < current.length; ++i) {
            if (current[i].shutdownApproved && current[i].versioningWindowOwner)
                current[i].versioningWindowOwner.requestCloseAll()
            else if (!current[i].shutdownApproved)
                current[i].continueQuitRequest()
        }
    }

    function hostForDownload(download, observedView, sourceHost) {
        function isRecord(value) {
            return value !== null && value !== undefined
                    && typeof value === "object" && !Array.isArray(value)
        }

        function indexedValue(index, key) {
            return Object.prototype.hasOwnProperty.call(index, key)
                    ? index[key] : undefined
        }

        function requestIdentityMatches(host, requestId, request) {
            if (!host)
                return null
            var active = host.activeDownloads
            var pendingRequests = host.pendingDownloadRequests
            var completionRequests = host.downloadCompletionRequests
            if (!isRecord(active) || !isRecord(pendingRequests)
                    || !isRecord(completionRequests))
                return null
            return indexedValue(active, requestId) === request
                    || indexedValue(pendingRequests, requestId) === request
                    || indexedValue(completionRequests, requestId) === request
        }

        function retiredSaveIdentityMatches(host, allowAbsentIndexes) {
            if (!host || download.isSavePageDownload !== true
                    || !/^\d{1,20}$/.test(id)
                    || (requestProfile
                        && host.browserProfile !== requestProfile
                        && host.popupProfile !== requestProfile))
                return false
            // Retired save callbacks still belong to this host after its
            // active capture has been cleared. Keep that exact routing keyed
            // to the source profile because download IDs are profile-local.
            var retiredDownloads = host.nativePageCaptureRetiredDownloads
            var retiredPaths = host.nativePageCaptureRetiredPaths
            if (allowAbsentIndexes === true
                    && retiredDownloads === undefined && retiredPaths === undefined)
                return false // Popup windows do not own page-capture tombstones.
            if (!isRecord(retiredDownloads) || !isRecord(retiredPaths))
                return null
            return indexedValue(retiredDownloads, id) === true
                    || (savePath.length > 0
                        && indexedValue(retiredPaths, savePath) === true)
        }

        try {
        if (!download)
            return null
        var id = String(download.id)
        // Callers supply view evidence explicitly when they have it. Never
        // fall back to Qt Quick's unsafe request getter here.
        var view = arguments.length > 1 ? observedView : null
        var saveDirectory = download.isSavePageDownload === true
                ? String(download.downloadDirectory || "") : ""
        var saveFileName = download.isSavePageDownload === true
                ? String(download.downloadFileName || "") : ""
        var savePath = saveDirectory.length > 0 && saveFileName.length > 0
                && saveDirectory.length + saveFileName.length + 1 <= 4096
                && saveFileName.length <= 255
                ? saveDirectory + "/" + saveFileName : ""
        var requestProfile = sourceHost
                ? (sourceHost.browserProfile || sourceHost.popupProfile)
                : (owner.hostWindow
                   ? (owner.hostWindow.browserProfile || owner.hostWindow.popupProfile)
                   : null)
        var root = owner.rootOwner()
        if (!root)
            return undefined

        var pending = sourceHost ? [sourceHost] : windows.slice()
        var maximumHosts = maximumWindows + (sourceHost ? 1 : 0)
        var seen = []
        var seenPopupHosts = []
        var match = null
        var popupCount = 0
        for (var i = 0; i < pending.length; ++i) {
            if (i >= maximumHosts)
                return undefined
            var child = pending[i]
            if (!child || seen.indexOf(child) >= 0)
                return undefined
            seen.push(child)
            var views = child.tabViews
            if (!Array.isArray(views))
                return undefined
            var capture = child.nativePageCaptureSave
            if (capture !== null && capture !== undefined && !isRecord(capture))
                return undefined
            var captureProfile = capture ? capture.profile : null
            var capturePath = capture ? capture.path : null
            var captureDownload = capture ? capture.download : null
            var requestMatch = requestIdentityMatches(child, id, download)
            if (requestMatch === null)
                return undefined
            var retiredMatch = retiredSaveIdentityMatches(child, false)
            if (retiredMatch === null)
                return undefined
            var childProfile = child.browserProfile
            var matches = (view && views.indexOf(view) >= 0)
                    || requestMatch
                    || (captureDownload === download)
                    || (savePath.length > 0 && capture
                        && capturePath === savePath
                        && captureProfile === childProfile)
                    || retiredMatch
            if (matches) {
                if (match)
                    return undefined
                match = child
            }
            var popupEntries
            if (child === root.hostWindow) {
                popupEntries = PopupWindowOwnership.snapshotPopupEntries(child)
                if (popupEntries === null)
                    return undefined
            } else {
                var childPopupRegistry
                try {
                    childPopupRegistry = child.popupWindowRegistry
                } catch (error) {
                    return undefined
                }
                if (childPopupRegistry === undefined) {
                    // Popups are process-wide; restore children do not carry
                    // their own registry unless explicitly configured.
                    popupEntries = []
                } else {
                    popupEntries = PopupWindowOwnership.snapshotPopupEntriesFromRegistry(
                                childPopupRegistry)
                    if (popupEntries === null)
                        return undefined
                }
            }
            if (popupCount + popupEntries.length > 64)
                return undefined
            popupCount += popupEntries.length
            for (var popupIndex = 0; popupIndex < popupEntries.length; ++popupIndex) {
                var popupEntry = popupEntries[popupIndex]
                var popupHost = popupEntry.host
                if (seenPopupHosts.indexOf(popupHost) >= 0)
                    return undefined
                seenPopupHosts.push(popupHost)
                var popupRequestMatch = requestIdentityMatches(popupHost, id, download)
                if (popupRequestMatch === null)
                    return undefined
                var popupRetiredMatch = retiredSaveIdentityMatches(popupHost, true)
                if (popupRetiredMatch === null)
                    return undefined
                var popupMatch = (view && popupEntry.view === view)
                        || popupRequestMatch
                        || popupRetiredMatch
                if (popupMatch) {
                    if (match && match !== popupHost)
                        return undefined
                    match = popupHost
                }
            }
            if (child.versioningWindowOwner) {
                var descendants = child.versioningWindowOwner.windows
                if (!Array.isArray(descendants)
                        || pending.length + descendants.length > maximumHosts)
                    return undefined
                pending = pending.concat(descendants)
            }
        }
        return match
        } catch (error) {
            return undefined
        }
    }

    function cancelCloseAll() {
        var current = windows.slice()
        for (var i = 0; i < current.length; ++i) {
            if (current[i].shutdownApproved && current[i].versioningWindowOwner)
                current[i].versioningWindowOwner.cancelCloseAll()
            else
                current[i].abortApplicationShutdown()
        }
    }
}
