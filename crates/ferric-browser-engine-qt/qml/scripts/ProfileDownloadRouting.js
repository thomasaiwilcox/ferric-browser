.pragma library

// A profile-level signal is already bound to one owning adapter. It is safe
// to use that adapter as the manager for an ordinary transfer, but only when
// the host still proves it owns the exact profile that emitted the signal.
// This does not establish a tab origin; Save Page additionally needs an
// explicit profile-wide quarantine proof from its caller.
function boundProfileOwner(download, host, profile, savePageProfileSafe) {
    if (!download || !host || !profile
            || (download.isSavePageDownload === true
                && savePageProfileSafe !== true))
        return null
    return host.browserProfile === profile
            || host.popupProfile === profile
            || host.windowWebEngineProfile === profile
            ? host : null
}

// A request remains owned until the profile-level terminal callback is
// handled. `isFinished` can become true before that callback is delivered.
function hasOutstandingDownloadRequests(downloads) {
    try {
        // These registries are initialized on every owner. Missing or
        // malformed state cannot prove that releasing the owner is safe.
        if (!isRecord(downloads))
            return true
        for (var id in downloads) {
            if (!Object.prototype.hasOwnProperty.call(downloads, id))
                continue
            var request = downloads[id]
            if (!request
                    || (typeof request !== "object" && typeof request !== "function"))
                return true
            return true
        }
        return false
    } catch (error) {
        return true
    }
}

function isRecord(value) {
    return value !== null && value !== undefined
            && typeof value === "object" && !Array.isArray(value)
}

// Snapshot registry fields through their own descriptors. Qt's QML JS bridge
// can otherwise evaluate an accessor more than once during one property read;
// ownership decisions must use one stable value or fail closed.
function ownRecordValue(entry, key) {
    if (!isRecord(entry))
        return undefined
    var descriptor = Object.getOwnPropertyDescriptor(entry, key)
    if (!descriptor)
        return undefined
    return typeof descriptor.get === "function"
            ? descriptor.get.call(entry) : descriptor.value
}

function indexedRequest(index, id) {
    return Object.prototype.hasOwnProperty.call(index, id) ? index[id] : undefined
}

// Finds a live request only when every observed owner index is well-formed
// and the profile-local ID identifies at most one request for the active profile.
// `null` means a clean no-match; `undefined` means ownership is ambiguous.
function liveRequestForId(id, sourceHost, browserEntries, popupEntries, profile) {
    try {
        var requestId = String(id)
        if (!/^\d{1,20}$/.test(requestId) || !sourceHost || !profile
                || !Array.isArray(browserEntries) || browserEntries.length > 64
                || !Array.isArray(popupEntries) || popupEntries.length > 64) {
            return undefined
        }
        var sourceBrowserProfile = sourceHost.browserProfile
        var sourcePopupProfile = sourceHost.popupProfile
        var sourceWindowProfile = sourceHost.windowWebEngineProfile
        if (sourceBrowserProfile !== profile && sourcePopupProfile !== profile
                && sourceWindowProfile !== profile)
            return undefined
        var browserSnapshots = browserEntries.slice(0)
        var popupSnapshots = popupEntries.slice(0)
        var browserHosts = []
        var popupHosts = []
        var seenRegistryHosts = [sourceHost]
        var sourceRegistryEntrySeen = false
        for (var browserIndex = 0; browserIndex < browserSnapshots.length; ++browserIndex) {
            var browserEntry = browserSnapshots[browserIndex]
            if (!isRecord(browserEntry))
                return undefined
            var browserHost = browserEntry.host
            var browserProfile = browserEntry.profile
            if (!browserHost || !browserProfile)
                return undefined
            if (browserHost === sourceHost) {
                // The primary browser is intentionally present in its own
                // published registry; a second occurrence is still ambiguous.
                if (sourceRegistryEntrySeen || browserProfile !== profile)
                    return undefined
                sourceRegistryEntrySeen = true
                continue
            }
            if (seenRegistryHosts.indexOf(browserHost) >= 0)
                return undefined
            seenRegistryHosts.push(browserHost)
            browserHosts.push({ host: browserHost, profile: browserProfile })
        }
        for (var popupIndex = 0; popupIndex < popupSnapshots.length; ++popupIndex) {
            var popupEntry = popupSnapshots[popupIndex]
            if (!isRecord(popupEntry))
                return undefined
            var popupHost = popupEntry.host
            if (!popupHost || seenRegistryHosts.indexOf(popupHost) >= 0)
                return undefined
            var popupProfile = popupHost.popupProfile
            if (!popupProfile)
                return undefined
            seenRegistryHosts.push(popupHost)
            popupHosts.push({ host: popupHost, profile: popupProfile })
        }

        var seenHosts = []
        var match = null

        function consider(host) {
            if (!host || seenHosts.indexOf(host) >= 0)
                return false
            seenHosts.push(host)
            var active = host.activeDownloads
            var pending = host.pendingDownloadRequests
            var completion = host.downloadCompletionRequests
            if (!isRecord(active) || !isRecord(pending) || !isRecord(completion))
                return false
            var values = [active, pending, completion]
            var ownerRequest = null
            for (var mapIndex = 0; mapIndex < values.length; ++mapIndex) {
                var index = values[mapIndex]
                if (!Object.prototype.hasOwnProperty.call(index, requestId))
                    continue
                var candidate = index[requestId]
                if (!candidate
                        || (typeof candidate !== "object" && typeof candidate !== "function")
                        || (ownerRequest && ownerRequest !== candidate))
                    return false
                ownerRequest = candidate
            }
            if (ownerRequest) {
                if (match && match !== ownerRequest)
                    return false
                // The same request cannot be live under two distinct owners.
                if (match === ownerRequest)
                    return false
                match = ownerRequest
            }
            return true
        }

        if (!consider(sourceHost))
            return undefined
        for (var browserHostIndex = 0; browserHostIndex < browserHosts.length; ++browserHostIndex) {
            var browserOwner = browserHosts[browserHostIndex]
            if (browserOwner.profile === profile && !consider(browserOwner.host))
                return undefined
        }
        for (var popupHostIndex = 0; popupHostIndex < popupHosts.length; ++popupHostIndex) {
            var popupOwner = popupHosts[popupHostIndex]
            if (popupOwner.profile === profile && !consider(popupOwner.host))
                return undefined
        }
        return match
    } catch (error) {
        return undefined
    }
}

// Resolves an already-retained request to one exact owner. A clean no-match
// returns null so a caller may apply its separate, profile-bound admission
// policy; malformed state or an ID collision returns undefined.
function requestOwnerForExactDownload(download, sourceHost, popupHosts, profile) {
    try {
        if (!download || !sourceHost || !profile
                || !Array.isArray(popupHosts) || popupHosts.length > 64)
            return undefined
        var requestId = String(download.id)
        if (!/^\d{1,20}$/.test(requestId))
            return undefined
        var sourceBrowserProfile = sourceHost.browserProfile
        var sourcePopupProfile = sourceHost.popupProfile
        var sourceWindowProfile = sourceHost.windowWebEngineProfile
        if (sourceBrowserProfile !== profile && sourcePopupProfile !== profile
                && sourceWindowProfile !== profile)
            return undefined

        var popupSnapshot = popupHosts.slice(0)
        var candidates = [sourceHost]
        var seenHosts = [sourceHost]
        for (var popupIndex = 0; popupIndex < popupSnapshot.length; ++popupIndex) {
            var popupHost = popupSnapshot[popupIndex]
            if (!popupHost || seenHosts.indexOf(popupHost) >= 0)
                return undefined
            seenHosts.push(popupHost)
            var popupProfile = popupHost.popupProfile
            if (!popupProfile)
                return undefined
            if (popupProfile === profile)
                candidates.push(popupHost)
        }

        var match = null
        for (var hostIndex = 0; hostIndex < candidates.length; ++hostIndex) {
            var host = candidates[hostIndex]
            var active = host.activeDownloads
            var pending = host.pendingDownloadRequests
            var completion = host.downloadCompletionRequests
            if (!isRecord(active) || !isRecord(pending) || !isRecord(completion))
                return undefined
            var indexes = [active, pending, completion]
            var owns = false
            for (var indexNumber = 0; indexNumber < indexes.length; ++indexNumber) {
                var index = indexes[indexNumber]
                if (!Object.prototype.hasOwnProperty.call(index, requestId))
                    continue
                var indexedDownload = index[requestId]
                if (!indexedDownload
                        || (typeof indexedDownload !== "object"
                            && typeof indexedDownload !== "function")
                        || indexedDownload !== download)
                    return undefined
                owns = true
            }
            if (owns) {
                if (match)
                    return undefined
                match = host
            }
        }
        return match
    } catch (error) {
        return undefined
    }
}

// Register the exact profile request before any shutdown decision. Once a
// host's close has been approved, cancel without presenting UI; retaining the
// request lets its profile-level terminal callback return to the same owner.
function retainAndCancelIfShutdownApproved(host, download, cancelRequest) {
    function cancelSafely() {
        var cancellationResolved = false
        try {
            if (typeof cancelRequest === "function")
                cancellationResolved = cancelRequest(download) === true
        } catch (error) {}
        if (!cancellationResolved) {
            try {
                if (download && typeof download.cancel === "function")
                    download.cancel()
            } catch (error) {}
        }
    }

    if (!host || !download) {
        cancelSafely()
        return true
    }
    var id = ""
    try {
        id = download.id === undefined || download.id === null
                ? "" : String(download.id)
    } catch (error) {}
    if (!/^\d{1,20}$/.test(id)) {
        cancelSafely()
        return true
    }
    var completionRequests = null
    var alreadyRetained = false
    try {
        completionRequests = host.downloadCompletionRequests
        if (!isRecord(completionRequests)) {
            cancelSafely()
            return true
        }
        if (Object.prototype.hasOwnProperty.call(completionRequests, id)) {
            if (completionRequests[id] !== download) {
                cancelSafely()
                return true
            }
            alreadyRetained = true
        } else {
            completionRequests[id] = download
            if (!Object.prototype.hasOwnProperty.call(completionRequests, id)
                    || completionRequests[id] !== download) {
                cancelSafely()
                return true
            }
        }
    } catch (error) {
        cancelSafely()
        return true
    }
    var shutdownApproved = false
    try {
        shutdownApproved = host.shutdownApproved === true
                || host.windowShutdownApproved === true
    } catch (error) {
        cancelSafely()
        return true
    }
    if (!shutdownApproved) {
        // A duplicate notification for the same request must not open a
        // second chooser or attach duplicate state handlers.
        return alreadyRetained
    }
    cancelSafely()
    return true
}

// A profile emits one callback for all of its views. IDs identify a request
// inside that profile, not the QML window that owns it. Return undefined for
// ambiguous/malformed ownership and null when no exact owner can be found.
function resolve(download, view, sourceHost, restoredHost,
                 browserEntries, popupEntries, profile) {
    try {
        return resolveUnchecked(download, view, sourceHost, restoredHost,
                                browserEntries, popupEntries, profile)
    } catch (error) {
        return undefined
    }
}

function resolveUnchecked(download, view, sourceHost, restoredHost,
                          browserEntries, popupEntries, profile) {
    if (!download || !sourceHost || !profile || restoredHost === undefined
            || !Array.isArray(browserEntries) || browserEntries.length > 64
            || !Array.isArray(popupEntries) || popupEntries.length > 64) {
        return undefined
    }
    if (restoredHost && restoredHost.browserProfile !== profile
            && restoredHost.popupProfile !== profile) {
        return undefined
    }
    var id = String(download.id)
    var saveDirectory = download.isSavePageDownload === true
            ? String(download.downloadDirectory || "") : ""
    var saveFileName = download.isSavePageDownload === true
            ? String(download.downloadFileName || "") : ""
    var savePath = saveDirectory.length > 0 && saveFileName.length > 0
            && saveDirectory.length + saveFileName.length + 1 <= 4096
            && saveFileName.length <= 255
            ? saveDirectory + "/" + saveFileName : ""
    var match = restoredHost || null

    function consider(host, views) {
        if (!host || !Array.isArray(views)) {
            return false
        }
        // These indexes are initialized on every browser and popup owner. A
        // missing or malformed index is unknown ownership, not an empty owner.
        // Snapshot each property once so a changing getter cannot split one
        // routing decision across different request registries.
        var activeDownloads = host.activeDownloads
        var pendingDownloadRequests = host.pendingDownloadRequests
        var downloadCompletionRequests = host.downloadCompletionRequests
        var capture = host.nativePageCaptureSave
        if (!isRecord(activeDownloads) || !isRecord(pendingDownloadRequests)
                || !isRecord(downloadCompletionRequests)
                || (capture !== null && capture !== undefined && !isRecord(capture))) {
            return false
        }
        var captureProfile = capture ? capture.profile : null
        var capturePath = capture ? capture.path : null
        var captureDownload = capture ? capture.download : null
        var activeRequest = indexedRequest(activeDownloads, id)
        var pendingRequest = indexedRequest(pendingDownloadRequests, id)
        var completedRequest = indexedRequest(downloadCompletionRequests, id)
        var exactCapturePath = savePath.length > 0 && capture
                && captureProfile === profile && capturePath === savePath
        var owns = (view && views.indexOf(view) >= 0)
                || activeRequest === download
                || pendingRequest === download
                || completedRequest === download
                || captureDownload === download
                || exactCapturePath
        if (owns) {
            if (match && match !== host) {
                return false
            }
            match = host
        }
        return true
    }

    var sourceViews = sourceHost.tabViews
    if (!Array.isArray(sourceViews) || !consider(sourceHost, sourceViews)) {
        return undefined
    }
    var checkedBrowserEntries = []
    var seenBrowserHosts = []
    try {
        for (var browserIndex = 0; browserIndex < browserEntries.length; ++browserIndex) {
            var browserEntry = browserEntries[browserIndex]
            if (!browserEntry)
                return undefined
            var browserHost = ownRecordValue(browserEntry, "host")
            var browserProfile = ownRecordValue(browserEntry, "profile")
            var browserView = ownRecordValue(browserEntry, "view")
            if (!browserHost || seenBrowserHosts.indexOf(browserHost) >= 0)
                return undefined
            var browserActiveView = browserHost.activeView
            seenBrowserHosts.push(browserHost)
            checkedBrowserEntries.push({
                host: browserHost,
                profile: browserProfile,
                views: [browserView, browserActiveView]
            })
        }
    } catch (error) {
        return undefined
    }
    for (var i = 0; i < checkedBrowserEntries.length; ++i) {
        var checkedBrowser = checkedBrowserEntries[i]
        if (checkedBrowser.profile !== profile) {
            continue
        }
        if (!consider(checkedBrowser.host, checkedBrowser.views)) {
            return undefined
        }
    }
    // Popup records are external ownership state. Read the host, view, and
    // profile once before routing so duplicate hosts or unstable getters can
    // never be interpreted as an ordinary non-match.
    var checkedPopups = []
    var seenPopupHosts = []
    try {
        for (var popupIndex = 0; popupIndex < popupEntries.length; ++popupIndex) {
            var popup = popupEntries[popupIndex]
            if (!popup)
                return undefined
            var popupHost = ownRecordValue(popup, "host")
            var popupView = ownRecordValue(popup, "view")
            if (!popupHost || seenPopupHosts.indexOf(popupHost) >= 0
                    || seenBrowserHosts.indexOf(popupHost) >= 0)
                return undefined
            var popupProfile = popupHost.popupProfile
            seenPopupHosts.push(popupHost)
            checkedPopups.push({
                host: popupHost,
                view: popupView || null,
                profile: popupProfile
            })
        }
    } catch (error) {
        return undefined
    }
    for (var j = 0; j < checkedPopups.length; ++j) {
        var checkedPopup = checkedPopups[j]
        if (checkedPopup.profile !== profile) {
            continue
        }
        if (!consider(checkedPopup.host, [checkedPopup.view])) {
            return undefined
        }
    }
    return match
}
