import QtQuick
import QtQuick.Controls
import "../scripts/BrowserScripts.js" as BrowserScripts

Item {
    id: content
    required property var webView
    required property var browserUi
    required property var browserWindow
    required property var browserHost
    Timer { id: historyRefreshTimer; interval: 0; onTriggered: content.synchronizeHistory() }
    Timer { id: contentFocusTimer; interval: 0; onTriggered: content.focusContent() }
    property var tabsModel: null
    property string nativeUrl: ""
    property bool traversalPending: false
    property int requestedHistoryIndex: -1
    property bool engineTraversalPending: false
    property bool sameDocumentTraversalPending: false
    property int historyProbeEpoch: 0
    property bool historyProbePending: false
    property bool historyProbeAgain: false
    property var historyTraversalRequest: null
    property int sameDocumentNotificationSerial: 0
    property var pendingSameDocumentNotification: null
    property string historyDocument: ""
    property int historySequence: 0
    property bool navigationPending: false
    property bool committedContent: false
    property string engineLoadState: "idle"
    readonly property bool engineDocumentReady: engineLoadState === "complete"
    property bool engineDocumentHasFormData: false
    property bool navigationHasFormData: false
    property string engineDocumentUrl: ""
    property var fragmentVisit: null
    property var formReloadRequest: null
    property bool formReloadCreated: false
    property var reloadAttempt: null
    property var queuedReload: null
    property var blockedReload: null
    property var verifiedFormReload: null
    property var beforeUnloadCancellation: null
    property int navigationSerial: 0
    onEngineDocumentReadyChanged: if (!engineDocumentReady) fragmentVisit = null
    property int navigationBaseIndex: 0
    readonly property int tabIndex: webView ? webView.tabIndex : -1
    // A provisional or retired renderer must never borrow the active tab.
    function hasLiveOwner() {
        return !!browserUi && !!webView && tabIndex >= 0
            && typeof webView.ownsTab === "function" && webView.ownsTab()
    }
    onBrowserUiChanged: { invalidateHistoryProbe(); reportUndoFallback() }
    onTabIndexChanged: { invalidateHistoryProbe(); reportUndoFallback() }
    readonly property int engineHistoryIndex: historyCursor.count
    readonly property bool canNavigateBack: { if (!hasLiveOwner()) return false; var revision = browserUi.content_history_revision; return browserUi.content_history_available(tabIndex, -1) }
    readonly property bool canNavigateForward: { if (!hasLiveOwner()) return false; var revision = browserUi.content_history_revision; return browserUi.content_history_available(tabIndex, 1) }
    readonly property var startPage: startLoader.item
    readonly property bool undoFallbackReady: hasLiveOwner() && nativeUrl === "rb://start"
        && !!startPage && !startPage.inputActive && !startPage.closeNeedsConfirmation
        && !navigationPending && !traversalPending
    onUndoFallbackReadyChanged: reportUndoFallback()
    function reportUndoFallback() {
        if (browserUi && webView && typeof browserUi.report_undo_fallback === "function")
            browserUi.report_undo_fallback(String(webView.stableTabId || ""), undoFallbackReady)
    }
    Component.onCompleted: reportUndoFallback()
    property bool startCreated: false
    visible: true
    z: 3

    Instantiator {
        id: historyCursor
        model: content.webView.history.backItems
        onCountChanged: historyRefreshTimer.restart()
        delegate: QtObject { required property url url; required property int offset }
    }
    Instantiator {
        id: forwardCursor
        model: content.webView.history.forwardItems
        delegate: QtObject { required property url url; required property int offset }
    }
    // Own the complete side-button gesture in Normal mode. Letting WebEngine
    // handle it independently bypasses native-page anchors and can traverse
    // twice as the event crosses the Quick/renderer boundary.
    MouseArea {
        anchors.fill: parent
        z: 4
        enabled: content.hasLiveOwner() && content.browserUi.mode === "normal"
        acceptedButtons: Qt.BackButton | Qt.ForwardButton
        onClicked: function(mouse) {
            content.browserHost.submitInteractiveCommand(
                mouse.button === Qt.BackButton ? "back" : "forward")
        }
    }

    function engineOffset(destination, expected, backwards) {
        if (webView.url.toString() === destination && expected === 0) return 0
        // WebEngine can publish its history model after the URL notification.
        // Resolve against the settled model rather than a captured stale count.
        var models = backwards ? [historyCursor, forwardCursor] : [forwardCursor, historyCursor]
        var nearest = null
        for (var modelIndex = 0; modelIndex < models.length; ++modelIndex) {
            for (var index = 0; index < models[modelIndex].count; ++index) {
                var entry = models[modelIndex].objectAt(index)
                if (!entry || entry.url.toString() !== destination) continue
                if (entry.offset === expected) return expected
                if (nearest === null || Math.abs(entry.offset) < Math.abs(nearest)) nearest = entry.offset
            }
            if (nearest !== null) return nearest
        }
        return null
    }
    function engineEntry(index) {
        if (index === engineHistoryIndex) return { url: webView.url.toString(), engine_index: index }
        for (var row = 0; row < historyCursor.count; ++row) {
            var entry = historyCursor.objectAt(row)
            if (entry && engineHistoryIndex + entry.offset === index) return { url: entry.url.toString(), engine_index: index }
        }
        return null
    }
    function synchronizeHistory() {
        if (!hasLiveOwner()) return
        settleFragmentVisit()
        if (nativeUrl.length > 0) { finishHistoryTraversal(true); return }
        if (typeof webView.runJavaScript !== "function" || typeof browserWindow.browserScriptWorld !== "number") {
            if (!webView.loading && (!navigationPending || browserUi.load_state === "complete")) reconcileHistory()
            finishHistoryTraversal(!webView.loading)
            return
        }
        if (historyProbePending) { historyProbeAgain = true; return }
        var epoch = historyProbeEpoch
        var destination = webView.url.toString()
        var position = engineHistoryIndex
        var identityIndex = tabIndex
        var identityTabId = identityIndex >= 0
                ? String(browserUi.tab_id_for_index(identityIndex)) : ""
        var identityInputEpoch = Number(browserUi.input_epoch) || 0
        historyProbePending = true
        var launched = browserWindow.runBrowserScript(webView, BrowserScripts.historyTraversalProbe(), function(value) {
            if (epoch !== historyProbeEpoch) return
            historyProbePending = false
            if (!hasLiveOwner()) { historyProbeAgain = false; historyTraversalRequest = null; return }
            if (nativeUrl.length > 0) return
            if (destination !== webView.url.toString() || position !== engineHistoryIndex) {
                historyRefreshTimer.restart()
                return
            }
            var provenDocument = value && typeof value.document === "string" && value.document.length > 0
                    && typeof value.sequence === "number" && value.sequence >= 0
                    && typeof value.url === "string" && value.url.length > 0
            // location.href is encoded; QML's native QUrl text is decoded.
            // Use Qt's representation on both sides without decoding separators.
            var observedUrl = provenDocument ? Qt.resolvedUrl(value.url).toString() : ""
            var matchesDestination = provenDocument && observedUrl === destination
            if (matchesDestination) {
                if (engineDocumentReady && historyDocument === value.document)
                    engineDocumentUrl = destination
                var notification = pendingSameDocumentNotification
                var identityMatches = !!notification
                        && notification.probeEpoch === epoch
                        && notification.destination === destination
                        && notification.index === identityIndex
                        && notification.tabId === identityTabId
                        && notification.document.length > 0
                        && notification.document === value.document
                        && observedUrl === notification.destination
                        && !webView.loading && !navigationPending
                        && browserUi.load_state === "complete"
                if (!webView.loading && identityTabId.length > 0
                        && typeof browserUi.page_document_identity_for === "function") {
                    browserUi.page_document_identity_for(
                        identityIndex, identityTabId, value.document, identityInputEpoch)
                }
                var baseline = historyDocument === value.document ? historySequence : 0
                if (value.sequence > baseline) sameDocumentTraversalPending = true
                historyDocument = value.document
                historySequence = value.sequence
                if (notification
                        && notification.serial === sameDocumentNotificationSerial) {
                    pendingSameDocumentNotification = null
                    if (notification.destination === webView.url.toString()
                            && notification.index === tabIndex
                            && notification.tabId === String(browserUi.tab_id_for_index(tabIndex))) {
                        if (typeof browserUi.complete_same_document_url_change_for === "function") {
                            browserUi.complete_same_document_url_change_for(
                                notification.index, notification.tabId,
                                notification.destination,
                                identityMatches ? value.document : "",
                                notification.inputEpoch)
                        }
                    }
                }
            } else {
                settleUnprovenSameDocumentChange(epoch, destination)
            }
            if (matchesDestination || (!webView.loading && (!navigationPending || browserUi.load_state === "complete"))) {
                reconcileHistory()
                finishHistoryTraversal(true)
            } else {
                // Before headers the renderer still shows the previous visit.
                // Its history position remains the basis for Back/Forward.
                var current = null
                try {
                    var snapshot = JSON.parse(browserUi.content_history_export(tabIndex))
                    current = snapshot.entries[snapshot.cursor]
                } catch (error) {}
                finishHistoryTraversal(provenDocument && current && current.url === observedUrl)
            }
            if (historyProbeAgain) { historyProbeAgain = false; historyRefreshTimer.restart() }
        }, browserWindow.browserScriptWorld)
        if (!launched) { historyProbePending = false; finishHistoryTraversal(false) }
    }
    function requestHistoryTraversal(backwards, count, perform) {
        if (!hasLiveOwner()) return
        var offset = (backwards ? -1 : 1) * Math.max(1, Math.min(100, Number(count) || 1))
        var pending = historyTraversalRequest
        if (pending && reloadOwnerCurrent(pending.owner)) {
            pending.offset = Math.max(-100, Math.min(100, pending.offset + offset))
        } else {
            var target = browserUi.navigation_target_for_index(tabIndex)
            if (!target) return
            historyTraversalRequest = {owner: reloadSnapshot(target, false), offset: offset, perform: perform}
        }
        synchronizeHistory()
    }
    function finishHistoryTraversal(ready) {
        var pending = historyTraversalRequest
        if (!pending || !reloadOwnerCurrent(pending.owner)) {
            historyTraversalRequest = null
            return
        }
        // A returned document may already be proven while the native history
        // model still describes its predecessor. Keep subsequent commands
        // queued so their offsets resolve against the requested position.
        if (ready && traversalPending && requestedHistoryIndex >= 0
                && engineHistoryIndex !== requestedHistoryIndex) return
        historyTraversalRequest = null
        if (!ready) { browserUi.status_text = "History is changing; try again"; return }
        if (pending.offset !== 0) pending.perform(pending.offset < 0, Math.abs(pending.offset))
    }
    function invalidateHistoryProbe() {
        historyProbeEpoch += 1
        historyProbePending = false
        historyProbeAgain = false
        historyTraversalRequest = null
        sameDocumentTraversalPending = false
        pendingSameDocumentNotification = null
    }
    function reconcileHistory() {
        if (!hasLiveOwner()) return
        var snapshot
        try { snapshot = JSON.parse(browserUi.content_history_export(tabIndex)) } catch (error) { return }
        if (!snapshot || !snapshot.entries || !snapshot.entries[snapshot.cursor]) return
        var current = snapshot.entries[snapshot.cursor]
        var destination = webView.url.toString()
        var loadPending = webView.loading || browserUi.load_state === "provisional"
        var nativeCurrent = current.url === "rb://start" || current.url === "rb://blank"
        if (traversalPending && requestedHistoryIndex >= 0) {
            if (engineHistoryIndex !== requestedHistoryIndex
                    || (loadPending && current.url !== destination)) return
            browserUi.content_navigated(tabIndex, destination, engineHistoryIndex, true)
            requestedHistoryIndex = -1
            sameDocumentTraversalPending = false
            navigationPending = navigationPending && loadPending
            traversalPending = false
            return
        }
        // Browser traversal has already selected the logical destination.
        // Chromium publishes its URL and history index independently; neither
        // intermediate pair may replace that entry or discard its visit ID.
        if (traversalPending && loadPending
                && (current.url !== destination || current.engine_index !== engineHistoryIndex)) return
        var observedBack = false
        if (!nativeCurrent && current.engine_index > engineHistoryIndex) {
            observedBack = current.url !== destination
            for (var earlier = 0; !observedBack && earlier < snapshot.cursor; ++earlier) {
                var entry = snapshot.entries[earlier]
                observedBack = entry.url === destination && entry.engine_index === engineHistoryIndex
            }
        }
        var observedTraversal = sameDocumentTraversalPending && current.engine_index !== engineHistoryIndex
        sameDocumentTraversalPending = false
        if (!nativeCurrent && (engineTraversalPending || observedBack || observedTraversal)) {
            // A history request can commit before the history models move.
            // Wait for their new position instead of recording a new visit.
            if (engineTraversalPending && current.engine_index === engineHistoryIndex) return
            if (browserUi.content_engine_traversed(tabIndex, destination, engineHistoryIndex)) {
                engineTraversalPending = false; navigationPending = navigationPending && loadPending; traversalPending = false
                return
            }
        }
        if (current.url !== destination) {
            if (!nativeCurrent && current.engine_index === engineHistoryIndex) {
                // replaceState changes this engine entry rather than adding one.
                current.url = destination
            } else {
                var base = nativeCurrent ? navigationBaseIndex
                        : engineHistoryIndex + engineOffset(current.url, current.engine_index - engineHistoryIndex, true)
                if (!nativeCurrent && base >= engineHistoryIndex) return
                if (!nativeCurrent) current.engine_index = base
                snapshot.entries.length = snapshot.cursor + 1
                for (var added = base + 1; added < engineHistoryIndex; ++added) {
                    var intermediate = engineEntry(added)
                    if (intermediate) snapshot.entries.push(intermediate)
                }
                snapshot.entries.push({url: destination, engine_index: engineHistoryIndex})
                snapshot.cursor = snapshot.entries.length - 1
            }
            while (snapshot.entries.length > 4096) { snapshot.entries.shift(); snapshot.cursor -= 1 }
            browserUi.content_history_import(tabIndex, JSON.stringify(snapshot))
            navigationPending = navigationPending && loadPending; traversalPending = false
            return
        }
        if (current.engine_index === engineHistoryIndex) { navigationPending = navigationPending && loadPending; traversalPending = false; return }
        var previous = snapshot.entries[snapshot.cursor - 1]
        var baseline = navigationBaseIndex
        if (previous && previous.url !== "rb://start" && previous.url !== "rb://blank") {
            baseline = engineHistoryIndex + engineOffset(previous.url, previous.engine_index - engineHistoryIndex, true)
            previous.engine_index = baseline
        }
        var old = engineEntry(current.engine_index)
        // The first engine entry has no earlier logical baseline. A push
        // of the same URL still creates a distinct, traversable state there.
        var preserveCurrent = old && old.url === current.url
                && (baseline < current.engine_index || snapshot.cursor === 0)
        if (preserveCurrent && engineHistoryIndex > current.engine_index) {
            // A new pushState branches here, even if its URL matches an old
            // forward entry. A trusted popstate was handled above instead.
            snapshot.entries.length = snapshot.cursor + 1
        }
        var segment = []
        for (var index = (preserveCurrent ? current.engine_index : baseline) + 1; index < engineHistoryIndex; ++index) {
            var missing = engineEntry(index)
            if (missing) segment.push(missing)
        }
        segment.push({ url: current.url, engine_index: engineHistoryIndex })
        var insertion = snapshot.cursor + (preserveCurrent ? 1 : 0)
        snapshot.entries.splice.apply(snapshot.entries, [insertion, preserveCurrent ? 0 : 1].concat(segment))
        snapshot.cursor += segment.length - (preserveCurrent ? 0 : 1)
        if (snapshot.entries.length > 4096) {
            var removed = snapshot.entries.length - 4096
            snapshot.entries.splice(0, removed); snapshot.cursor -= removed
        }
        browserUi.content_history_import(tabIndex, JSON.stringify(snapshot))
    }
    function focusContent() { if (hasLiveOwner() && nativeUrl === "rb://start" && startPage) startPage.focusContent() }
    function requestReload(request) {
        if (!hasLiveOwner()) return
        if (queuedReload && reloadOwnerCurrent(queuedReload)) {
            browserUi.status_text = "Reload pending"
            return
        }
        queuedReload = reloadSnapshot(request, false)
        reloadDispatchTimer.restart()
    }
    Timer {
        id: reloadDispatchTimer
        interval: 0
        onTriggered: {
            var pending = content.queuedReload
            content.queuedReload = null
            // Qt changes loading before queuing its load notification. Let
            // that fact settle before choosing the current entry's preflight.
            if (content.reloadOwnerCurrent(pending)) content.dispatchReload(pending.target)
        }
    }
    function dispatchReload(request) {
        if (!hasLiveOwner()) return
        if (formReloadRequest) {
            if (reloadOwnerCurrent(formReloadRequest)) restoreReloadState(formReloadRequest.state)
            browserUi.status_text = "Confirm form resubmission"
            return
        }
        if (blockedReload || reloadAttempt) {
            browserUi.status_text = "Reload pending"
            return
        }
        var failedForm = engineLoadState === "failed" && navigationHasFormData
        var verifiedForm = verifiedFormReload && reloadOwnerCurrent(verifiedFormReload)
        if (!(engineDocumentReady && engineDocumentHasFormData) && !failedForm && !verifiedForm) {
            performReload(request, false)
            return
        }
        showFormReload(reloadSnapshot(request, false), true)
    }
    function reloadSnapshot(request, approved) {
        return { target: request, view: webView, ui: browserUi, host: browserHost,
            documentOwner: webView.documentRequestOwner, url: webView.url.toString(),
            content: content, serial: navigationSerial, state: engineLoadState,
            wasLoading: webView.loading, approved: approved }
    }
    function navigationOwnerCurrent(pending) {
        return hasLiveOwner() && pending && pending.view === webView && pending.ui === browserUi
            && pending.host === browserHost && pending.serial === navigationSerial
            && webView.viewUi === pending.ui && webView.viewHost === pending.host
            && pending.documentOwner === webView.documentRequestOwner
            && browserUi.request_target_current(pending.target.tabId, pending.target.generation, pending.target.documentId)
    }
    function reloadOwnerCurrent(pending) {
        return navigationOwnerCurrent(pending) && pending.url === webView.url.toString()
    }
    function restoreReloadState(state) {
        if (state === "complete") browserUi.navigation_completed_for(tabIndex)
        else if (state === "failed") browserUi.navigation_failed_for(tabIndex)
        else if (state === "stopped") browserUi.navigation_stopped_for(tabIndex)
        // A response still arriving keeps its provisional state and connection.
    }
    function showFormReload(pending, restoreState) {
        if (!reloadOwnerCurrent(pending)) return
        if (restoreState) restoreReloadState(pending.state)
        if (tabIndex !== browserUi.active_tab_index) {
            browserUi.status_text = "Select this tab to confirm resending its form"
            return
        }
        if (!formReloadRequest) {
            formReloadRequest = pending
            formReloadCreated = true
        }
        browserUi.status_text = "Confirm form resubmission"
    }
    function performReload(request, approved) {
        if (!hasLiveOwner()) return
        reloadAttempt = reloadSnapshot(request, approved)
        if (request.bypassCache) webView.reloadAndBypassCache()
        else webView.reload()
    }
    // Qt reports the method of the entry it will actually reload. A pending
    // form may still reload its previous GET; a committed streaming POST does not.
    function interceptReloadNavigation(request) {
        var pending = reloadAttempt
        reloadAttempt = null
        if (!pending) {
            if (request.hasFormData && (formReloadRequest || blockedReload)) {
                request.reject()
                return true
            }
            return false
        }
        if (!reloadOwnerCurrent(pending) || request.url.toString() !== pending.url) {
            request.reject()
            return true
        }
        if (!request.hasFormData || pending.approved) return false
        request.reject()
        blockedReload = pending
        verifiedFormReload = pending
        // Rejecting Reload during an active response leaves that response
        // running. An idle rejection first emits a transient start/stop pair.
        if (pending.wasLoading) reloadPresentationTimer.restart()
        return true
    }
    function blockedReloadLoading(stage) {
        var pending = blockedReload
        if (!pending) return false
        if (!reloadOwnerCurrent(pending)) { blockedReload = null; return false }
        if (stage === "started") return true
        if (stage === "stopped") {
            if (pending.wasLoading || (pending.state !== "complete" && pending.state !== "failed")) {
                engineLoadState = "stopped"
                pending.state = "stopped"
            }
            pending.wasLoading = false
            restoreReloadState(pending.state)
            reloadPresentationTimer.restart()
            return true
        }
        return false
    }
    function updateReloadSnapshots() {
        var pending = [formReloadRequest, blockedReload, verifiedFormReload]
        for (var i = 0; i < pending.length; ++i) {
            if (!reloadOwnerCurrent(pending[i])) continue
            pending[i].state = engineLoadState
            pending[i].wasLoading = webView.loading
        }
        if (blockedReload) reloadPresentationTimer.restart()
    }
    function cancelReloadAttempt() {
        var pending = reloadAttempt || verifiedFormReload
        reloadAttempt = null
        if (!reloadOwnerCurrent(pending)) return
        // An approved native reload can reach BeforeUnload before another
        // load starts. Stay may stop the original response without a load
        // notification; retain the verified entry and settle the stopped view.
        if (engineLoadState === "loading" && !webView.loading) noteStopped()
        else restoreReloadState(pending.state)
    }
    function noteBeforeUnloadRejected() {
        beforeUnloadCancellation = null
        if (hasLiveOwner() && nativeUrl.length === 0) {
            requestedHistoryIndex = -1
            traversalPending = false
            var target = browserUi.navigation_target_for_index(tabIndex)
            if (target) beforeUnloadCancellation = reloadSnapshot(target, false)
        }
        cancelReloadAttempt()
        var pending = beforeUnloadCancellation
        if (reloadOwnerCurrent(pending) && !webView.loading) {
            if (pending.state === "complete" || pending.state === "failed") restoreReloadState(pending.state)
            else if (engineLoadState !== "stopped") noteStopped()
        }
    }
    function settleBeforeUnloadLoading(stage, info) {
        var pending = beforeUnloadCancellation
        if (!pending) return false
        if (stage === "started") { beforeUnloadCancellation = null; return false }
        if (stage !== "stopped" && stage !== "succeeded" && stage !== "failed") return false
        beforeUnloadCancellation = null
        // Qt can finish an interrupted streaming response as failed with no
        // error facts. Only an explicit Stay on this exact document owns it.
        if ((stage !== "failed" && stage !== "stopped") || !navigationOwnerCurrent(pending)
                || webView.loading || info.isDownload === true || info.isErrorPage === true
                || info.url.toString() !== webView.url.toString()) return false
        if (stage === "failed" && (Number(info.errorCode) !== 0 || Number(info.errorDomain) !== 0
                || String(info.errorString || "").length > 0)) return false
        if (pending.state === "complete" || pending.state === "failed") {
            engineLoadState = pending.state
            navigationPending = false
            updateReloadSnapshots()
            restoreReloadState(pending.state)
        } else noteStopped()
        if (tabIndex === browserUi.active_tab_index) browserUi.status_text = "Stayed on page"
        return true
    }
    function noteStopped() {
        if (!hasLiveOwner()) return
        engineLoadState = "stopped"
        navigationPending = false
        requestedHistoryIndex = -1
        traversalPending = false
        updateReloadSnapshots()
        browserUi.navigation_stopped_for(tabIndex)
        historyRefreshTimer.restart()
    }
    Timer {
        id: reloadPresentationTimer
        interval: 0
        onTriggered: {
            var pending = content.blockedReload
            if (!pending || (!pending.wasLoading && content.webView.loading)) return
            content.blockedReload = null
            content.showFormReload(pending, false)
        }
    }
    function settleFragmentVisit() {
        var visit = fragmentVisit
        if (!visit || !hasLiveOwner() || nativeUrl.length > 0 || webView.loading
                || browserUi !== visit.ui
                || webView.url.toString() !== visit.destination
                || browserUi.tab_id_for_index(tabIndex) !== visit.tabId
                || (visit.changed && engineHistoryIndex <= visit.baseIndex)) return
        fragmentVisit = null
        navigationHasFormData = visit.hasFormData
        engineDocumentUrl = visit.destination
        browserUi.navigation_committed_for(tabIndex, visit.destination, webView.title)
        browserUi.navigation_completed_for(tabIndex)
        // Explicit Open admitted a new target while keeping this renderer.
        // No load-success notification will refresh its live-title owner.
        webView.documentTitleTarget = browserUi.navigation_target_for_index(tabIndex)
        // The history model now includes a changed fragment's new entry.
        // An unchanged anchor keeps the current entry and document method.
        noteCommit()
    }
    function setContentUrl(destination, traversal, engineMoves, freshRequest, historyIndex) {
        if (!hasLiveOwner()) return
        // Typed commands carry canonical URLs; the view and history expose
        // Qt's native text. Keep fragment ownership in that same representation.
        destination = Qt.resolvedUrl(destination).toString()
        fragmentVisit = null
        invalidateHistoryProbe()
        engineTraversalPending = false
        requestedHistoryIndex = traversal && engineMoves && typeof historyIndex === "number"
                && historyIndex >= 0 ? historyIndex : -1
        if (destination === "rb://start" || destination === "rb://blank") {
            webView.stop()
            nativeUrl = destination
            committedContent = true
            traversalPending = false
            navigationPending = false
            browserUi.content_navigated(tabIndex, destination, engineHistoryIndex, !!traversal)
            if (tabsModel && tabIndex >= 0 && tabIndex < tabsModel.count) {
                tabsModel.setProperty(tabIndex, "url", destination)
                tabsModel.setProperty(tabIndex, "title", destination === "rb://start" ? "Start page" : "Blank")
            }
            if (destination === "rb://start") {
                startCreated = true
                if (startPage) startPage.refresh()
                contentFocusTimer.restart()
            }
        } else {
            if (!traversal) navigationBaseIndex = engineHistoryIndex
            nativeUrl = ""
            if (typeof browserWindow.installFocusObserver === "function") browserWindow.installFocusObserver(webView)
            traversalPending = !!traversal
            var sameDocumentVisit = freshRequest && engineDocumentReady
                && !webView.loading && destination.indexOf("#") >= 0
                && destination.split("#")[0] === engineDocumentUrl.split("#")[0]
            // A completed document keeps its renderer for fragment visits.
            // Chromium may emit no loading or URL change event for the same
            // anchor, so explicitly settle the browser's navigation request.
            if (sameDocumentVisit && !traversal) {
                navigationPending = true
                fragmentVisit = {
                    destination: destination,
                    changed: webView.url.toString() !== destination,
                    baseIndex: engineHistoryIndex,
                    hasFormData: engineDocumentHasFormData,
                    ui: browserUi,
                    tabId: browserUi.tab_id_for_index(tabIndex)
                }
                webView.url = destination
                settleFragmentVisit()
            } else if ((webView.url.toString() !== destination || freshRequest) && !traversal) {
                // Explicit Open requests reach the engine even for the same
                // address. URL assignment starts GET; Reload could resend POST.
                // Model synchronization omits freshRequest and stays idempotent.
                navigationPending = true
                webView.url = destination
            } else if (!traversal && committedContent && !navigationPending && !webView.loading) {
                browserUi.navigation_committed_for(tabIndex, destination, webView.title)
                browserUi.navigation_completed_for(tabIndex)
                noteCommit()
            }
            if (tabsModel && tabIndex >= 0 && tabIndex < tabsModel.count) tabsModel.setProperty(tabIndex, "url", destination)
            if (traversal && !engineMoves && webView.url.toString() === destination) {
                noteSameDocument(destination)
                browserUi.navigation_committed_for(tabIndex, destination, webView.title)
                browserUi.navigation_completed_for(tabIndex)
                browserUi.content_navigated(tabIndex, destination, engineHistoryIndex, true)
                traversalPending = false
            }
        }
    }
    function beginNavigation(engineTraversal, hasFormData, reloading) {
        if (!hasLiveOwner()) return
        if (nativeUrl.length > 0) return
        browserUi.navigation_kind_for(tabIndex, !!engineTraversal)
        if (!engineTraversal) {
            requestedHistoryIndex = -1
            traversalPending = false
        }
        var retainedForm = reloading && hasFormData && reloadOwnerCurrent(verifiedFormReload)
                ? verifiedFormReload : null
        navigationSerial += 1
        beforeUnloadCancellation = null
        blockedReload = null
        // Reload policy does not replace the document. BeforeUnload can still
        // cancel it; a real load start changes documentRequestOwner instead.
        verifiedFormReload = retainedForm
        if (retainedForm) retainedForm.serial = navigationSerial
        invalidateHistoryProbe()
        navigationPending = true
        engineTraversalPending = !!engineTraversal && !traversalPending
        if (engineTraversal) fragmentVisit = null
        if (typeof hasFormData === "boolean") navigationHasFormData = hasFormData
    }
    function reportUrlChange(index, destination) {
        if (!hasLiveOwner()) return
        if (index >= 0 && typeof browserUi.navigation_url_changed_for === "function") {
            browserUi.navigation_url_changed_for(index, destination)
        } else {
            browserUi.navigation_url_changed(destination)
        }
    }
    function settleUnprovenSameDocumentChange(epoch, probedDestination) {
        if (!hasLiveOwner()) return
        var notification = pendingSameDocumentNotification
        if (!notification || notification.serial !== sameDocumentNotificationSerial
                || notification.probeEpoch !== epoch
                || notification.destination !== probedDestination
                || notification.destination !== webView.url.toString()
                || notification.index !== tabIndex
                || notification.tabId !== String(browserUi.tab_id_for_index(tabIndex))) {
            return
        }
        pendingSameDocumentNotification = null
        if (typeof browserUi.complete_same_document_url_change_for === "function") {
            browserUi.complete_same_document_url_change_for(
                notification.index, notification.tabId,
                notification.destination, "", notification.inputEpoch)
        }
    }
    function noteSameDocument(destination) {
        if (!hasLiveOwner()) return
        if (typeof destination !== "string" || destination.length === 0) {
            historyRefreshTimer.restart()
            return
        }
        var index = tabIndex
        var tabId = index >= 0 ? String(browserUi.tab_id_for_index(index)) : ""
        var notification = {
            serial: ++sameDocumentNotificationSerial,
            probeEpoch: historyProbeEpoch,
            destination: destination,
            index: index,
            tabId: tabId,
            inputEpoch: Number(browserUi.input_epoch) || 0,
            document: historyDocument
        }
        if (nativeUrl.length > 0 || index < 0 || tabId.length === 0
                || typeof webView.runJavaScript !== "function"
                || typeof browserWindow.browserScriptWorld !== "number"
                || typeof browserUi.begin_same_document_url_change_for !== "function"
                || webView.loading || navigationPending
                || browserUi.load_state !== "complete" || historyDocument.length === 0) {
            pendingSameDocumentNotification = null
            reportUrlChange(index, destination)
            historyRefreshTimer.restart()
            return
        }
        pendingSameDocumentNotification = notification
        browserUi.begin_same_document_url_change_for(
            index, tabId, destination, historyDocument, notification.inputEpoch)
        historyRefreshTimer.restart()
    }
    function noteCommit(succeeded) {
        if (!hasLiveOwner()) return
        if (nativeUrl.length > 0) return
        navigationPending = false
        committedContent = true
        if (typeof succeeded === "boolean") {
            engineLoadState = succeeded ? "complete" : "failed"
            if (succeeded) {
                engineDocumentHasFormData = navigationHasFormData
                engineDocumentUrl = webView.url.toString()
            }
            updateReloadSnapshots()
        }
        // Load success can precede the history model's move. Keep the selected
        // entry and its expected position until the native facts agree.
        if (succeeded === true && traversalPending && requestedHistoryIndex >= 0
                && engineHistoryIndex !== requestedHistoryIndex) {
            historyRefreshTimer.restart()
            return
        }
        if (!engineTraversalPending) browserUi.content_navigated(tabIndex, webView.url.toString(), engineHistoryIndex, traversalPending)
        requestedHistoryIndex = -1
        traversalPending = false
        historyRefreshTimer.restart()
    }
    Rectangle { anchors.fill: parent; visible: content.nativeUrl.length > 0; color: content.browserWindow.backgroundColor }
    Loader {
        parent: content.browserHost.contentItem
        anchors.fill: parent
        z: 65
        active: content.formReloadCreated
        sourceComponent: FerricFormResubmission {
            browserWindow: content.browserHost.chromeAppearance || content.browserWindow
            hostWindow: content.browserHost
            browserUi: content.browserUi
            reloadRequest: content.formReloadRequest
            onFinished: function(resubmit) {
                var pending = content.formReloadRequest
                var current = targetCurrent()
                if (resubmit && current) content.browserUi.clear_navigation_failure()
                content.formReloadRequest = null
                if (!pending || !current) return
                if (resubmit) content.performReload(pending.target, true)
                else content.browserUi.status_text = "Reload cancelled"
            }
        }
    }
    Loader {
        id: startLoader
        anchors.fill: parent
        active: content.startCreated
        visible: content.nativeUrl === "rb://start"
        enabled: visible
        onLoaded: contentFocusTimer.restart()
        sourceComponent: FerricStartPage {
            browserUi: content.browserUi
            browserWindow: content.browserWindow
            browserHost: content.browserHost
            contentView: content.webView
        }
    }
    Image {
        x: -10000
        width: 32; height: 32
        source: content.nativeUrl.length === 0 ? content.webView.icon : ""
        sourceSize: Qt.size(32, 32)
        // Keep the capture item outside the viewport; opacity would be baked
        // into the cached image. The provider reads already-loaded engine icons.
        visible: true
        onStatusChanged: {
            if (status !== Image.Ready || content.nativeUrl.length > 0) return
            var path = content.browserUi.start_page_icon_path(content.webView.url.toString(), true)
            if (path.length > 0) grabToImage(function(result) { result.saveToFile(path) })
        }
    }
}
