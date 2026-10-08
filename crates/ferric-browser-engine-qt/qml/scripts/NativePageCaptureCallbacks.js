.pragma library

// Keep canceled native page-save identities long enough to absorb callbacks
// that arrive after their Rust operation has been retired. Entries are never
// evicted while the process is alive: if the bounded quarantine fills, native
// page-save routing is disabled for that window rather than risking ordinary
// download finalization for a late callback.
function retireDownload(owner, downloadId, limit) {
    var id = String(downloadId || "")
    if (!/^\d{1,20}$/.test(id))
        return false

    var maximum = Number(limit)
    var retired = owner.nativePageCaptureRetiredDownloads || ({})
    var order = (owner.nativePageCaptureRetiredDownloadOrder || []).slice()
    if (retired[id] === true)
        return true
    if (!Number.isInteger(maximum) || maximum < 1 || order.length >= maximum)
        return false

    retired[id] = true
    order.push(id)
    owner.nativePageCaptureRetiredDownloads = retired
    owner.nativePageCaptureRetiredDownloadOrder = order
    return true
}

function retirementCapacityAvailable(owner, limit) {
    if (owner.nativePageCaptureRetirementExhausted === true)
        return false
    var maximum = Number(limit)
    var order = owner.nativePageCaptureRetiredDownloadOrder || []
    var paths = owner.nativePageCaptureRetiredPathOrder || []
    // One operation can retire its request, a duplicate request, and its exact
    // staged path if cancellation wins the callback race.
    return Number.isInteger(maximum) && maximum > 0
            && order.length + 2 <= maximum
            && paths.length + 1 <= maximum
}

function retirementCapacityAvailableForOwner(owner) {
    return typeof owner.nativePageCaptureRetirementCapacityAvailable !== "function"
            || owner.nativePageCaptureRetirementCapacityAvailable()
}

function captureTabIsPoisoned(owner, tabId) {
    var value = String(tabId || "")
    if (value.length === 0)
        return false
    var poisoned = owner.nativePageCapturePoisonedTabIds || []
    return poisoned.indexOf(value) >= 0
}

function poisonCaptureTab(owner, capture, force) {
    if (!capture || (force !== true && capture.downloadFinished === true)
            || !capture.path || capture.path.length === 0)
        return true
    var tabId = String(capture.tabId || "")
    if (tabId.length === 0)
        return true
    var poisoned = (owner.nativePageCapturePoisonedTabIds || []).slice()
    if (poisoned.indexOf(tabId) >= 0)
        return true
    var maximum = Number(owner.nativePageCaptureRetiredDownloadLimit || 512)
    if (!Number.isInteger(maximum) || maximum < 1
            || poisoned.length >= maximum) {
        owner.nativePageCaptureRetirementExhausted = true
        return false
    }
    poisoned.push(tabId)
    owner.nativePageCapturePoisonedTabIds = poisoned
    return true
}

function retireCaptureIdentities(owner, capture, extraDownloadIds) {
    if (!retirementCapacityAvailableForOwner(owner))
        return false
    if (!poisonCaptureTab(owner, capture))
        return false
    if (capture && capture.path && capture.path.length > 0
            && !owner.retireNativePageCapturePath(capture.path))
        return false
    var ids = []
    if (capture && capture.downloadId && capture.downloadId.length > 0)
        ids.push(capture.downloadId)
    extraDownloadIds = extraDownloadIds || []
    for (var i = 0; i < extraDownloadIds.length; ++i) {
        if (extraDownloadIds[i] && String(extraDownloadIds[i]).length > 0)
            ids.push(String(extraDownloadIds[i]))
    }
    for (var j = 0; j < ids.length; ++j) {
        if (!owner.retireNativePageCaptureDownload(ids[j]))
            return false
    }
    return true
}

// If identities cannot be quarantined, suppress page-save callbacks for this
// owner window for the rest of its lifetime. Keeping the window-local latch
// is bounded and prevents any late completion from entering ordinary download
// finalization after its exact request identity could not be retained.
function failClosedRetirement(owner, browserUi, capture, downloads, status,
                              cancelNative) {
    owner.nativePageCaptureRetirementExhausted = true
    var captures = []
    if (capture)
        captures.push(capture)
    var activeCapture = owner.nativePageCaptureSave
    if (activeCapture && activeCapture !== capture)
        captures.push(activeCapture)
    owner.nativePageCaptureSave = null
    for (var j = 0; j < captures.length; ++j) {
        var retiringCapture = captures[j]
        if (typeof owner.discardNativePageCaptureMutationProbe === "function")
            owner.discardNativePageCaptureMutationProbe(retiringCapture)
        if (browserUi
                && typeof browserUi.cancel_native_page_capture_request === "function")
            browserUi.cancel_native_page_capture_request(retiringCapture.operationId)
    }
    if (cancelNative !== false) {
        downloads = downloads || []
        for (var k = 0; k < captures.length; ++k) {
            if (captures[k].download)
                downloads.push(captures[k].download)
        }
        var cancelled = []
        for (var i = 0; i < downloads.length; ++i) {
            var download = downloads[i]
            if (!download || cancelled.indexOf(download) >= 0)
                continue
            var finishedCapture = false
            for (var n = 0; n < captures.length; ++n) {
                if (captures[n].downloadFinished === true
                        && captures[n].download === download) {
                    finishedCapture = true
                    break
                }
            }
            if (finishedCapture)
                continue
            cancelled.push(download)
            try { download.cancel() } catch (error) {}
        }
    }
    if (browserUi)
        browserUi.status_text = status
    if (typeof owner.scheduleRuntimeWork === "function")
        owner.scheduleRuntimeWork(0)
    return true
}

function shouldIgnoreSavePageCompletion(owner, download) {
    return owner.nativePageCaptureRetirementExhausted === true
            && download && download.isSavePageDownload === true
}

function publicSavePagePath(download) {
    if (!download || download.isSavePageDownload !== true)
        return ""
    var directory = String(download.downloadDirectory || "")
    var fileName = String(download.downloadFileName || "")
    if (directory.length === 0 || fileName.length === 0
            || directory.length + fileName.length + 1 > 4096
            || fileName.length > 255)
        return ""
    return directory + "/" + fileName
}

function shutdownApproved(owner) {
    return owner && (owner.shutdownApproved === true
                     || owner.windowShutdownApproved === true)
}

function refuseSavePageDuringShutdown(owner, browserUi, download, id, path) {
    var capture = owner.nativePageCaptureSave
    if (capture) {
        if (typeof owner.rejectNativePageCaptureRequest === "function")
            owner.rejectNativePageCaptureRequest(capture, download, id)
        else
            failClosedRetirement(owner, browserUi, capture, [download],
                "Native page save refused after shutdown approval")
        if (browserUi)
            browserUi.status_text = "Native page save refused after shutdown approval"
        return true
    }

    if (typeof owner.retireNativePageCaptureDownload !== "function"
            || typeof owner.retireNativePageCapturePath !== "function"
            || !owner.retireNativePageCaptureDownload(id)
            || (path.length > 0 && !owner.retireNativePageCapturePath(path))) {
        failClosedRetirement(owner, browserUi, null, [download],
            "Native page-save callback capacity is exhausted; page saves are blocked in this window")
        return true
    }
    try { download.cancel() } catch (error) {}
    return true
}

function retirePath(owner, path, limit) {
    var value = String(path || "")
    var maximum = Number(limit)
    if (value.length === 0 || value.length > 4096
            || !Number.isInteger(maximum) || maximum < 1)
        return false

    var retired = owner.nativePageCaptureRetiredPaths || ({})
    var order = (owner.nativePageCaptureRetiredPathOrder || []).slice()
    if (retired[value] === true)
        return true
    if (order.length >= maximum)
        return false

    retired[value] = true
    order.push(value)
    owner.nativePageCaptureRetiredPaths = retired
    owner.nativePageCaptureRetiredPathOrder = order
    return true
}

function isRetiredPath(owner, path) {
    var value = String(path || "")
    var retired = owner.nativePageCaptureRetiredPaths || ({})
    return retired[value] === true
}

function isRetiredDownload(owner, downloadId) {
    var id = String(downloadId || "")
    if (!/^\d{1,20}$/.test(id))
        return false
    var retired = owner.nativePageCaptureRetiredDownloads || ({})
    return retired[id] === true
}

function consumeDownload(owner, downloadId) {
    var id = String(downloadId || "")
    if (!/^\d{1,20}$/.test(id))
        return false

    var retired = owner.nativePageCaptureRetiredDownloads || ({})
    if (retired[id] !== true)
        return false
    delete retired[id]
    var order = (owner.nativePageCaptureRetiredDownloadOrder || []).slice()
    var index = order.indexOf(id)
    if (index >= 0)
        order.splice(index, 1)
    owner.nativePageCaptureRetiredDownloads = retired
    owner.nativePageCaptureRetiredDownloadOrder = order
    return true
}

// The download's URL and view identity were observed before the asynchronous
// mutation probe. Refuse to reuse those facts after a tab/view or route change.
function mutationEvidenceForCurrentView(capture, facts, evidence, activeView) {
    if (!capture || !facts || !capture.view || capture.view !== activeView)
        return "document-changed"
    try {
        var liveUrl = String(capture.view.url)
        if (typeof capture.capturedUrl !== "string"
                || capture.capturedUrl.length === 0
                || liveUrl !== capture.capturedUrl
                || liveUrl !== String(facts.observed_url || ""))
            return "document-changed"
    } catch (error) {
        return "document-changed"
    }
    return evidence === "observed-stable" || evidence === "observed-changed"
            || evidence === "document-changed" ? evidence : "not-observed"
}

// Completion may arrive after Qt has destroyed the initiating view. The
// download's QML view getter is not safe to inspect then; require the exact
// request retained at start and the captured view still active instead.
function completionViewFacts(capture, download, activeView) {
    var unavailable = { request_matches: false, view_matches: false,
                        observed_url: "" }
    if (!capture || !download || capture.download !== download)
        return unavailable
    var requestOwned = { request_matches: true, view_matches: false,
                         observed_url: "" }
    if (!capture.view || capture.view !== activeView)
        return requestOwned
    try {
        var url = String(capture.view.url || "")
        if (url.length === 0)
            return requestOwned
        return { request_matches: true, view_matches: true, observed_url: url }
    } catch (error) {
        return requestOwned
    }
}

// Collection is asynchronous. Before accepting its sidecar or starting Qt's
// auto-starting Save Page request, prove that the exact captured view still
// owns the logical tab and still shows the captured document. A vanished view
// and a navigated document have different partial-window outcomes.
function preSaveCaptureStatus(capture, activeView) {
    if (!capture || !capture.view || capture.view !== activeView
            || typeof capture.capturedUrl !== "string"
            || capture.capturedUrl.length === 0)
        return "native-view-lost"
    try {
        var liveUrl = String(capture.view.url || "")
        if (liveUrl.length === 0)
            return "native-view-lost"
        return liveUrl === capture.capturedUrl ? "current" : "document-changed"
    } catch (error) {
        return "native-view-lost"
    }
}

// Keep terminal download failures distinct. A successful byte stream without
// the exact captured view is not evidence that the captured document survived
// until completion, even when the mutation probe also reports a document
// change because that view disappeared.
function completedArchiveFailure(facts, maximumBytes, requestMatches) {
    if (!facts)
        return "invalid-archive"
    if (requestMatches !== true)
        return "invalid-archive"
    var state = String(facts.terminal_state || "")
    if (state === "cancelled" || state === "interrupted")
        return state
    if (state !== "completed")
        return "invalid-archive"
    var received = Number(facts.received_bytes)
    var maximum = Number(maximumBytes)
    if (!Number.isSafeInteger(received) || !Number.isSafeInteger(maximum)
            || maximum <= 0)
        return "invalid-archive"
    if (received <= 0)
        return "empty-archive"
    if (received > maximum)
        return "invalid-archive"
    if (facts.view_matches !== true)
        return "native-view-lost"
    if (facts.mutation_evidence === "document-changed")
        return "document-changed"
    return ""
}

// Consume the exact preauthorized archive save, its retired-path callback, or
// any ambiguous page-save event that arrives while the profile slot is held.
// Ordinary non-save downloads continue through normal routing.
function routeSaveRequest(owner, browserUi, download, mhtmlFormat,
                          allowUnpermittedSavePage) {
    if (!download || download.isSavePageDownload !== true)
        return false
    if (owner.nativePageCaptureRetirementExhausted === true) {
        try { download.cancel() } catch (error) {}
        return true
    }
    var id = String(download.id)
    // A duplicate/late request can arrive after its native operation retired.
    // Keep it out of ordinary download routing and leave the tombstone in place
    // so a later finish callback is still absorbed by the same owner.
    if (isRetiredDownload(owner, id)) {
        try { download.cancel() } catch (error) {}
        return true
    }
    var path = publicSavePagePath(download)
    if (owner.isRetiredNativePageCapturePath(path)) {
        if (!owner.retireNativePageCaptureDownload(id)) {
            failClosedRetirement(owner, browserUi, null, [download],
                "Native page-save callback capacity is exhausted; page saves are blocked in this window")
        } else {
            download.cancel()
        }
        return true
    }
    if (shutdownApproved(owner))
        return refuseSavePageDuringShutdown(owner, browserUi, download, id, path)
    var capture = owner.nativePageCaptureSave
    if (!capture)
        return routeUnpermittedSavePageRequest(
                    owner, browserUi, download, id,
                    allowUnpermittedSavePage === true)

    // A private one-use staging path is the public correlation token when the
    // Qt Quick request cannot safely expose its originating view. Captures
    // are serialized per profile, and the exact path is never supplied to
    // page content. Do not evaluate Qt Quick's view getter on this path.
    var exactPath = path.length > 0 && path === capture.path
    if (exactPath && captureTabIsPoisoned(owner, capture.tabId)) {
        if (!owner.retireNativePageCaptureDownload(id)) {
            failClosedRetirement(owner, browserUi, capture, [download],
                "Native page-save callback capacity is exhausted; page saves are blocked in this window")
        } else {
            owner.rejectNativePageCaptureRequest(capture, download, id)
            browserUi.status_text = "Page saves are unavailable for this tab after an interrupted native save"
        }
        return true
    }
    var exact = exactPath && download.savePageFormat === mhtmlFormat
    if (exact) {
        if (capture.downloadId.length > 0) {
            owner.rejectNativePageCaptureRequest(capture, download, id)
            return true
        }
        if (!browserUi.note_native_page_capture_download_requested(
                    capture.operationId, id, path, true, true, true)) {
            if (retireChangedCaptureBeforeRequest(owner, browserUi,
                                                  capture, download, id, path))
                return true
            owner.rejectNativePageCaptureRequest(capture, download, id)
            return true
        }
        if (owner.nativePageCaptureSave !== capture) {
            owner.rejectNativePageCaptureRequest(capture, download, id)
            return true
        }
        capture.downloadId = id
        capture.download = download
        var retireOnTerminalState = function() {
            var terminal = owner.downloadStateName(download)
            if (terminal === "cancelled" || terminal === "interrupted") {
                Qt.callLater(function() {
                    owner.retireNativePageCaptureWithoutFinish(
                                capture, download, terminal)
                })
            }
        }
        download.stateChanged.connect(retireOnTerminalState)
        retireOnTerminalState()
        if (download.receivedBytesChanged
                && typeof download.receivedBytesChanged.connect === "function") {
            download.receivedBytesChanged.connect(function() {
                retireOversizedCapture(owner, browserUi, capture, download)
            })
            retireOversizedCapture(owner, browserUi, capture, download)
        }
        // WebEngineView.save auto-starts this request.
        return true
    }
    // There can be only one page save in flight for a profile during capture.
    // A second save with any other path/format is ambiguous, so it cannot be
    // diverted into the ordinary Save Page destination path.
    owner.rejectNativePageCaptureRequest(capture, download, id)
    return true
}

function routeUnpermittedSavePageRequest(owner, browserUi, download, id,
                                         profileAdmissionSafe) {
    // The production caller has checked every capture-capable owner on this
    // profile before enabling normal Save Page handling. Synthetic callers
    // without that proof conservatively refuse when this owner's poison state
    // is nonempty or malformed; they never inspect Qt's view getter.
    if (profileAdmissionSafe === true)
        return false
    var poisoned = owner.nativePageCapturePoisonedTabIds || []
    if (Array.isArray(poisoned) && poisoned.length === 0)
        return false
    if (typeof owner.retireNativePageCaptureDownload !== "function"
            || !owner.retireNativePageCaptureDownload(id)) {
        failClosedRetirement(owner, browserUi, null, [download],
            "Native page-save callback capacity is exhausted; page saves are blocked in this window")
        return true
    }
    try { download.cancel() } catch (error) {}
    browserUi.status_text = "Page saves are unavailable after an interrupted native save"
    return true
}

// Qt reports native MHTML growth asynchronously. Stop the exact admitted save
// on the first observed byte beyond its limit; completion retains a separate
// authoritative size check because signal cadence is not a filesystem quota.
function retireOversizedCapture(owner, browserUi, capture, download) {
    if (!capture || owner.nativePageCaptureSave !== capture
            || capture.download !== download)
        return false
    var maximum = Number(capture.maxArchiveBytes)
    var received = Number(download.receivedBytes)
    if (!Number.isSafeInteger(maximum) || maximum <= 0
            || !Number.isFinite(received) || received <= maximum)
        return false
    if (!retireCaptureIdentities(owner, capture, [])) {
        return failClosedRetirement(owner, browserUi, capture, [download],
            "Native page-save callback capacity is exhausted; window capture was stopped")
    }

    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.downloadFinished !== true) {
        try { download.cancel() } catch (error) {}
    }
    var continued = browserUi.fail_native_window_capture_page(
                capture.operationId, capture.tabId, "invalid-archive",
                capture.path, capture.downloadId)
    if (!continued)
        browserUi.cancel_native_page_capture_request(capture.operationId)
    browserUi.status_text = continued
            ? "An oversized page archive was discarded; continuing window capture"
            : "Page archive exceeded its size limit and was discarded"
    owner.scheduleRuntimeWork(0)
    return true
}

// Navigation can replace the captured document before Qt emits the first
// save-request callback. Only the exact staged request may become a failed
// page; quarantine its path and ID before canceling Qt's callback source.
function retireChangedCaptureBeforeRequest(owner, browserUi, capture,
                                           download, id, path) {
    if (!capture || owner.nativePageCaptureSave !== capture
            || capture.path !== path
            || typeof capture.capturedUrl !== "string"
            || capture.capturedUrl.length === 0)
        return false
    var liveUrl = ""
    try { liveUrl = String(capture.view.url || "") } catch (error) {}
    if (liveUrl === capture.capturedUrl)
        return false
    if (!retireCaptureIdentities(owner, capture, [id])) {
        failClosedRetirement(owner, browserUi, capture, [download],
            "Native page-save callback capacity is exhausted; window capture was stopped")
        return true
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    try { download.cancel() } catch (error) {}
    var continued = typeof browserUi.fail_native_window_capture_page === "function"
            && browserUi.fail_native_window_capture_page(
                   capture.operationId, capture.tabId,
                   "document-changed", path, "")
    if (!continued)
        browserUi.cancel_native_page_capture_request(capture.operationId)
    browserUi.status_text = continued
            ? "One page changed; continuing window capture"
            : "Page archive save did not match the captured page and was discarded"
    owner.scheduleRuntimeWork(0)
    return true
}

// A cancelled/interrupted page-save may never emit downloadFinished. Retire
// only the still-current, exactly correlated capture from its terminal state.
function retireTerminalCapture(owner, browserUi, capture, download, state) {
    if (!capture || !download || owner.nativePageCaptureSave !== capture
            || capture.download !== download
            || capture.downloadFinished === true
            || (state !== "cancelled" && state !== "interrupted")) {
        return false
    }
    if (!retireCaptureIdentities(owner, capture, [])) {
        return failClosedRetirement(owner, browserUi, capture, [],
            "Native page-save callback capacity is exhausted; window capture was stopped",
            false)
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    var continued = typeof browserUi.fail_native_window_capture_page === "function"
            && browserUi.fail_native_window_capture_page(
                   capture.operationId, capture.tabId, state,
                   capture.path, capture.downloadId)
    if (!continued) {
        browserUi.cancel_native_page_capture_request(capture.operationId)
    }
    browserUi.status_text = continued
            ? "One page archive save failed; continuing window capture"
            : state === "cancelled"
                ? "Page archive save was cancelled and discarded"
                : "Page archive save was interrupted and discarded"
    owner.scheduleRuntimeWork(0)
    return true
}

// The native save deadline applies only to its still-current page. Retire
// callback identities before canceling Qt, which may synchronously report a
// terminal state. A separately consented partial window may then advance.
function retireTimedOutCapture(owner, browserUi, capture) {
    if (!capture || owner.nativePageCaptureSave !== capture
            || capture.downloadFinished === true)
        return false
    if (!retireCaptureIdentities(owner, capture, [])) {
        return failClosedRetirement(owner, browserUi, capture,
            capture.download ? [capture.download] : [],
            "Native page-save callback capacity is exhausted; window capture was stopped")
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.download) {
        try { capture.download.cancel() } catch (error) {}
    }
    if (!browserUi.fail_native_window_capture_page(
                capture.operationId, capture.tabId, "timeout",
                capture.path, capture.downloadId)) {
        browserUi.cancel_native_page_capture_request(capture.operationId)
    }
    browserUi.status_text = "Page archive save timed out; the page was discarded"
    owner.scheduleRuntimeWork(0)
    return true
}

// Shutdown cancellation is all-or-nothing, including when Qt has finished
// writing but Rust has not yet installed/published the archive. Retain exact
// late-callback identities before clearing ownership or cancelling Qt.
function cancelCaptureForShutdown(owner, browserUi, capture) {
    if (!capture || owner.nativePageCaptureSave !== capture)
        return false
    if (!retireCaptureIdentities(owner, capture, [])) {
        return failClosedRetirement(owner, browserUi, capture,
            capture.download ? [capture.download] : [],
            "Native page-save callback capacity is exhausted; shutdown cancelled the capture")
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.download && capture.downloadFinished !== true) {
        try { capture.download.cancel() } catch (error) {}
    }
    browserUi.cancel_native_page_capture_request(capture.operationId)
    browserUi.status_text = "Native page capture cancelled for shutdown"
    owner.scheduleRuntimeWork(0)
    return true
}

// Rust owns capture disclosure and staging before the exact Qt save is
// delivered to QML. Shutdown must cancel that exact outstanding operation too.
function cancelPendingCaptureForShutdown(owner, browserUi) {
    var capture = owner.nativePageCaptureSave
    if (capture)
        return cancelCaptureForShutdown(owner, browserUi, capture)
    if (!browserUi
            || typeof browserUi.cancel_pending_native_page_capture_for_shutdown !== "function"
            || !browserUi.cancel_pending_native_page_capture_for_shutdown())
        return false
    if (owner.nativePageCaptureDisclosure
            && owner.nativePageCaptureDisclosure.length === 2)
        owner.nativePageCaptureDisclosure = []
    browserUi.status_text = "Native page capture cancelled for shutdown"
    owner.scheduleRuntimeWork(0)
    return true
}

// Production queues a close made before request registration until Qt has
// emitted the profile event. Once registered, quarantine the exact capture
// before letting the normal tab-close route remove its view.
function retireClosingView(owner, browserUi, view) {
    var capture = owner.nativePageCaptureSave
    if (!capture || !view || capture.view !== view
            // A finished Qt download may still be awaiting exact validation
            // and immutable-object installation. Let that callback retire it
            // before allowing the view that owns the archive to disappear.
            || capture.downloadFinished === true)
        return false
    if (!retireCaptureIdentities(owner, capture, [])) {
        failClosedRetirement(owner, browserUi, capture,
            capture.download ? [capture.download] : [],
            "Native page-save callback capacity is exhausted; window capture was stopped")
        return true
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.downloadFinished !== true && capture.download) {
        try { capture.download.cancel() } catch (error) {}
    }
    var continued = browserUi.fail_native_window_capture_page(
                capture.operationId, capture.tabId, "native-view-lost",
                capture.path, capture.downloadId)
    if (!continued) {
        browserUi.cancel_native_page_capture_request(capture.operationId)
        browserUi.status_text = "Window capture was cancelled because a page closed"
    } else {
        browserUi.status_text = "The closed page could not be archived; continuing the partial capture"
    }
    owner.scheduleRuntimeWork(0)
    return true
}

// A replacement Rust operation may become ready while QML still owns an old
// native save. Retire that old request before starting the replacement; a
// stale Qt callback must not be routed as an ordinary download or write into
// a stage path now owned by no capture.
function retireSupersededCapture(owner, browserUi, capture, replacementOperation) {
    var replacement = String(replacementOperation || "")
    if (!capture || owner.nativePageCaptureSave !== capture
            || replacement.length === 0
            || replacement === String(capture.operationId || "")) {
        return false
    }
    if (capture.downloadFinished !== true) {
        if (!retireCaptureIdentities(owner, capture, [])) {
            failClosedRetirement(owner, browserUi, capture,
                capture.download ? [capture.download] : [],
                "Native page-save callback capacity is exhausted; replacement capture was blocked")
            return false
        }
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.downloadFinished !== true && capture.download) {
        try { capture.download.cancel() } catch (error) {}
    }
    browserUi.cancel_native_page_capture_request(capture.operationId)
    return true
}

// A second or mismatched save request invalidates the capture. Quarantine the
// stage path and both request IDs before cancelling either Qt request: Qt may
// emit a synchronous or late finish signal during cancellation.
function rejectCaptureRequest(owner, browserUi, capture, download, downloadId) {
    if (!download)
        return false
    var id = String(downloadId || "")
    if (!capture || owner.nativePageCaptureSave !== capture) {
        // A capture can retire during Rust's synchronous request admission.
        // The new Qt request is still ours to reject, but never cancel a
        // different capture that became current in the meantime.
        var retired = owner.nativePageCaptureRetiredDownloads || ({})
        if (retired[id] === true)
            return false
        if (id.length > 0 && !owner.retireNativePageCaptureDownload(id)) {
            failClosedRetirement(owner, browserUi, null, [download],
                "Native page-save callback capacity is exhausted; page saves are blocked in this window")
            return false
        }
        try { download.cancel() } catch (error) {}
        return false
    }
    if (!retireCaptureIdentities(owner, capture, [id])) {
        failClosedRetirement(owner, browserUi, capture,
            capture.download && capture.download !== download
                ? [capture.download, download] : [download],
            "Native page-save callback capacity is exhausted; window capture was stopped")
        return true
    }
    owner.nativePageCaptureSave = null
    owner.discardNativePageCaptureMutationProbe(capture)
    if (capture.download && capture.download !== download) {
        try { capture.download.cancel() } catch (error) {}
    }
    try { download.cancel() } catch (error) {}
    browserUi.cancel_native_page_capture_request(capture.operationId)
    browserUi.status_text = "Page archive save did not match the captured page and was discarded"
    owner.scheduleRuntimeWork(0)
    return true
}
