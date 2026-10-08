import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/ShutdownPageProbeState.js" as ShutdownPageProbeState
import "../scripts/VersionedRestorePresentation.js" as VersionedRestorePresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation
import "../scripts/ProfileDownloadRouting.js" as ProfileDownloadRouting
import "../scripts/CleanBranchPreparation.js" as CleanBranchPreparation
import "../scripts/CleanBranchSwitch.js" as CleanBranchSwitch

FerricBrowserRuntimeServices {
    id: window

    readonly property var activationOutcomeTimer: window.activationOutcomeTimerObject
    readonly property var fileDialogSurfaces: window.fileDialogSurfacesObject
    readonly property var certificatePrompts: window.certificatePromptsObject
    readonly property var webAuthPrompt: window.webAuthPromptObject
    readonly property var contextMenu: window.contextMenuObject
    readonly property var tabs: window.tabsModel
    readonly property var namedSessions: window.namedSessionsModel
    readonly property var profiles: window.profilesModel
    readonly property var downloads: window.downloadsModel
    readonly property var libraryEntries: window.libraryEntriesModel
    readonly property var libraryGraphEntries: window.libraryGraphEntriesModel
    readonly property var libraryGraphNodes: window.libraryGraphNodesModel

    readonly property alias switcherRefreshTimerObject: switcherRefreshTimer
    readonly property alias switcherBatchTimerObject: switcherBatchTimer

    function refreshDownloads() {
        downloads.clear()
        var values = browserUi.list_downloads()
        if (values.length === 0) {
            return
        }
        var lines = values.split("\n")
        for (var i = 0; i < lines.length; ++i) {
            var fields = lines[i].split("\t")
            if (fields.length >= 4) {
                var live = window.liveDownloadForId(fields[0])
                var metrics = window.downloadMetrics(fields[0], live)
                downloads.append({
                    id: fields[0], state: fields[1], bytes: fields[2], destination: fields[3],
                    total: metrics.total, speed: metrics.speed, reason: metrics.reason
                })
            }
        }
    }

    function liveDownloadForId(id) {
        try {
            var browserEntries = window.browserWindowRegistry
            var popupEntries = window.rootPopupEntriesSnapshot()
            return ProfileDownloadRouting.liveRequestForId(
                        id, window, browserEntries, popupEntries, window.browserProfile)
        } catch (error) {
            return undefined
        }
    }

    function downloadMetrics(id, download) {
        var requested = String(id)
        if (!download) {
            delete window.downloadSamples[requested]
            return { total: -1, speed: 0, reason: "" }
        }
        var now = Date.now()
        var bytes = Math.max(0, Number(download.receivedBytes || 0))
        var sample = window.downloadSamples[requested]
        var speed = 0
        if (sample && now > sample.at && bytes >= sample.bytes) {
            var elapsed = now - sample.at
            if (elapsed >= 200) {
                speed = Math.max(0, Math.round((bytes - sample.bytes) * 1000 / elapsed))
            }
        }
        window.downloadSamples[requested] = { at: now, bytes: bytes }
        var total = Number(download.totalBytes)
        return {
            total: isFinite(total) && total >= 0 ? total : -1,
            speed: speed,
            reason: window.boundedPageDialogText(download.interruptReasonString)
        }
    }

    function formatDownloadRate(bytesPerSecond) {
        var value = Number(bytesPerSecond)
        if (!isFinite(value) || value <= 0) {
            return ""
        }
        var units = ["B/s", "KiB/s", "MiB/s", "GiB/s"]
        var index = 0
        while (value >= 1024 && index < units.length - 1) {
            value /= 1024
            index += 1
        }
        return value.toFixed(index === 0 ? 0 : 1) + " " + units[index]
    }

    function openDownload(id, reveal) {
        if (!browserUi.download_desktop_action(id, reveal)) {
            return
        }
        if (browserUi.download_desktop_uri.length > 0) {
            window.openExternalUri(browserUi, browserUi.download_desktop_uri)
        }
    }

    function externalUriAllowed(uri) {
        var value = String(uri || "")
        if (value.length === 0 || value.length > 8192
                || /[\u0000-\u001f\u007f\n\r]/.test(value)) {
            return false
        }
        var match = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(value)
        if (!match) {
            return false
        }
        var scheme = match[1].toLowerCase()
        if (scheme === "file") {
            return /^file:\/\/\/[^/]/i.test(value)
        }
        return scheme === "mailto" || scheme === "tel"
                || scheme === "sms" || scheme === "geo"
    }

    function openExternalUri(ui, uri) {
        var targetUi = ui || browserUi
        if (!uri || String(uri).length === 0) {
            return false
        }
        if (!window.externalUriAllowed(uri)) {
            targetUi.status_text = "External URI rejected by scheme policy"
            return false
        }
        if (window.desktopPortalMode(targetUi) === "required") {
            var status = window.desktopPortalCapabilityStatus(targetUi, "open_uri")
            if (status === "not-probed") {
                browserUi.probe_desktop_portals()
                status = window.desktopPortalCapabilityStatus(targetUi, "open_uri")
            }
            if (status === "pending" || status === "not-probed") {
                var pending = window.pendingExternalUris || []
                pending.push({
                    ui: targetUi,
                    uri: String(uri),
                    deadlineMs: Date.now() + 5000
                })
                window.pendingExternalUris = pending.slice(-16)
                targetUi.status_text = "Checking required OpenURI portal…"
                externalOpenPortalTimer.start()
                return true
            }
            if (status !== "available") {
                targetUi.status_text = status === "pending"
                        ? "Checking required OpenURI portal; external action cancelled"
                        : "Required OpenURI portal unavailable; external action cancelled"
                return false
            }
        }
        Qt.openUrlExternally(String(uri))
        return true
    }

    function requestDownloadAction(id, action) {
        var actionIds = {
            pause: "browser.download.pause",
            resume: "browser.download.resume",
            cancel: "browser.download.cancel",
            retry: "browser.download.retry"
        }
        var actionId = actionIds[action]
        if (!actionId) {
            browserUi.status_text = "Unknown download action"
            return
        }
        if (browserUi.execute_ui_action(actionId, id)) {
            window.executePendingEngineAction()
        }
    }

    function hasActiveDownloads() {
        if (window.pendingNativePageCaptureClose || window.nativePageCaptureSave) {
            return true
        }
        if (window.pendingDownloadId.length > 0) {
            return true
        }
        return ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    window.pendingDownloadRequests)
                || ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    window.activeDownloads)
                || ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    window.downloadCompletionRequests)
    }

    function hasActiveShutdownRequestsFor(ui, hostWindow) {
        if (!ui) {
            return false
        }
        if (hostWindow === window && ui === browserUi
                && typeof ui.has_native_page_capture_request === "function"
                && ui.has_native_page_capture_request()) {
            return true
        }
        if (window.pendingFileDialogUi === ui
                || window.pendingPermissionUi === ui
                || window.pendingPageDialogUi === ui
                || window.pendingClientCertificateUi === ui
                || window.pendingCertificateErrorUi === ui
                || window.pendingWebAuthUi === ui
                || window.pendingContextMenuUi === ui) {
            return true
        }
        if (window.pendingDesktopMediaHost === hostWindow) {
            return true
        }
        return (window.captureSessions || []).some(function(session) {
            return session && session.host === hostWindow && session.status === "active"
        })
    }

    function cancelShutdownRequestsFor(ui, hostWindow) {
        if (!ui) {
            return false
        }
        var cancelled = false
        if (window.pendingFileDialogUi === ui) {
            if (hostWindow === window) {
                window.clearEngineFileDialog(true, "File selection cancelled for shutdown")
            } else if (hostWindow && hostWindow.pendingFileDialogView
                       && hostWindow.clearFileDialogForView) {
                hostWindow.clearFileDialogForView(hostWindow.pendingFileDialogView)
            }
            cancelled = true
        } else if (hostWindow && hostWindow !== window
                   && hostWindow.pendingFileDialogRequest
                   && hostWindow.clearFileDialogForView) {
            hostWindow.clearFileDialogForView(hostWindow.pendingFileDialogView)
            cancelled = true
        }
        if (window.pendingPermissionUi === ui) {
            window.cancelPermissionForUi(ui)
            cancelled = true
        }
        if (window.pendingPageDialogUi === ui) {
            window.clearPageDialogForView(window.pendingPageDialogView)
            cancelled = true
        }
        if (window.pendingClientCertificateUi === ui) {
            window.clearClientCertificateRequest(true)
            cancelled = true
        }
        if (window.pendingCertificateErrorUi === ui) {
            window.clearCertificateErrorRequest(true)
            cancelled = true
        }
        if (window.pendingWebAuthUi === ui) {
            window.clearWebAuthRequest(true)
            cancelled = true
        }
        if (window.pendingContextMenuUi === ui) {
            window.clearContextMenuRequest()
            cancelled = true
        }
        if (window.pendingDesktopMediaHost === hostWindow) {
            window.clearDesktopMediaRequest(true)
            cancelled = true
        }
        var sessions = (window.captureSessions || []).slice(0)
        for (var i = 0; i < sessions.length; ++i) {
            var session = sessions[i]
            if (session && session.ui === ui && session.host === hostWindow
                    && session.status === "active") {
                session.status = "stop-requested"
                if (session.view) {
                    session.view.reload()
                }
                cancelled = true
            }
        }
        window.captureSessions = sessions
        if (cancelled) {
            ui.status_text = "Browser requests cancelled for shutdown"
        }
        return cancelled
    }

    function tabSuspensionBlockReason(index, id, view) {
        if (index < 0 || !id || id.length === 0) {
            return "tab target is stale"
        }
        if (index === browserUi.active_tab_index) {
            return "only hidden tabs may be suspended"
        }
        if (!view) {
            return "tab view is not ready"
        }
        if (view.devToolsView) {
            return "DevTools is attached"
        }
        if (view.recentlyAudible) {
            return "audio is active or was recently audible"
        }
        if (view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return "engine view is not active"
        }
        if (view.recommendedState !== WebEngineView.LifecycleState.Frozen) {
            return "the engine has not approved freezing this tab"
        }
        if (window.hasActiveDownloads() || window.pendingDownloadId.length > 0) {
            return "a download is active"
        }
        if (window.hasActiveShutdownRequestsFor(browserUi, window)) {
            return "a browser request or capture is active"
        }
        if (browserUi.tab_has_active_operations(id)) {
            return "a browser operation is active"
        }
        return ""
    }

    function requestTabSuspension(id) {
        var index = browserUi.tab_index_for_id(id)
        var view = tabViewAt(index)
        var reason = window.tabSuspensionBlockReason(index, id, view)
        if (reason.length > 0) {
            browserUi.status_text = "Tab suspension refused: " + reason
            return
        }
        var probe = BrowserScripts.formStateProbe()
        view.runJavaScript(probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab suspension refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_suspend(id)) {
                return
            }
            window.executePendingEngineAction()
        })
    }

    function tabDiscardBlockReason(index, id, view) {
        if (index < 0 || !id || id.length === 0) {
            return "tab target is stale"
        }
        if (index === browserUi.active_tab_index) {
            return "only hidden tabs may be discarded"
        }
        if (!view) {
            return "tab view is not ready"
        }
        if (view.devToolsView) {
            return "DevTools is attached"
        }
        if (view.recentlyAudible) {
            return "audio is active or was recently audible"
        }
        if (view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return "engine view is not active"
        }
        if (view.recommendedState !== WebEngineView.LifecycleState.Discarded) {
            return "the engine has not approved discarding this tab"
        }
        if (window.hasActiveDownloads() || window.pendingDownloadId.length > 0) {
            return "a download is active"
        }
        if (window.hasActiveShutdownRequestsFor(browserUi, window)) {
            return "a browser request or capture is active"
        }
        if (browserUi.tab_has_active_operations(id)) {
            return "a browser operation is active"
        }
        return ""
    }

    function requestTabDiscard(id) {
        var index = browserUi.tab_index_for_id(id)
        var view = tabViewAt(index)
        var reason = window.tabDiscardBlockReason(index, id, view)
        if (reason.length > 0) {
            browserUi.status_text = "Tab discard refused: " + reason
            return
        }
        var probe = BrowserScripts.formStateProbe()
        view.runJavaScript(probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab discard refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_discard(id)) {
                return
            }
            window.executePendingEngineAction()
        })
    }

    function shutdownPageProbeScript() {
        return BrowserScripts.shutdownPageProbe()
    }

    function showShutdownPagePrompt(reason) {
        shutdownPageProbeTimer.stop()
        window.shutdownPagePromptSnapshot = ShutdownPageProbeState.snapshotActive(
            tabViews, WebEngineView.LifecycleState.Active, true)
        window.shutdownPagePromptReason = reason
        window.shutdownPagePromptVisible = true
    }

    function confirmShutdownPagePrompt() {
        window.shutdownPagePromptVisible = false
        if (!ShutdownPageProbeState.stillCurrent(
                window.shutdownPagePromptSnapshot, tabViews,
                WebEngineView.LifecycleState.Active, true)) {
            window.shutdownPagePromptSnapshot = null
            window.checkPageStateBeforeQuit()
            return
        }
        window.shutdownPagePromptSnapshot = null
        window.finalizeQuit()
    }

    function checkPageStateBeforeQuit() {
        var generation = ++window.shutdownPageProbeGeneration
        window.shutdownPagePromptSnapshot = null
        shutdownPageProbeTimer.restart()
        var candidates = ShutdownPageProbeState.snapshotActive(
            tabViews, WebEngineView.LifecycleState.Active)
        if (!candidates) {
            window.showShutdownPagePrompt(
                "A page or browser activity changed during the close check.")
            return
        }
        var probe = function(position) {
            if (generation !== window.shutdownPageProbeGeneration) {
                return
            }
            if (!ShutdownPageProbeState.stillCurrent(
                    candidates, tabViews, WebEngineView.LifecycleState.Active)) {
                window.showShutdownPagePrompt(
                    "A page or browser activity changed during the close check.")
                return
            }
            if (position >= candidates.length) {
                shutdownPageProbeTimer.stop()
                window.finalizeQuit()
                return
            }
            candidates[position].view.runJavaScript(
                    window.shutdownPageProbeScript(), function(result) {
                if (generation !== window.shutdownPageProbeGeneration) {
                    return
                }
                if (!ShutdownPageProbeState.stillCurrent(
                        candidates, tabViews, WebEngineView.LifecycleState.Active)) {
                    window.showShutdownPagePrompt(
                        "A page or browser activity changed during the close check.")
                    return
                }
                if (result === "dirty") {
                    window.showShutdownPagePrompt(
                        "A page has unsaved form or editor state.")
                    return
                }
                if (result !== "clean") {
                    window.showShutdownPagePrompt(
                        "Ferric Browser could not verify every page before closing.")
                    return
                }
                probe(position + 1)
            })
        }
        probe(0)
    }

    function beginQuitRequest(commandText) {
        if (window.shutdownConfirmationVisible
                || window.shutdownPromptVisible
                || window.shutdownPagePromptVisible
                || window.applicationShutdownInProgress) {
            return
        }
        window.shutdownConfirmationCommand = commandText === ":quit"
                ? ":quit" : ":window-close"
        window.shutdownConfirmationVisible = true
        browserUi.status_text = window.shutdownConfirmationCommand === ":quit"
                ? "Confirm browser shutdown" : "Confirm window close"
    }

    function continueQuitRequest() {
        window.shutdownConfirmationVisible = false
        if (window.hasActiveDownloads()
                || window.hasActiveShutdownRequestsFor(browserUi, window)) {
            window.shutdownPromptVisible = true
            return
        }
        if (browserUi.has_pending_versioning_object_work()) {
            window.shutdownWaitingForDownloadFinishes = true
            window.shutdownPromptVisible = true
            window.scheduleRuntimeWork(50)
            return
        }
        window.shutdownWaitingForDownloadFinishes = false
        window.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        shutdownPageProbeTimer.stop()
        window.shutdownPagePromptVisible = false
        window.shutdownPagePromptSnapshot = null
        if (window.hasActiveDownloads()
                || window.hasActiveShutdownRequestsFor(browserUi, window)) {
            window.shutdownPromptVisible = true
            return
        }
        if (browserUi.has_pending_versioning_object_work()) {
            window.shutdownWaitingForDownloadFinishes = true
            window.shutdownPromptVisible = true
            window.scheduleRuntimeWork(50)
            return
        }
        window.shutdownPromptVisible = false
        window.beginApplicationShutdown()
    }

    function cancelDownloadsAndQuit() {
        window.shutdownWaitingForDownloadFinishes = true
        if (window.nativePageCaptureSave
                || (browserUi
                    && typeof browserUi.has_native_page_capture_request === "function"
                    && browserUi.has_native_page_capture_request())) {
            window.cancelNativePageCaptureForShutdown()
        }
        if (window.pendingDownloadId.length > 0) {
            window.cancelPendingDownload()
        }
        for (var pendingKey in window.pendingDownloadRequests) {
            var pending = window.pendingDownloadRequests[pendingKey]
            if (pending) {
                window.resolveQtRequest(browserUi, pending, "download", "cancel", [])
                browserUi.update_download(pendingKey, "cancelled", pending.receivedBytes, true)
            }
        }
        for (var activeKey in window.activeDownloads) {
            var download = window.activeDownloads[activeKey]
            if (download && !download.isFinished) {
                if (window.resolveQtRequest(browserUi, download, "download", "cancel", [])) {
                    var state = window.downloadStateName(download)
                    if (!download.isFinished)
                        state = "cancelled"
                    browserUi.update_download(activeKey, state,
                                              download.receivedBytes, true)
                }
            }
        }
        window.cancelShutdownRequestsFor(browserUi, window)
        if (window.hasActiveDownloads()) {
            window.shutdownPromptVisible = true
        } else {
            window.shutdownWaitingForDownloadFinishes = false
            window.continueQuitRequest()
        }
    }

    function refreshLibraryManager() {
        libraryEntries.clear()
        libraryGraphEntries.clear()
        libraryGraphNodes.clear()
        window.libraryGraphLineData = []
        var kind = browserUi.library_kind
        var values = browserUi.library_values
        if (values.length === 0) {
            window.libraryTotalEntries = 0
            window.libraryPage = 0
            return
        }
        var lines = values.split("\n")
        window.libraryTotalEntries = lines.length
        var visibleLines = lines
        if (kind !== "journey") {
            var pageCount = Math.max(1, Math.ceil(lines.length / window.libraryPageSize))
            window.libraryPage = Math.max(0, Math.min(window.libraryPage, pageCount - 1))
            var pageStart = window.libraryPage * window.libraryPageSize
            visibleLines = lines.slice(pageStart, pageStart + window.libraryPageSize)
        }
        for (var i = 0; i < visibleLines.length; ++i) {
            var fields = visibleLines[i].split("\t")
            if ((kind === "history" || kind === "bookmarks") && fields.length >= 3) {
                libraryEntries.append({
                    label: fields[1],
                    secondary: fields[2],
                    entryId: fields[0],
                    entryKind: kind === "history" ? "history" : "bookmark"
                })
            } else if (kind === "journey" && fields.length >= 4) {
                var journeyLabel = fields[1].length > 0 ? fields[1] : fields[2]
                libraryEntries.append({
                    label: journeyLabel + " · " + fields[3],
                    secondary: fields[2] + (fields.length >= 5 && fields[4].length > 0
                                           ? " · " + fields[4] : ""),
                    nodeId: fields[0]
                })
            } else if (kind === "quickmarks" && fields.length >= 2) {
                libraryEntries.append({
                    label: fields[0],
                    secondary: fields[1],
                    entryId: fields[0],
                    entryKind: "quickmark"
                })
            } else if (kind === "actions" && fields.length >= 3) {
                libraryEntries.append({
                    label: fields[0] + " · " + fields[1],
                    secondary: fields[2] + (fields.length >= 4 && fields[3].length > 0
                                           ? " · Example: " + fields[3] : "")
                })
            }
        }
        if (kind === "journey") {
            var edgeSources = browserUi.library_graph_edge_sources
            var edgeTargets = browserUi.library_graph_edge_targets
            var edgeTransitions = browserUi.library_graph_edge_transitions
            if (edgeSources.length !== edgeTargets.length
                    || edgeSources.length !== edgeTransitions.length) {
                libraryGraphEntries.append({
                    label: "Relationship graph unavailable",
                    secondary: "The graph data was incomplete"
                })
            } else {
                var nodeLabels = ({})
                for (var n = 0; n < lines.length; ++n) {
                    var nodeFields = lines[n].split("\t")
                    if (nodeFields.length >= 4) {
                        nodeLabels[nodeFields[0]] = nodeFields[1].length > 0
                                ? nodeFields[1] : nodeFields[2]
                    }
                }
                var nodeIndex = ({})
                var graphNodeLimit = Math.min(lines.length, 120)
                var graphNodeWidth = 170
                var graphNodeHeight = 48
                var graphNodeGapX = 24
                var graphNodeGapY = 18
                var graphColumns = 4
                for (var g = 0; g < graphNodeLimit; ++g) {
                    var graphFields = lines[g].split("\t")
                    if (graphFields.length < 4 || nodeIndex[graphFields[0]] !== undefined) {
                        continue
                    }
                    var graphIndex = libraryGraphNodes.count
                    var graphX = (graphIndex % graphColumns)
                            * (graphNodeWidth + graphNodeGapX)
                    var graphY = Math.floor(graphIndex / graphColumns)
                            * (graphNodeHeight + graphNodeGapY)
                    nodeIndex[graphFields[0]] = graphIndex
                    libraryGraphNodes.append({
                        nodeId: graphFields[0],
                        label: graphFields[1].length > 0 ? graphFields[1] : graphFields[2],
                        transition: graphFields[3],
                        source: graphFields.length >= 5 ? graphFields[4] : "",
                        x: graphX,
                        y: graphY
                    })
                }
                for (var e = 0; e < edgeSources.length; ++e) {
                    var sourceId = String(edgeSources[e])
                    var targetId = String(edgeTargets[e])
                    var transition = String(edgeTransitions[e] || "navigate")
                    var source = nodeLabels[sourceId] || sourceId
                    var target = nodeLabels[targetId] || targetId
                    libraryGraphEntries.append({
                        label: source + " → " + target,
                        secondary: transition
                    })
                    if (nodeIndex[sourceId] !== undefined
                            && nodeIndex[targetId] !== undefined) {
                        var sourceNode = libraryGraphNodes.get(nodeIndex[sourceId])
                        var targetNode = libraryGraphNodes.get(nodeIndex[targetId])
                        window.libraryGraphLineData.push({
                            x1: sourceNode.x + graphNodeWidth / 2,
                            y1: sourceNode.y + graphNodeHeight / 2,
                            x2: targetNode.x + graphNodeWidth / 2,
                            y2: targetNode.y + graphNodeHeight / 2,
                            transition: transition
                        })
                    }
                }
                if (lines.length > graphNodeLimit) {
                    libraryGraphEntries.insert(0, {
                        label: "Graph layout capped at " + graphNodeLimit + " nodes",
                        secondary: "Use Search or Expand for a smaller neighborhood"
                    })
                }
                if (libraryGraphEntries.count === 0) {
                    libraryGraphEntries.append({
                        label: "No retained relationships",
                        secondary: "The current node has no visible edge in this bounded view"
                    })
                }
            }
        }
        libraryManager.requestGraphPaint()
    }

    function libraryPageCount() {
        return Math.max(1, Math.ceil(window.libraryTotalEntries / window.libraryPageSize))
    }

    function changeLibraryPage(delta) {
        var next = window.libraryPage + delta
        if (next < 0 || next >= window.libraryPageCount()) {
            return
        }
        window.libraryPage = next
        window.refreshLibraryManager()
    }

    function runJourneyQuery(argument) {
        var command = ":journey"
        if (argument.length > 0) {
            command += " " + argument
        }
        if (browserUi.execute_command(command)) {
            window.syncTabModel()
            window.refreshLibraryManager()
        }
    }

    function runJourneyCurrentQuery() {
        window.libraryJourneyCurrentOnly = true
        window.libraryJourneySearchText = ""
        window.libraryJourneyExpandedNode = ""
        window.runJourneyQuery("--current")
    }

    function runJourneyAllQuery() {
        window.libraryJourneyCurrentOnly = false
        window.libraryJourneySearchText = ""
        window.libraryJourneyExpandedNode = ""
        window.runJourneyQuery("")
    }

    function libraryCommandArgument(value) {
        return '"' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"'
    }

    function openLibraryEntry(entryKind, entryId) {
        var command = entryKind === "history" ? "history-open"
                : entryKind === "bookmark" ? "bookmark-open" : "quickmark-open"
        if (browserUi.execute_command(":" + command + " " + libraryCommandArgument(entryId))) {
            window.syncTabModel()
            window.executePendingEngineAction()
            window.libraryManagerVisible = false
            window.closeInternalSurface()
        }
    }

    function showPrivateHistoryTransfer(entryId, title, url) {
        if (!window.temporaryProfile || String(url).length === 0) {
            browserUi.status_text = "Private history transfer is unavailable"
            return
        }
        window.privateHistoryTransferId = String(entryId)
        window.privateHistoryTransferTitle = String(title || "")
        window.privateHistoryTransferUrl = String(url)
        window.privateHistoryTransferVisible = true
        window.refreshProfiles()
        window.openInternalSurface()
    }

    function cancelPrivateHistoryTransfer() {
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.closeInternalSurface()
    }

    function confirmPrivateHistoryTransfer(profileName) {
        var command = ":profile-open " + window.libraryCommandArgument(profileName)
                + " " + window.libraryCommandArgument(window.privateHistoryTransferUrl)
        if (!browserUi.execute_command(command)) {
            return
        }
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.libraryManagerVisible = false
        window.closeInternalSurface()
        Qt.callLater(function() { window.executePendingEngineAction() })
    }

    function confirmPrivateHistoryBookmark(profileName, profileLabel) {
        var entries = window.browserWindowRegistry || []
        for (var index = 0; index < entries.length; ++index) {
            var entry = entries[index]
            if (entry && entry.ui && !entry.ephemeralProfile
                    && String(entry.profileName || "") === String(profileName)) {
                if (!entry.ui.queue_bookmark_transfer(
                            window.privateHistoryTransferUrl,
                            window.privateHistoryTransferTitle)) {
                    browserUi.status_text = "Destination profile is not ready for bookmark transfer"
                    return
                }
                window.activateBrowserWindow(entry.host, entry.ui)
                window.cancelPrivateHistoryTransfer()
                return
            }
        }
        window.browserWindowFactory.createObject(null, {
            windowStartupUrl: window.privateHistoryTransferUrl,
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: false,
            windowBookmarkTransferUrl: window.privateHistoryTransferUrl,
            windowBookmarkTransferTitle: window.privateHistoryTransferTitle
        })
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.libraryManagerVisible = false
        window.closeInternalSurface()
    }

    function deleteLibraryEntry(entryKind, entryId) {
        var command = entryKind === "bookmark" ? "bookmark-delete" : "quickmark-delete"
        var pendingKey = entryKind + "\t" + entryId
        if (window.pendingLibraryDelete !== pendingKey) {
            window.pendingLibraryDelete = pendingKey
            return
        }
        if (browserUi.execute_command(":" + command + " " + libraryCommandArgument(entryId))) {
            window.pendingLibraryDelete = ""
            window.refreshLibraryManager()
        }
    }

    function editLibraryEntry(entryKind, entryId, value) {
        var trimmed = String(value).trim()
        if (trimmed.length === 0) {
            browserUi.status_text = "Library edit requires a non-empty value"
            return false
        }
        var command = entryKind === "bookmark" ? "bookmark-edit" : "quickmark-edit"
        var argumentsText = entryKind === "bookmark"
                ? libraryCommandArgument(entryId) + " --title " + libraryCommandArgument(trimmed)
                : libraryCommandArgument(entryId) + " " + libraryCommandArgument(trimmed)
        if (browserUi.execute_command(":" + command + " " + argumentsText)) {
            window.pendingLibraryDelete = ""
            window.refreshLibraryManager()
            return true
        }
        return false
    }

    function beginJourneyExport() {
        var preview = browserUi.journey_export_preview()
        if (preview.length === 0) {
            window.journeyExportAwaiting = true
            return
        }
        window.journeyExportAwaiting = false
        window.journeyExportPreviewText = preview
        window.journeyExportPreviewVisible = true
    }

    function finishJourneyExport() {
        var selected = fileDialogSurfaces.journeyExportSelectedFile
        var path = selected && selected.scheme === "file" ? selected.toLocalFile() : ""
        if (path.length === 0 || !browserUi.export_journey(path)) {
            window.journeyExportPreviewVisible = true
            return
        }
        window.journeyExportPreviewVisible = false
    }

    function finishDiagnosticsExport() {
        var selected = fileDialogSurfaces.diagnosticsExportSelectedFile
        var path = selected && selected.scheme === "file" ? selected.toLocalFile() : ""
        if (!path || !browserUi.export_diagnostics(path)) {
            window.diagnosticsVisible = true
        }
    }

    function journeySearchArgument(value) {
        return value.length > 0 ? "--search " + JSON.stringify(value) : ""
    }

    function journeyExpandArgument(nodeId) {
        return "--expand " + JSON.stringify(nodeId)
    }

    function cancelSwitcherBatch() {
        switcherBatchGeneration += 1
        switcherBatchTimer.stop()
        switcherBatchEntries = []
        switcherBatchMerged = []
        switcherBatchIndex = 0
    }

    function switcherMaxResults() {
        return browserUi.feature_switcher_max_results
    }

    function processSwitcherBatch() {
        if (switcherBatchScheduledGeneration !== switcherBatchGeneration
                || !window.switcherVisible
                || switcherBatchQuery !== switcherSurface.query
                || switcherBatchScope !== window.switcherScope) {
            window.cancelSwitcherBatch()
            return
        }
        if (switcherBatchIndex >= switcherBatchEntries.length) {
            switcherResults = switcherBatchMerged
            return
        }
        var entry = switcherBatchEntries[switcherBatchIndex]
        switcherBatchIndex += 1
        if (entry && entry.ui) {
            if (entry.ui.switcher_query(switcherBatchQuery, switcherBatchScope)) {
                var kinds = entry.ui.switcher_result_kinds || []
                var ids = entry.ui.switcher_result_ids || []
                var generations = entry.ui.switcher_result_generations || []
                var labels = entry.ui.switcher_result_labels || []
                var secondaries = entry.ui.switcher_result_secondaries || []
                var profiles = entry.ui.switcher_result_profiles || []
                var workspaces = entry.ui.switcher_result_workspaces || []
                var actions = entry.ui.switcher_result_actions || []
                var ranks = entry.ui.switcher_result_ranks || []
                var recencies = entry.ui.switcher_result_recencies || []
                var count = kinds.length
                if (ids.length === count && generations.length === count
                        && labels.length === count && secondaries.length === count
                        && profiles.length === count && workspaces.length === count
                        && actions.length === count && ranks.length === count
                        && recencies.length === count) {
                    for (var j = 0; j < count; ++j) {
                        var actionText = String(actions[j])
                        switcherBatchMerged.push({
                            kind: String(kinds[j]),
                            id: String(ids[j]),
                            generation: String(generations[j]),
                            label: String(labels[j]),
                            secondary: String(secondaries[j]),
                            profile: String(profiles[j]),
                            workspace: String(workspaces[j]),
                            actions: actionText.length > 0 ? actionText.split("\t") : [],
                            rank: Number(ranks[j]),
                            recency: Number(recencies[j]),
                            owner_token: String(entry.ui.window_token)
                        })
                    }
                }
            }
        }
        // Publish each source as it arrives. The next source is scheduled on
        // the GUI event loop, so a large multi-window search cannot monopolize
        // keyboard, focus, or portal callbacks.
        switcherBatchMerged.sort(function(left, right) {
            return Number(right.rank || 0) - Number(left.rank || 0)
                    || Number(right.recency || 0) - Number(left.recency || 0)
                    || String(left.kind || "").localeCompare(String(right.kind || ""))
                    || String(left.id || "").localeCompare(String(right.id || ""))
        })
        switcherResults = switcherBatchMerged.slice(0)
        switcherBatchTimer.start()
    }

    function refreshSwitcher() {
        window.cancelSwitcherBatch()
        var entries = window.browserWindowRegistry || []
        if (entries.length === 0) {
            entries = [{ host: window, ui: browserUi }]
        }
        switcherBatchEntries = entries.slice(0)
        switcherBatchMerged = []
        switcherBatchIndex = 0
        switcherBatchQuery = switcherSurface.query
        switcherBatchScope = window.switcherScope
        switcherBatchScheduledGeneration = switcherBatchGeneration
        switcherResults = []
        switcherBatchTimer.start()
    }

    // Querying every key event synchronously across all browser windows can
    // build a backlog while the user is typing. Coalesce edits into one
    // latest-input refresh; activation and scope changes still refresh
    // immediately at their explicit boundaries.
    Timer {
        id: switcherRefreshTimer
        interval: 35
        repeat: false
        onTriggered: {
            if (window.switcherVisible) {
                window.refreshSwitcher()
            }
        }
    }

    Timer {
        id: switcherBatchTimer
        interval: 0
        repeat: false
        onTriggered: window.processSwitcherBatch()
    }

    function showSwitcherWith(scope, query) {
        window.captureOverlayFocus(window, window.activeWebView())
        switcherVisible = true
        switcherScope = scope && scope.length > 0 ? scope : "all"
        switcherRefreshTimer.stop()
        switcherSurface.query = query || ""
        refreshSwitcher()
        switcherSurface.focusInput()
    }

    function showSwitcher() {
        window.showSwitcherWith("all", "")
    }

    function closeSwitcher() {
        var wasVisible = switcherVisible
        switcherRefreshTimer.stop()
        window.cancelSwitcherBatch()
        switcherVisible = false
        switcherResults = []
        if (wasVisible) {
            window.restoreOverlayFocus()
        }
    }

    function setSwitcherScope(scope) {
        if (!scope || scope.length === 0) {
            return
        }
        switcherScope = scope
        switcherRefreshTimer.stop()
        window.cancelSwitcherBatch()
        refreshSwitcher()
        switcherSurface.focusInput()
    }

    function showLinkPreview() {
        if (!linkPreviewVisible && browserUi.link_preview_visible) {
            window.openInternalSurface()
        }
        linkPreviewVisible = browserUi.link_preview_visible
    }

    function closeLinkPreview() {
        var wasVisible = linkPreviewVisible
        linkPreviewVisible = false
        browserUi.clear_link_preview()
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function hintCollectorScript(family) {
        return BrowserScripts.hintCollector(family)
    }

    function hintDirtyRevisionScript() {
        return BrowserScripts.hintDirtyRevision()
    }

    function hintStopTrackingScript() {
        return BrowserScripts.hintStopTracking()
    }

    function selectionScript() {
        return BrowserScripts.selection()
    }

    function downloadLinkScript(url) {
        return BrowserScripts.downloadLink(url)
    }

    function editorScript() {
        return BrowserScripts.editor()
    }

    function editorApplyScript(original, updated) {
        return BrowserScripts.editorApply(original, updated)
    }

    function caretScript(operation, selecting) {
        return BrowserScripts.caret(operation, selecting)
    }

    function hintFreshScript(candidate) {
        return BrowserScripts.hintFresh(candidate)
    }

    function hintFocusScript(elementId) {
        return BrowserScripts.hintFocus(elementId)
    }

    function hintClickScript(elementId) {
        return BrowserScripts.hintClick(elementId)
    }

    function hintSetScrollTargetScript(elementId, framePath) {
        return BrowserScripts.hintSetScrollTarget(elementId, framePath)
    }

    function startHintCollection() {
        var view = window.activeWebView()
        if (!view) {
            browserUi.cancel_hints()
            return
        }
        if (window.hintRefreshPending) {
            return
        }
        if (window.hintTrackingView && window.hintTrackingView !== view) {
            window.closeHints()
            return
        }
        window.hintTrackingView = view
        var collectionEpoch = window.hintCollectionEpoch + 1
        window.hintCollectionEpoch = collectionEpoch
        window.hintRefreshPending = true
        window.runBrowserScript(view, window.hintCollectorScript(browserUi.hint_family), function(value) {
            if (window.hintCollectionEpoch !== collectionEpoch) {
                return
            }
            window.hintRefreshPending = false
            if (browserUi.mode !== "hint" || window.hintTrackingView !== view) {
                return
            }
            window.hintDirtyRevision = Number(value && value.revision || 0)
            var collectedViewport = value && value.viewport ? value.viewport : ({})
            var collectedWidth = Number(collectedViewport.width || 0)
            var collectedHeight = Number(collectedViewport.height || 0)
            window.hintViewport = ({
                width: Number.isFinite(collectedWidth) && collectedWidth > 0
                       ? collectedWidth : 0,
                height: Number.isFinite(collectedHeight) && collectedHeight > 0
                        ? collectedHeight : 0
            })
            var payload = value && value.candidates !== undefined
                    ? {candidates: value.candidates}
                    : {candidates: value || []}
            var response = {}
            try {
                response = JSON.parse(browserUi.begin_hint_session(JSON.stringify(payload)))
            } catch (error) {
                response = {}
            }
            if (!response.hints || response.hints.length === 0) {
                var hintFailure = response.error
                        ? browserUi.status_text : "No visible hint targets"
                window.hintResults = []
                browserUi.cancel_hints()
                browserUi.status_text = hintFailure
                window.showCommandNotice(hintFailure, true)
                return
            }
            window.hintState = response.state || window.hintState
            window.hintInput = window.hintState.mode === "text"
                    ? "/" + window.hintState.query : window.hintState.prefix
            window.hintResults = response.hints
            if (response.first_label) {
                Qt.callLater(function() { window.activateHint(String(response.first_label)) })
            }
        })
    }

    function pollHintRefresh() {
        var view = window.activeWebView()
        if (!view || !browserUi.hint_visible || window.hintRefreshPending) {
            return
        }
        window.runBrowserScript(view, window.hintDirtyRevisionScript(), function(value) {
            if (value && value.dirty === true
                    && Number(value.revision || 0) !== window.hintDirtyRevision) {
                window.startHintCollection()
            }
        })
    }

    function applyHintInteraction(action, text) {
        var response = {}
        try {
            response = JSON.parse(browserUi.update_hint_interaction(action, text || ""))
        } catch (error) {
            return false
        }
        if (response.state) {
            window.hintState = response.state
            window.hintInput = response.state.mode === "text"
                    ? "/" + response.state.query : response.state.prefix
        }
        if (response.outcome === "activate") {
            window.activateHint(String(response.label || ""))
        } else if (response.outcome === "cancel") {
            window.closeHints()
        } else if (response.outcome === "rotate") {
            window.rotateHintCollision()
        }
        return true
    }

    function rotateHintCollision() {
        var active = String(window.hintState.active_label || "")
        var selected = null
        for (var i = 0; i < window.hintResults.length; ++i) {
            if (String(window.hintResults[i].label) === active) {
                selected = window.hintResults[i]
                break
            }
        }
        if (!selected) return
        var cluster = window.hintResults.filter(function(candidate) {
            return candidate.x < selected.x + selected.width
                    && candidate.x + candidate.width > selected.x
                    && candidate.y < selected.y + selected.height
                    && candidate.y + candidate.height > selected.y
        })
        if (cluster.length < 2) return
        var current = cluster.indexOf(selected)
        var next = cluster[(current + 1) % cluster.length]
        window.hintCollisionRotation += 1
        window.applyHintInteraction("select", String(next.label))
    }

    function closeHints() {
        window.hintCollectionEpoch += 1
        window.stopHintTracking()
        window.hintRefreshPending = false
        window.hintInput = ""
        window.hintViewport = ({ width: 0, height: 0 })
        window.hintState = ({ mode: "label", prefix: "", query: "", matching_labels: [], active_label: "", remaining: 0, total: 0 })
        window.hintResults = []
        window.rapidHintConfirmationVisible = false
        browserUi.cancel_hints()
    }

    function stopHintTracking() {
        var view = window.hintTrackingView
        window.hintTrackingView = null
        if (view) {
            window.runBrowserScript(view, window.hintStopTrackingScript(), function(value) {})
        }
    }

    function openEphemeralWindow(url, requestedToken) {
        var marker = Date.now().toString(36) + "-" + window.browserWindowRegistry.length
        var token = String(requestedToken || "")
        var sharedProfile = window.ephemeralProfileForToken(token)
        if (token.length === 0) {
            token = "ephemeral-" + marker
        }
        var child = window.browserWindowFactory.createObject(null, {
            windowStartupUrl: url,
            windowProfileName: sharedProfile ? "ephemeral-shared" : "ephemeral-" + marker,
            windowProfileLabel: "Ephemeral · " + marker,
            windowPrivateProfile: false,
            windowEphemeralProfile: true,
            windowEphemeralInvocationToken: token,
            windowSharedProfile: sharedProfile,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            browserUi.status_text = "Could not create ephemeral window"
            return false
        }
        browserUi.status_text = "Opened link in a fresh ephemeral window"
        return true
    }

    function openReopenedWindow(ui, hostWindow, action) {
        var fields = String(action || "").split("\t")
        if (fields.length < 2 || fields.slice(1).join("\t").length === 0) {
            if (ui) {
                ui.status_text = "Reopen-in-window payload was invalid"
            }
            return false
        }
        var url = fields.slice(1).join("\t")
        var secondary = hostWindow !== window
        var profileName = secondary ? hostWindow.windowProfileName : window.profileName
        var profileLabel = secondary ? hostWindow.windowProfileLabel : window.profileLabel
        var privateProfile = secondary
                ? hostWindow.windowPrivateProfile
                : (window.temporaryProfile && !window.ephemeralProfile)
        var ephemeralProfile = secondary
                ? hostWindow.windowEphemeralProfile : window.ephemeralProfile
        var sharedProfile = secondary
                ? hostWindow.windowWebEngineProfile : browserProfile
        var invocationToken = secondary
                ? hostWindow.windowEphemeralInvocationToken
                : window.ephemeralInvocationToken
        var child = window.browserWindowFactory.createObject(null, {
            windowStartupUrl: url,
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: privateProfile,
            windowEphemeralProfile: ephemeralProfile,
            windowEphemeralInvocationToken: invocationToken,
            windowSharedProfile: (privateProfile || ephemeralProfile) ? sharedProfile : null,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            if (ui) {
                ui.status_text = "Could not create reopen-in-window target"
            }
            return false
        }
        if (ui) {
            ui.status_text = "Opened safe URL in a new same-profile window; live state was lost"
        }
        return true
    }

    function clearVersionedRestoreOptions(hostWindow) {
        if (!hostWindow) {
            return
        }
        hostWindow.versioningOpenRestoreCommitId = ""
        hostWindow.versioningOpenRestoreOptionsAt = 0
        hostWindow.versioningOpenRestoreMediaPositionsRequested = false
        hostWindow.versioningOpenRestoreEditableDraftsRequested = false
    }

    property var pendingCleanBranchWindows: ({})

    function cleanBranchPageOutcomeIndex(view, ui, index) {
        if (!window.windowCleanBranchLineagePublished || ui !== browserUi
                || !view || view.viewUi !== ui || view.profile !== browserProfile
                || index < 0 || index >= ui.tab_count
                || String(view.stableTabId) !== String(ui.tab_id_for_index(index)))
            return -1
        return window.windowBranchPageOutcomeIds.indexOf(String(view.stableTabId))
    }

    function startCleanBranchPageNavigation(view, ui, index, url) {
        if (window.cleanBranchPageOutcomeIndex(view, ui, index) < 0)
            return false
        var next = CleanBranchPreparation.startWindowPageNavigation(
                    window.windowBranchPageOutcomeIds,
                    window.windowBranchPageExpectedUrls,
                    window.windowBranchPageNavigationStarted,
                    window.windowBranchPageOutcomes,
                    String(view.stableTabId), String(url || ""))
        if (!next)
            return false
        window.windowBranchPageNavigationStarted = next
        var originalIndex = window.windowBranchPageOutcomeIds.indexOf(
                    String(view.stableTabId))
        var startedAt = window.windowBranchPageStartedAt.slice()
        if (!startedAt[originalIndex])
            startedAt[originalIndex] = Date.now()
        window.windowBranchPageStartedAt = startedAt
        return true
    }

    function recordCleanBranchPageOutcome(view, ui, index, outcome, url) {
        if (window.cleanBranchPageOutcomeIndex(view, ui, index) < 0)
            return false
        var terminal = CleanBranchPreparation.classifyWindowTerminalOutcome(
                    outcome, String(url || ""))
        if (!terminal)
            return false
        var next = CleanBranchPreparation.recordWindowPageOutcome(
                    window.windowBranchPageOutcomeIds,
                    window.windowBranchPageNavigationStarted,
                    window.windowBranchPageOutcomes,
                    String(view.stableTabId), terminal)
        if (!next)
            return false
        window.windowBranchPageOutcomes = next
        return true
    }

    function loadDeferredCleanBranchPage(index) {
        if (!window.windowCleanBranch || !window.windowCleanBranchPublished
                || browserUi.active_tab_index !== index)
            return false
        var next = CleanBranchPreparation.takeDeferredWindowUrl(
                    window.windowBranchDeferredNavigations, index)
        if (!next)
            return false
        window.windowBranchDeferredNavigations = next.deferred
        if (!browserUi.navigate_clean_branch_safe_get(next.url)) {
            browserUi.status_text = "Clean branch page " + (index + 1)
                    + " could not start its proved safe navigation"
            return false
        }
        window.executePendingEngineAction()
        window.updateActiveTabUrl()
        return true
    }

    Connections {
        target: browserUi
        function onActive_tab_indexChanged() {
            window.loadDeferredCleanBranchPage(browserUi.active_tab_index)
        }
    }

    Timer {
        interval: 1000
        repeat: true
        running: window.windowCleanBranch && window.windowCleanBranchPublished
        onTriggered: {
            var expired = CleanBranchPreparation.expiredWindowPageIndexes(
                        window.windowBranchPageOutcomeIds,
                        window.windowBranchPageNavigationStarted,
                        window.windowBranchPageOutcomes,
                        window.windowBranchPageStartedAt, Date.now(), 15000)
            for (var expiredIndex = 0; expiredIndex < expired.length; ++expiredIndex) {
                var original = expired[expiredIndex]
                var tabId = window.windowBranchPageOutcomeIds[original]
                for (var index = 0; index < window.tabViews.length; ++index) {
                    var view = window.tabViews[index]
                    if (view && String(view.stableTabId) === tabId
                            && window.recordCleanBranchPageOutcome(
                                view, browserUi, index, "failed",
                                window.windowBranchPageExpectedUrls[original])) {
                        if (browserUi.active_tab_index === index)
                            browserUi.status_text = "Clean branch page load timed out"
                        break
                    }
                }
            }
        }
    }

    // Resolve in the source application's typed lineage registry, then focus
    // only the exact native window still registered for that owner. No URL is
    // replayed and no profile or view is reparented by switching.
    function switchVersionedCleanBranch(parentUi, sourceUi, command) {
        if (!parentUi || !sourceUi)
            return false
        var target = sourceUi === parentUi
                ? parentUi.switch_versioned_clean_branch_parent(command)
                : parentUi.switch_versioned_clean_branch_child(sourceUi, command)
        return window.activateVersionedBranchTarget(parentUi, target)
    }

    function activateVersionedBranchTarget(parentUi, target) {
        return CleanBranchSwitch.activate(
                    parentUi, String(target || ""),
                    (window.browserWindowRegistry || []).concat(
                        window.versioningWindowOwner.discoveryEntries(64)),
                    function(token) { return window.cleanBranchOwner(token) },
                    function(childUi, branchId) {
                        return parentUi.validate_versioned_clean_branch_target(
                                    childUi, branchId)
                    },
                    function(host, ui) {
                        return window.activateBrowserWindow(host, ui)
                    })
    }

    Timer {
        id: cleanBranchPreparationTimer
        interval: 25
        repeat: true
        onTriggered: window.pollPreparedCleanBranches()
    }

    function openVersionedCleanBranch(ui, hostWindow, action) {
        var prefix = "versioning-clean-branch-ready\t"
        var encoded = String(action || "")
        if (!ui || !hostWindow || encoded.indexOf(prefix) !== 0
                || encoded.length > 1200000) {
            return false
        }
        var request
        try {
            request = JSON.parse(encoded.slice(prefix.length))
        } catch (error) {
            ui.status_text = "Clean branch request was invalid; source remains active"
            return false
        }
        if (request && request.scope === "window")
            return window.openVersionedCleanBranchWindow(ui, hostWindow, request)
        if (encoded.length > 12 * 1024) {
            ui.status_text = "Clean branch request exceeded its bound"
            return false
        }
        var token = request && String(request.branch || "")
        var navigation = request && String(request.navigation || "")
        var url = request && String(request.url || "")
        var safeGet = navigation === "safe-get"
        if (!/^branch:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(token)
                || (navigation !== "placeholder" && !safeGet)
                || (safeGet && (url.length === 0 || url.length > 8192
                                || !/^https?:\/\//.test(url)
                                || /[\u0000-\u001f\u007f]/.test(url)))
                || (!safeGet && url.length > 0)
                || Object.keys(window.pendingCleanBranchWindows).length >= 32
                || window.pendingCleanBranchWindows[token]) {
            ui.status_text = "Clean branch request was invalid; source remains active"
            ui.abort_versioned_clean_branch(token)
            return false
        }
        var sourceTransient = hostWindow === window
                ? !!(window.temporaryProfile || window.ephemeralProfile)
                : !!hostWindow.windowTransientProfile
        var factory = window.browserWindowFactory
        if (sourceTransient || !factory || typeof factory.createObject !== "function") {
            ui.status_text = "Clean branch source or child factory unavailable"
            ui.abort_versioned_clean_branch(token)
            return false
        }
        var child = CleanBranchPreparation.createHiddenChild(factory, {
            rootWindow: window,
            windowBranchToken: token,
            windowBranchParentUi: ui,
            windowStartupUrl: "about:blank",
            windowProfileName: "branch-" + token.slice(7, 15),
            windowProfileLabel: "Clean branch"
        }, token, ui)
        if (!child) {
            ui.abort_versioned_clean_branch(token)
            ui.status_text = "Clean branch child preparation failed; source remains active"
            return false
        }
        var pending = window.pendingCleanBranchWindows
        pending[token] = {
            parentUi: ui,
            child: child,
            navigation: navigation,
            url: url,
            createdAt: Date.now()
        }
        window.pendingCleanBranchWindows = pending
        cleanBranchPreparationTimer.start()
        return true
    }

    function openVersionedCleanBranchWindow(ui, hostWindow, request) {
        var token = String(request.branch || "")
        var pages = request.pages
        var selected = Number(request.selected_index)
        var valid = /^branch:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(token)
                && Array.isArray(pages) && pages.length > 0 && pages.length <= 128
                && Number.isInteger(selected) && selected >= 0 && selected < pages.length
                && Object.keys(window.pendingCleanBranchWindows).length < 32
                && !window.pendingCleanBranchWindows[token]
                && !window.temporaryProfile && !window.ephemeralProfile
                && (hostWindow === window || !hostWindow.windowTransientProfile)
        if (valid) {
            for (var index = 0; index < pages.length; ++index) {
                var page = pages[index]
                var navigation = page && String(page.navigation || "")
                var url = page && String(page.url || "")
                if ((navigation !== "placeholder" && navigation !== "safe-get")
                        || (navigation === "placeholder" && url.length > 0)
                        || (navigation === "safe-get"
                            && (url.length === 0 || url.length > 8192
                                || !/^https?:\/\//.test(url)
                                || /[\u0000-\u001f\u007f]/.test(url)))) {
                    valid = false
                    break
                }
            }
        }
        if (!valid || !window.tabCapableWindowFactory) {
            ui.status_text = "Clean window branch request or host unavailable"
            ui.abort_versioned_clean_branch(token)
            return false
        }
        var factory = window.tabCapableWindowFactory
        var child = VersionedRestorePresentation.openRestoreWindow(
            factory, {
                startupUrl: "about:blank",
                startupAdditionalUrls: [],
                startupEntryPoint: "typed-initial-url",
                startupTrustedLocalInput: false,
                temporaryProfile: true,
                ephemeralProfile: true,
                instanceRequestOwner: false,
                safeMode: window.safeMode,
                userscriptsOff: window.userscriptsOff,
                softwareRendering: window.softwareRendering,
                nativeWayland: window.nativeWayland,
                instanceLockPath: window.instanceLockPath,
                instanceSelector: window.instanceSelector,
                profileName: "branch-" + token.slice(7, 15),
                profileLabel: "Clean branch",
                storageBasePath: window.storageBasePath,
                startupConfigJson: String(ui.config_json || window.startupConfigJson),
                startupConfigBaseJson: String(ui.config_base_json
                                              || window.startupConfigBaseJson),
                startupCliOverridesJson: String(ui.cli_overrides_json
                                                || window.startupCliOverridesJson),
                startupProfileOverridesJson: String(ui.profile_overrides_json
                                                    || window.startupProfileOverridesJson),
                startupConfigPath: String(ui.config_path || window.startupConfigPath),
                startupConfigSource: String(ui.config_source || window.startupConfigSource),
                startupContextsJson: String(ui.contexts_json || window.startupContextsJson),
                tabCapableWindowFactory: factory,
                versioningRestoreParentOwner: window.versioningWindowOwner,
                windowBranchToken: token,
                windowBranchParentUi: ui,
                windowBranchRootWindow: window
            }, ui, window.versioningWindowOwner)
        if (!child) {
            ui.abort_versioned_clean_branch(token)
            return false
        }
        var pending = window.pendingCleanBranchWindows
        pending[token] = {
            scope: "window", parentUi: ui, child: child,
            pages: pages, selectedIndex: selected, createdAt: Date.now()
        }
        window.pendingCleanBranchWindows = pending
        cleanBranchPreparationTimer.start()
        return true
    }

    function publishPreparedCleanBranchWindow(entry, token, owner) {
        var child = entry.child
        var childUi = child.windowBranchUi
        if (!owner || owner.host !== child
                || owner.profile !== child.windowBranchProfile
                || owner.view !== child.windowBranchInitialView
                || !childUi || childUi.tab_count !== 1) {
            return false
        }
        for (var index = 1; index < entry.pages.length; ++index) {
            if (childUi.new_tab() !== index)
                return false
        }
        child.syncTabModel()
        if (child.tabViews.length !== entry.pages.length
                || !childUi.select_tab(entry.selectedIndex))
            return false
        var seen = []
        for (var viewIndex = 0; viewIndex < entry.pages.length; ++viewIndex) {
            var view = child.tabViews[viewIndex]
            if (!view || seen.indexOf(view) >= 0
                    || view.profile !== owner.profile
                    || view.viewUi !== childUi
                    || view.viewInterceptor !== child.primaryRequestInterceptor
                    || String(view.stableTabId) !== String(childUi.tab_id_for_index(viewIndex)))
                return false
            seen.push(view)
        }
        child.windowBranchGroupViews = seen.slice(1)
        if (!entry.parentUi.publish_versioned_clean_branch_window(childUi, token))
            return false
        child.windowCleanBranchLineagePublished = true
        child.windowBranchPageOutcomeIds = seen.map(function(view) {
            return String(view.stableTabId)
        })
        child.windowBranchPageOutcomes = seen.map(function() { return "pending" })
        child.windowBranchPageExpectedUrls = entry.pages.map(function(page) {
            return page.navigation === "safe-get" ? String(page.url) : ""
        })
        child.windowBranchPageNavigationStarted = seen.map(function() { return false })
        child.windowBranchPageStartedAt = seen.map(function() { return 0 })
        var schedule = CleanBranchPreparation.windowNavigationSchedule(
                    entry.pages, entry.selectedIndex)
        if (!schedule)
            return false
        child.windowBranchDeferredNavigations = schedule.deferred
        if (schedule.selectedUrl.length > 0) {
            if (!childUi.select_tab(entry.selectedIndex)
                    || !childUi.navigate_clean_branch_safe_get(schedule.selectedUrl))
                return false
            child.executePendingEngineAction()
            child.updateActiveTabUrl()
        }
        if (!childUi.select_tab(entry.selectedIndex))
            return false
        child.syncTabModel()
        if (!child.registerBrowserWindow(
                child, childUi, child.windowBranchInitialView,
                child.browserProfile, child.profileName, token, true))
            return false
        child.windowCleanBranchPublished = true
        child.show()
        return true
    }

    function pollPreparedCleanBranches() {
        var pending = window.pendingCleanBranchWindows || ({})
        var remaining = ({})
        var tokens = Object.keys(pending)
        for (var i = 0; i < tokens.length; ++i) {
            var token = tokens[i]
            var entry = pending[token]
            var child = entry && entry.child
            var ui = entry && entry.parentUi
            if (!child || !ui || !entry || Date.now() - entry.createdAt > 10000) {
                if (child)
                    child.destroy()
                if (ui)
                    ui.abort_versioned_clean_branch(token)
                continue
            }
            if (!child.windowCleanBranchNativeBound) {
                remaining[token] = entry
                continue
            }
            var owner = window.cleanBranchOwner(token)
            if (entry.scope === "window") {
                if (!window.publishPreparedCleanBranchWindow(entry, token, owner)) {
                    ui.status_text = "Clean window branch publication failed; source remains active"
                    child.destroy()
                    ui.abort_versioned_clean_branch(token)
                } else {
                    ui.status_text = "Clean window branch opened (structural-only; local site data not copied)"
                }
                continue
            }
            var result = CleanBranchPreparation.publishPreparedChild(
                    entry, token, owner)
            if (result === "owner-failed") {
                ui.status_text = "Clean branch owner publication failed; source remains active"
                continue
            }
            if (result === "navigation-failed") {
                ui.status_text = "Clean branch navigation failed; source remains active"
                continue
            }
            if (result === "publication-failed") {
                ui.status_text = "Clean branch window publication failed; source remains active"
            } else {
                ui.status_text = "Clean branch opened (parent-active; child structural-only; local site data not copied)"
            }
        }
        window.pendingCleanBranchWindows = remaining
        if (Object.keys(remaining).length === 0)
            cleanBranchPreparationTimer.stop()
    }

    function openVersionedRestoreWindow(ui, hostWindow, action) {
        var fields = String(action || "").split("\t")
        if (fields.length !== 2 || fields[0] !== "versioning-restore-window"
                || fields[1].length === 0) {
            window.clearVersionedRestoreOptions(hostWindow)
            if (ui) {
                ui.status_text = "Versioned restore target was invalid"
            }
            return false
        }
        var secondary = hostWindow !== window
        var profileName = secondary ? hostWindow.windowProfileName : window.profileName
        var profileLabel = secondary ? hostWindow.windowProfileLabel : window.profileLabel
        var optionsAgeMs = hostWindow
                ? Date.now() - Number(hostWindow.versioningOpenRestoreOptionsAt) : -1
        var restoreOptionsMatch = !!hostWindow
                && String(hostWindow.versioningOpenRestoreCommitId || "") === fields[1]
                && Number.isFinite(optionsAgeMs) && optionsAgeMs >= 0
                && optionsAgeMs <= 60000
        var restoreMediaPositions = restoreOptionsMatch
                && !!hostWindow.versioningOpenRestoreMediaPositionsRequested
        var restoreEditableDrafts = restoreOptionsMatch
                && !!hostWindow.versioningOpenRestoreEditableDraftsRequested
        window.clearVersionedRestoreOptions(hostWindow)
        var sourceProfile = secondary ? hostWindow.windowWebEngineProfile : browserProfile
        // The owning primary surface alone receives profile-wide callbacks.
        // A different-profile secondary needs its own qualified owner adapter.
        if (!sourceProfile || sourceProfile !== browserProfile || !requestInterceptor) {
            if (ui)
                ui.status_text = "Same-profile restore owner unavailable; current work remains open"
            return false
        }
        var restoreFactory = window.tabCapableWindowFactory
        if (!restoreFactory || typeof restoreFactory.createObject !== "function") {
            if (ui) {
                ui.status_text = "Tab-capable restore window unavailable; current work remains open"
            }
            return false
        }

        var profileOverrides = secondary
                ? String(hostWindow.windowProfileOverridesJson || "{}")
                : String(ui.profile_overrides_json || window.startupProfileOverridesJson)
        var tabCapableChild = VersionedRestorePresentation.openRestoreWindow(
            restoreFactory, {
                startupUrl: "about:blank",
                startupAdditionalUrls: [],
                startupEntryPoint: "typed-initial-url",
                startupTrustedLocalInput: false,
                startupBackground: false,
                startupContext: "",
                startupCleanLink: false,
                temporaryProfile: false,
                ephemeralProfile: false,
                safeMode: window.safeMode,
                userscriptsOff: window.userscriptsOff,
                softwareRendering: window.softwareRendering,
                nativeWayland: window.nativeWayland,
                instanceLockPath: window.instanceLockPath,
                instanceSelector: window.instanceSelector,
                profileName: profileName,
                profileLabel: profileLabel,
                storageBasePath: window.storageBasePath,
                startupConfigJson: String(ui.config_json || window.startupConfigJson),
                startupConfigBaseJson: String(ui.config_base_json
                                              || window.startupConfigBaseJson),
                startupCliOverridesJson: String(ui.cli_overrides_json
                                                 || window.startupCliOverridesJson),
                startupProfileOverridesJson: profileOverrides,
                startupConfigPath: String(ui.config_path || window.startupConfigPath),
                startupConfigSource: String(ui.config_source || window.startupConfigSource),
                startupContextsJson: String(ui.contexts_json || window.startupContextsJson),
                tabCapableWindowFactory: restoreFactory,
                versioningRestoreStartupCommitId: fields[1],
                versioningRestoreSourceUi: ui,
                versioningRestoreParentOwner: window.versioningWindowOwner,
                versioningRestoreSharedProfile: sourceProfile,
                versioningRestoreSharedInterceptor: requestInterceptor,
                versioningRestoreStartupMediaPositions: restoreMediaPositions,
                versioningRestoreStartupEditableDrafts: restoreEditableDrafts
            }, ui, window.versioningWindowOwner)
        if (!tabCapableChild) {
            return false
        }
        return true
    }

    function transferPayloadForUi(sourceUi) {
        if (!sourceUi) {
            return ""
        }
        var index = Number(sourceUi.active_tab_index)
        var id = index >= 0 ? sourceUi.tab_id_for_index(index) : ""
        return id.length > 0 ? sourceUi.tab_transfer_payload(id) : ""
    }

    function detachSourceView(sourceUi, sourceHost) {
        if (sourceHost === window) {
            var index = sourceUi ? sourceUi.active_tab_index : -1
            var view = tabViewAt(index)
            return view ? window.detachTabViewAt(index) : null
        }
        return sourceHost && sourceHost.detachActiveViewForTransfer
                ? sourceHost.detachActiveViewForTransfer() : null
    }

    function restoreSourceView(sourceUi, sourceHost, view) {
        if (!view) {
            return false
        }
        if (sourceHost === window) {
            var index = sourceUi ? sourceUi.active_tab_index : -1
            if (index < 0 || index > tabViews.length) {
                index = tabViews.length
            }
            tabViews.splice(index, 0, view)
            view.parent = window.webViewsObject
            view.visible = true
            for (var restoredIndex = index; restoredIndex < tabViews.length; ++restoredIndex) {
                tabViews[restoredIndex].tabIndex = restoredIndex
                tabViews[restoredIndex].viewModel = tabs.get(restoredIndex)
            }
            return true
        }
        return sourceHost && sourceHost.restoreDetachedView
                ? sourceHost.restoreDetachedView(view) : false
    }

    function sameProfileWindow(sourceEntry, targetEntry) {
        return !!sourceEntry && !!targetEntry
                && sourceEntry.host !== targetEntry.host
                && sourceEntry.profileName === targetEntry.profileName
                && !!sourceEntry.privateProfile === !!targetEntry.privateProfile
                && !!sourceEntry.ephemeralProfile === !!targetEntry.ephemeralProfile
    }

    function attachTransferredView(view, payload, sourceUi, sourceHost) {
        if (!view || typeof payload !== "string" || payload.length === 0) {
            return false
        }
        var sourceUrl = view.url ? view.url.toString() : "about:blank"
        var transferData = null
        try {
            transferData = JSON.parse(payload)
        } catch (error) {
            transferData = null
        }
        if (!transferData || typeof transferData.tab_id !== "string"
                || !browserUi.adopt_tab_transfer(payload)) {
            return false
        }
        var adoptedTabId = browserUi.tab_id_for_index(browserUi.active_tab_index)
        window.syncTabModel()
        var index = browserUi.active_tab_index
        if (index < 0 || index >= tabs.count) {
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        tabs.setProperty(index, "url", transferData.url || "about:blank")
        tabs.setProperty(index, "title", transferData.title || "New tab")
        tabs.setProperty(index, "loaded", true)
        tabs.setProperty(index, "muted", !!transferData.muted)
        tabs.setProperty(index, "zoom", Number(transferData.zoom_hundredths || 100) / 100.0)
        var placeholder = tabViewAt(index)
        if (!placeholder) {
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        window.removeTabViewAt(index, null)
        view.viewUi = browserUi
        view.viewTabs = tabs
        view.viewModel = tabs.get(index)
        view.viewTransferred = false
        view.stableTabId = adoptedTabId
        if (view.viewHost !== undefined) {
            view.viewHost = window
        }
        if (view.viewIsSecondaryStatic === true) {
            view.url = sourceUrl
        }
        view.tabIndex = index
        view.anchors.fill = undefined
        view.parent = window.webViewsObject
        view.visible = true
        tabViews.splice(index, 0, view)
        for (var reindex = index; reindex < tabViews.length; ++reindex) {
            tabViews[reindex].tabIndex = reindex
            tabViews[reindex].viewModel = tabs.get(reindex)
        }
        window.updateBrowserWindowView(browserUi, view)
        if (!window.completeDetachedSource(sourceUi, sourceHost, payload)) {
            view.parent = null
            view.visible = false
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        if (arguments.length >= 5 && typeof arguments[4] === "string"
                && arguments[4].length > 0 && sourceUi) {
            sourceUi.complete_transfer_operation(arguments[4], true)
        }
        view.forceActiveFocus()
        browserUi.status_text = "Live tab attached without navigation"
        return true
    }

    function executeBrowserTransferAction(sourceUi, sourceHost, action) {
        var fields = String(action || "").split("\t")
        var kind = fields[0]
        var operationId = ""
        if (kind === "tab-detach" && fields.length >= 2
                && fields[1].indexOf("op-") === 0) {
            operationId = fields[1]
        } else if (kind === "tab-give" && fields.length >= 3
                && fields[2].indexOf("op-") === 0) {
            operationId = fields[2]
        } else if (kind === "tab-move-context" && fields.length >= 4
                && fields[3].indexOf("op-") === 0) {
            operationId = fields[3]
        }
        var sourceEntry = window.browserWindowEntryForUi(sourceUi)
        if (!sourceEntry || !sourceUi || !sourceHost) {
            if (sourceUi) {
                sourceUi.status_text = "Live tab source window is unavailable"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
            }
            return false
        }
        var payload = window.transferPayloadForUi(sourceUi)
        if (payload.length === 0) {
            sourceUi.status_text = "Active tab cannot be transferred"
            if (operationId.length > 0) {
                sourceUi.complete_transfer_operation(operationId, false)
            }
            return false
        }
        var targetEntry = kind === "tab-give"
                ? window.browserWindowEntryForTarget(fields.length >= 2 ? fields[1] : "")
                : (kind === "tab-move-context"
                        ? window.browserWindowEntryForContext(
                            fields.length >= 2 ? fields[1] : "", sourceEntry)
                        : null)
        if ((kind === "tab-give" || kind === "tab-move-context")
                && !window.sameProfileWindow(sourceEntry, targetEntry)) {
            sourceUi.status_text = !targetEntry
                    ? (kind === "tab-move-context"
                            ? "Context target window is unavailable; use context-enter to open it"
                            : "Tab give target window is unavailable")
                    : (kind === "tab-move-context"
                            ? "Context move requires a different same-profile window"
                    : "Tab give requires a different same-profile window")
            return false
        }
        var view = window.detachSourceView(sourceUi, sourceHost)
        if (!view) {
            sourceUi.status_text = "Live tab source view is unavailable"
            if (operationId.length > 0) {
                sourceUi.complete_transfer_operation(operationId, false)
            }
            return false
        }
        var transferAction = operationId.length > 0
                ? "tab-detach\t" + operationId + "\t" + payload
                : "tab-detach\t" + payload
        if (kind === "tab-give" || kind === "tab-move-context") {
            var attach = targetEntry.host === window
                    ? window.attachTransferredView : targetEntry.host.attachTransferredView
            if (!attach || !attach.call(targetEntry.host,
                        view, payload, sourceUi, sourceHost, operationId)) {
                window.restoreSourceView(sourceUi, sourceHost, view)
                sourceUi.status_text = "Tab give was rejected by the target window"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
                return false
            }
            return true
        }
        if (!window.openDetachedWindow(sourceUi, sourceHost, transferAction, view)) {
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        return true
    }

    function openDetachedWindow(sourceUi, hostWindow, action, view) {
        var fields = String(action || "").split("\t")
        if (fields.length < 2 || !view) {
            if (sourceUi) {
                sourceUi.status_text = "Live tab detach payload was invalid"
            }
            return false
        }
        var operationId = fields.length >= 3 && fields[1].indexOf("op-") === 0
                ? fields[1] : ""
        var payloadStart = operationId.length > 0 ? 2 : 1
        var payload = fields.slice(payloadStart).join("\t")
        var secondary = hostWindow !== window
        var profileName = secondary ? hostWindow.windowProfileName : window.profileName
        var profileLabel = secondary ? hostWindow.windowProfileLabel : window.profileLabel
        var privateProfile = secondary
                ? hostWindow.windowPrivateProfile
                : (window.temporaryProfile && !window.ephemeralProfile)
        var ephemeralProfile = secondary
                ? hostWindow.windowEphemeralProfile : window.ephemeralProfile
        var sharedProfile = secondary
                ? hostWindow.windowWebEngineProfile : browserProfile
        var sharedInterceptor = secondary
                ? hostWindow.windowRequestInterceptor : requestInterceptor
        var invocationToken = secondary
                ? hostWindow.windowEphemeralInvocationToken
                : window.ephemeralInvocationToken
        var child = window.browserWindowFactory.createObject(null, {
            windowStartupUrl: "about:blank",
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: privateProfile,
            windowEphemeralProfile: ephemeralProfile,
            windowEphemeralInvocationToken: invocationToken,
            windowSharedProfile: sharedProfile,
            windowSharedRequestInterceptor: sharedInterceptor,
            windowTransferView: view,
            windowTransferSourceUi: sourceUi,
            windowTransferSourceHost: hostWindow,
            windowTransferOperationId: operationId,
            windowTransferPayload: payload,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            if (sourceUi) {
                sourceUi.status_text = "Could not create detach target"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
            }
            return false
        }
        if (sourceUi) {
            sourceUi.status_text = "Live tab moved to same-profile window"
        }
        return true
    }

    function completeDetachedSource(sourceUi, sourceHost, payload) {
        var transfer = null
        try {
            transfer = JSON.parse(String(payload || ""))
        } catch (error) {
            transfer = null
        }
        if (!transfer || typeof transfer.tab_id !== "string") {
            return false
        }
        var index = sourceUi ? sourceUi.tab_index_for_id(transfer.tab_id) : -1
        var preparedSecondaryFallback = sourceHost && sourceHost.prepareTransferFallback
                ? sourceHost.prepareTransferFallback() : true
        if (!preparedSecondaryFallback) {
            return false
        }
        if (!sourceUi || index < 0 || !sourceUi.complete_tab_transfer(transfer.tab_id)) {
            if (sourceHost && sourceHost.discardPreparedTransferFallback) {
                sourceHost.discardPreparedTransferFallback()
            }
            return false
        }
        if (sourceHost === window && index < tabs.count) {
            tabs.remove(index)
            if (window.preparedTransferFallback) {
                tabs.append({
                    url: "about:blank",
                    title: "New tab",
                    loaded: true,
                    pinned: false,
                    muted: false,
                    zoom: 1.0,
                    suspended: false,
                    discarded: false
                })
                var fallback = window.preparedTransferFallback
                window.preparedTransferFallback = null
                fallback.viewUi = browserUi
                fallback.viewTabs = tabs
                fallback.viewModel = tabs.get(0)
                fallback.viewTransferred = false
                fallback.viewHost = window
                fallback.tabIndex = 0
                fallback.stableTabId = browserUi.tab_id_for_index(0)
                tabs.setProperty(0, "tabId", fallback.stableTabId)
                fallback.parent = window.webViewsObject
                fallback.visible = true
                tabViews.push(fallback)
                browserUi.status_text = "Live tab moved; blank fallback tab created"
            } else {
                window.syncTabModel()
            }
        } else if (sourceHost && sourceHost.resetAfterTransferSource) {
            if (!sourceHost.resetAfterTransferSource()) {
                return false
            }
        }
        return true
    }

    function showHintActions(label) {
        window.activateHint(label, "hint.choose")
    }

    function openHintActionPalette(label) {
        var view = window.activeWebView()
        if (!view) {
            window.closeHints()
            return
        }
        var selected = null
        for (var candidateIndex = 0; candidateIndex < window.hintResults.length; ++candidateIndex) {
            if (String(window.hintResults[candidateIndex].label) === String(label)) {
                selected = window.hintResults[candidateIndex]
                break
            }
        }
        if (!selected) return
        var items = [window.contextMenuItem("Activate / open current", "hint-userscript-action", label, "hint.current")]
        var hasUrl = selected.href !== null && String(selected.href || "").length > 0
        if (hasUrl) {
            items.push(window.contextMenuItem("Open in foreground tab", "hint-userscript-action", label, "hint.tab"))
            items.push(window.contextMenuItem("Open in background tab", "hint-userscript-action", label, "hint.tab-bg"))
            items.push(window.contextMenuItem("Open in new window", "hint-userscript-action", label, "hint.window"))
            items.push(window.contextMenuItem("Yank URL", "hint-userscript-action", label, "hint.yank"))
            items.push(window.contextMenuItem("Clean-yank URL", "hint-userscript-action", label, "hint.clean-yank"))
            items.push(window.contextMenuItem("Download", "hint-userscript-action", label, "hint.download"))
            items.push(window.contextMenuItem("Open in ephemeral window", "hint-userscript-action", label, "hint.ephemeral"))
        }
        if (hasUrl && browserUi.select_userscript_action_subject("link")) {
            var ids = browserUi.userscript_action_ids
            var labels = browserUi.userscript_action_labels
            var availability = browserUi.userscript_action_availability
            if (ids.length === labels.length && ids.length === availability.length) {
                for (var i = 0; i < ids.length; ++i) {
                    if (String(availability[i]) === "true" && String(ids[i]).length > 0
                            && String(labels[i]).length > 0) {
                        items.push(window.contextMenuItem(String(labels[i]),
                                                           "hint-userscript-action",
                                                           label, String(ids[i])))
                    }
                }
            }
        }
        window.pendingContextMenuRequest = { hintAction: true }
        window.pendingContextMenuView = view
        window.pendingContextMenuUi = browserUi
        window.pendingContextMenuHost = window
        window.contextMenuItems = items
        window.captureOverlayFocus(window, view)
        contextMenu.openAt(window, Math.round(window.width / 2),
                           Math.round(window.height / 2))
        browserUi.status_text = "Hint actions"
    }

    function activateHint(label, actionId) {
        var selected = null
        for (var i = 0; i < window.hintResults.length; ++i) {
            if (window.hintResults[i].label === label) {
                selected = window.hintResults[i]
                break
            }
        }
        var view = window.activeWebView()
        if (!selected || !view) {
            window.closeHints()
            return
        }
        window.runBrowserScript(view, window.hintFreshScript(selected), function(value) {
            var result = {}
            try {
                if (actionId && actionId.length > 0) {
                    result = JSON.parse(browserUi.select_hint_action(
                                label, JSON.stringify(value), actionId))
                } else {
                    result = JSON.parse(browserUi.select_hint(label, JSON.stringify(value)))
                }
            } catch (error) {
                result = {}
            }
            if (result.action === "navigate") {
                window.hintResults = []
                window.hintInput = ""
                window.executePendingEngineAction()
            } else if (result.action === "choose") {
                window.openHintActionPalette(label)
            } else if (result.action === "tab") {
                window.hintResults = []
                window.hintInput = ""
                // The engine has already inserted and selected the new tab.
                // Materialize its QML view before applying the queued load.
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (result.action === "window") {
                window.hintResults = []
                window.hintInput = ""
                window.executePendingEngineAction()
            } else if (result.action === "yank") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "clean-yank") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "tab-bg") {
                window.hintResults = []
                window.hintInput = ""
                window.syncTabModel()
                window.executePendingEngineAction()
                // Only rapid background hints remain in Hint mode. A one-shot
                // `;b` selection must return to Normal mode after opening.
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                }
            } else if (result.action === "userscript") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "download") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "ephemeral") {
                window.hintResults = []
                window.hintInput = ""
                window.openEphemeralWindow(
                            result.url, window.ephemeralTokenForUi(browserUi))
            } else if (result.error === "rapid-tab-limit") {
                window.hintResults = []
                window.hintInput = ""
                window.rapidHintConfirmationVisible = true
            } else if (result.action === "focus") {
                var focusScript = window.hintFocusScript(result.element_id)
                var focusStarted = window.runBrowserScript(view, focusScript, function(value) {
                    window.closeHints()
                    if (value !== true) {
                        browserUi.status_text = "Hint focus target is no longer available"
                        window.showCommandNotice(browserUi.status_text, true)
                        return
                    }
                    browserUi.enter_insert()
                })
                if (!focusStarted) {
                    window.closeHints()
                    browserUi.status_text = "Hint focus target is no longer available"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else if (result.action === "scroll-target") {
                var targetScript = window.hintSetScrollTargetScript(
                            result.element_id, result.frame_path)
                var targetStarted = window.runBrowserScript(view, targetScript, function(value) {
                    window.closeHints()
                    var targetSelected = value && typeof value === "object"
                            && value.ok === true && value.policy === "element"
                    browserUi.status_text = targetSelected
                            ? "Scroll target: selected region"
                            : "Scroll target selection failed: region is no longer available"
                    window.showCommandNotice(browserUi.status_text, !targetSelected)
                }, window.browserScriptWorld)
                if (!targetStarted) {
                    window.closeHints()
                    browserUi.status_text =
                            "Scroll target selection failed: region is no longer available"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else if (result.action === "click") {
                var clickScript = window.hintClickScript(result.element_id)
                var clickStarted = window.runBrowserScript(view, clickScript, function(value) {
                    window.closeHints()
                    browserUi.status_text = value === true
                            ? "Hint target activated"
                            : "Hint target is no longer clickable"
                    window.showCommandNotice(browserUi.status_text, value !== true)
                })
                if (!clickStarted) {
                    window.closeHints()
                    browserUi.status_text = "Hint target is no longer clickable"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else {
                window.hintResults = []
                window.hintInput = ""
                var rejectionStatus = browserUi.status_text
                browserUi.cancel_hints()
                browserUi.status_text = rejectionStatus
            }
        })
    }

}
