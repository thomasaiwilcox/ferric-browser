import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/TargetSearchPresentation.js" as TargetSearchPresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

FerricBrowserRuntimeServices {
    id: window


    readonly property var clipboardBridge: window.clipboardBridgeObject
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
        var requested = String(id)
        function find(host) {
            if (!host) {
                return null
            }
            var active = host.activeDownloads || ({})
            if (active[requested]) {
                return active[requested]
            }
            var pending = host.pendingDownloadRequests || ({})
            return pending[requested] || null
        }
        var result = find(window)
        if (result) {
            return result
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            result = find(entries[i] && entries[i].host)
            if (result) {
                return result
            }
        }
        var popups = window.popupWindowRegistry || []
        for (var j = 0; j < popups.length; ++j) {
            result = find(popups[j] && popups[j].host)
            if (result) {
                return result
            }
        }
        return null
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
        if (window.pendingDownloadId.length > 0) {
            return true
        }
        for (var key in window.pendingDownloadRequests) {
            if (window.pendingDownloadRequests[key]) {
                return true
            }
        }
        for (var activeKey in window.activeDownloads) {
            var download = window.activeDownloads[activeKey]
            if (download && !download.isFinished) {
                return true
            }
        }
        return false
    }

    function hasActiveShutdownRequestsFor(ui, hostWindow) {
        if (!ui) {
            return false
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
        window.runBrowserScript(view, probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab suspension refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_suspend(id)) {
                return
            }
            window.executePendingEngineAction()
        }, 0)
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
        window.runBrowserScript(view, probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab discard refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_discard(id)) {
                return
            }
            window.executePendingEngineAction()
        }, 0)
    }

    function shutdownPageProbeScript() {
        return BrowserScripts.shutdownPageProbe()
    }

    function checkPageStateBeforeQuit() {
        var generation = ++window.shutdownPageProbeGeneration
        shutdownPageProbeTimer.restart()
        var candidates = []
        for (var i = 0; i < tabViews.length; ++i) {
            var view = tabViewAt(i)
            if (view && view.lifecycleState === WebEngineView.LifecycleState.Active) {
                candidates.push(view)
            }
        }
        var probe = function(position) {
            if (generation !== window.shutdownPageProbeGeneration) {
                return
            }
            if (position >= candidates.length) {
                shutdownPageProbeTimer.stop()
                window.finalizeQuit()
                return
            }
            window.runBrowserScript(candidates[position], window.shutdownPageProbeScript(), function(result) {
                if (generation !== window.shutdownPageProbeGeneration) {
                    return
                }
                if (result === "dirty") {
                    shutdownPageProbeTimer.stop()
                    window.shutdownPagePromptReason = "A page has unsaved form or editor state."
                    window.shutdownPagePromptVisible = true
                    return
                }
                if (result !== "clean") {
                    shutdownPageProbeTimer.stop()
                    window.shutdownPagePromptReason = "Ferric Browser could not verify every page before closing."
                    window.shutdownPagePromptVisible = true
                    return
                }
                probe(position + 1)
            }, 0)
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
        window.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        shutdownPageProbeTimer.stop()
        window.shutdownPromptVisible = false
        window.shutdownPagePromptVisible = false
        window.beginApplicationShutdown()
    }

    function cancelDownloadsAndQuit() {
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
        window.pendingDownloadRequests = ({})
        for (var activeKey in window.activeDownloads) {
            var download = window.activeDownloads[activeKey]
            if (download && !download.isFinished) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(activeKey, "cancelled", download.receivedBytes, true)
            }
        }
        window.activeDownloads = ({})
        window.cancelShutdownRequestsFor(browserUi, window)
        window.continueQuitRequest()
    }

    Timer {
        id: libraryPublicationTimer
        interval: 0
        onTriggered: window.refreshLibraryManager()
    }
    Connections {
        target: browserUi
        function onLibrary_valuesChanged() { libraryPublicationTimer.restart() }
        function onLibrary_kindChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_sourcesChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_targetsChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_transitionsChanged() { libraryPublicationTimer.restart() }
    }
    function updateLibraryRows(rows) {
        // Retain delegates and drafts for stable records during asynchronous reads.
        for (var i = 0; i < rows.length; ++i) {
            var found = -1
            for (var j = i; j < libraryEntries.count; ++j) {
                var current = libraryEntries.get(j)
                if (current.entryKind === rows[i].entryKind && current.entryId === rows[i].entryId) { found = j; break }
            }
            if (found < 0) libraryEntries.insert(i, rows[i])
            else {
                if (found !== i) libraryEntries.move(found, i, 1)
                libraryEntries.set(i, rows[i])
            }
        }
        if (libraryEntries.count > rows.length) libraryEntries.remove(rows.length, libraryEntries.count - rows.length)
    }
    function refreshActiveLibrary() {
        window.pendingLibraryDelete = ""
        if (browserUi.library_kind === "journey") {
            if (window.libraryJourneyCurrentOnly) window.runJourneyCurrentQuery()
            else window.runJourneyQuery(window.libraryJourneyExpandedNode.length > 0
                ? window.journeyExpandArgument(window.libraryJourneyExpandedNode)
                : window.journeySearchArgument(window.libraryJourneySearchText))
        } else if (browserUi.library_kind === "actions") return browserUi.execute_command(":action-list")
        else if (browserUi.library_kind === "context-routes") return browserUi.execute_command(":context-route list")
        else browserUi.refresh_library()
    }
    function refreshLibraryManager() {
        window.pendingLibraryDelete = ""
        libraryGraphEntries.clear()
        libraryGraphNodes.clear()
        window.libraryGraphLineData = []
        var kind = browserUi.library_kind
        var values = browserUi.library_values
        var lines = values.length > 0 ? values.split("\n") : []
        window.libraryTotalEntries = lines.length
        var visibleLines = lines
        if (kind !== "journey") {
            var pageCount = Math.max(1, Math.ceil(lines.length / window.libraryPageSize))
            window.libraryPage = Math.max(0, Math.min(window.libraryPage, pageCount - 1))
            var pageStart = window.libraryPage * window.libraryPageSize
            visibleLines = lines.slice(pageStart, pageStart + window.libraryPageSize)
        }
        var rows = []
        for (var i = 0; i < visibleLines.length; ++i) {
            var fields = visibleLines[i].split("\t")
            var row = {label: "", secondary: "", detail: "", entryId: fields[0], entryKind: "", nodeId: ""}
            if ((kind === "history" || kind === "bookmarks") && fields.length >= 3) {
                row.label = fields[1] || fields[2]
                row.secondary = fields[2]
                row.entryKind = kind === "history" ? "history" : "bookmark"
            } else if (kind === "journey" && fields.length >= 4) {
                row.label = fields[1] || fields[2]
                row.secondary = fields[2] + " · " + ChromePresentation.journeyTransitionLabel(fields[3])
                row.detail = row.label + "\n" + fields[2] + "\nTransition: " + fields[3]
                    + (fields[4] ? "\nSource: " + fields[4] : "") + "\nRecord: " + fields[0]
                row.entryKind = "journey"; row.nodeId = fields[0]
            } else if (kind === "quickmarks" && fields.length >= 2) {
                row.label = fields[0]; row.secondary = fields[1]; row.entryKind = "quickmark"
            } else if (kind === "actions" && fields.length >= 3) {
                row.label = fields[1]; row.secondary = fields[2]; row.entryKind = "action"
                row.detail = fields[0] + "\n" + fields[2] + (fields[3] ? "\nExample: " + fields[3] : "")
            } else if (kind === "context-routes" && fields.length >= 6) {
                row.label = fields[1]; row.secondary = "Open in context " + fields[2]; row.entryKind = "route"
                row.detail = "Rule: " + fields[0] + "\n" + fields[3] + "\n" + fields[4] + "\nEntry points: " + fields[5]
            } else continue
            rows.push(row)
        }
        window.updateLibraryRows(rows)
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
                var graphNodeWidth = libraryManager.graphNodeWidth
                var graphNodeHeight = libraryManager.graphNodeHeight
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
                    var source = nodeLabels[sourceId] || "Earlier page"
                    var target = nodeLabels[targetId] || "Another page"
                    libraryGraphEntries.append({
                        label: source + " → " + target,
                        secondary: ChromePresentation.journeyTransitionLabel(transition)
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
                        label: "Showing " + graphNodeLimit + " pages on the map",
                        secondary: "Search or choose Nearby pages to narrow the view"
                    })
                }
                if (libraryGraphEntries.count === 0) {
                    libraryGraphEntries.append({
                        label: "No connections to show",
                        secondary: "Try showing all pages or searching for another page"
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
        window.pendingLibraryDelete = ""
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
            return true
        }
        return false
    }

    function showPrivateHistoryTransfer(entryId, title, url) {
        if (!window.temporaryProfile || String(url).length === 0) {
            browserUi.status_text = "Private history transfer is unavailable"
            return
        }
        if (window.privateBookmarkTransferPending) {
            var message = "A bookmark is still saving. Wait before choosing another page."
            browserUi.status_text = message
            window.showCommandNotice(message, false)
            return
        }
        window.privateBookmarkTransferMessage = ""
        window.privateBookmarkTransferDetail = ""
        window.privateBookmarkTransferFailed = false
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

    function checkPrivateBookmarkTransfer() {
        var ui = window.privateBookmarkTransferUi
        if (!window.privateBookmarkTransferPending || !ui) return
        var state = String(ui.bookmark_transfer_state)
        if (String(ui.bookmark_transfer_id) !== window.privateBookmarkTransferReceipt) {
            if (state === "idle") window.finishPrivateBookmarkTransfer(false, "The save could not be confirmed. Check the destination's bookmarks before trying again.", "")
            return
        }
        if (state === "succeeded") window.finishPrivateBookmarkTransfer(true, "Bookmark saved in " + window.privateBookmarkTransferLabel, "")
        else if (state === "failed") window.finishPrivateBookmarkTransfer(false, "The bookmark couldn't be saved. Your private page is still here. Try again.", String(ui.bookmark_transfer_detail || ""))
        else if (state === "idle") window.finishPrivateBookmarkTransfer(false, "The save could not be confirmed. Check the destination's bookmarks before trying again.", "")
    }

    function finishPrivateBookmarkTransfer(succeeded, message, detail) {
        window.privateBookmarkTransferPending = false
        window.privateBookmarkTransferReceipt = ""
        window.privateBookmarkTransferUi = null
        window.privateBookmarkTransferWindow = null
        window.privateBookmarkTransferMessage = message
        window.privateBookmarkTransferDetail = detail
        window.privateBookmarkTransferFailed = !succeeded
        browserUi.status_text = message
        if (!window.privateHistoryTransferVisible) window.showCommandNotice(message, !succeeded)
    }

    function confirmPrivateHistoryBookmark(profileName, profileLabel) {
        if (window.privateBookmarkTransferPending) return
        window.privateBookmarkTransferLabel = String(profileLabel || profileName)
        window.privateBookmarkTransferMessage = "Saving bookmark in " + window.privateBookmarkTransferLabel + "…"
        window.privateBookmarkTransferDetail = ""
        window.privateBookmarkTransferFailed = false
        var entries = window.browserWindowRegistry || []
        for (var index = 0; index < entries.length; ++index) {
            var entry = entries[index]
            if (entry && entry.ui && !entry.ephemeralProfile && String(entry.profileName || "") === String(profileName)) {
                if (!entry.ui.queue_bookmark_transfer(window.privateHistoryTransferUrl, window.privateHistoryTransferTitle)) {
                    window.finishPrivateBookmarkTransfer(false, "The bookmark couldn't be queued. Check the destination profile and try again.", String(entry.ui.status_text || ""))
                    return
                }
                window.privateBookmarkTransferUi = entry.ui
                window.privateBookmarkTransferReceipt = String(entry.ui.bookmark_transfer_id)
                window.privateBookmarkTransferPending = true
                window.checkPrivateBookmarkTransfer()
                return
            }
        }
        window.privateBookmarkTransferPending = true
        var destination = window.browserWindowFactory.createObject(null, {
            windowStartupUrl: "rb://start",
            windowProfileName: profileName,
            windowProfileLabel: String(profileLabel || profileName),
            windowPrivateProfile: false,
            windowBookmarkTransferUrl: window.privateHistoryTransferUrl,
            windowBookmarkTransferTitle: window.privateHistoryTransferTitle
        })
        if (!destination) { window.finishPrivateBookmarkTransfer(false, "The destination profile couldn't be opened. Try again.", ""); return }
        window.privateBookmarkTransferWindow = destination
        window.requestActivate()
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
            return true
        }
        return false
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
        var path = browserUi.local_file_path(selected || "")
        if (path.length === 0 || !browserUi.export_journey(path)) {
            window.journeyExportPreviewVisible = true
            return
        }
        window.journeyExportPreviewVisible = false
    }

    function finishDiagnosticsExport() {
        var selected = fileDialogSurfaces.diagnosticsExportSelectedFile
        var ui = window.diagnosticsUi
        if (!window.diagnosticsOwner(ui, window.diagnosticsHost)
                || (window.diagnosticsExportUi && window.diagnosticsExportUi !== ui)) return
        var path = ui.local_file_path(selected || "")
        if (!path || !ui.export_diagnostics(path)) {
            window.diagnosticsExportNotice = "The report couldn't be saved. Review the details and try again."
            window.diagnosticsExportDetail = path ? String(ui.status_text) : "Choose a local file on this computer."
        } else {
            window.diagnosticsExportDetail = ""
            window.diagnosticsExportNotice = "Report saved. Refresh to prepare another report."
        }
        window.diagnosticsVisible = true
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

    function hintSelectorPolicySnapshot(family) {
        return window.hintWorkspace.hintSelectorPolicySnapshot(family)
    }

    function hintCollectorScript(family, selectorSnapshot, scopeLeaseId, includeFacets) {
        return window.hintWorkspace.hintCollectorScript(family, selectorSnapshot, scopeLeaseId, includeFacets)
    }

    function targetSearchIdentity(candidate) {
        return window.hintWorkspace.targetSearchIdentity(candidate)
    }

    function targetSearchMeaning(candidate) {
        return window.hintWorkspace.targetSearchMeaning(candidate)
    }

    function targetSearchRows() {
        return window.hintWorkspace.targetSearchRows()
    }

    function reconcileTargetSearchSelection() {
        return window.hintWorkspace.reconcileTargetSearchSelection()
    }

    function applyTargetSearchQuery(query) {
        return window.hintWorkspace.applyTargetSearchQuery(query)
    }

    function selectTargetSearchIdentity(identity) {
        return window.hintWorkspace.selectTargetSearchIdentity(identity)
    }

    function moveTargetSearchSelection(delta) {
        return window.hintWorkspace.moveTargetSearchSelection(delta)
    }

    function confirmTargetSearchSelection() {
        return window.hintWorkspace.confirmTargetSearchSelection()
    }

    function hintDirtyRevisionScript() {
        return window.hintWorkspace.hintDirtyRevisionScript()
    }

    function hintStopTrackingScript(scopeLeaseId) {
        return window.hintWorkspace.hintStopTrackingScript(scopeLeaseId)
    }

    function spatialScopeGeometryScript(scopeLeaseId) {
        return window.hintWorkspace.spatialScopeGeometryScript(scopeLeaseId)
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
        return window.hintWorkspace.hintFreshScript(candidate)
    }

    function completeHintScopeHandoff(view, request, candidate, actionEpoch) {
        return window.hintWorkspace.completeHintScopeHandoff(view, request, candidate, actionEpoch)
    }

    function completeGridScopeHandoff(view, request, candidate, actionEpoch) {
        return window.hintWorkspace.completeGridScopeHandoff(view, request, candidate, actionEpoch)
    }

    function hintPointerPreflightScript(candidate, viewWidth, viewHeight) {
        return window.hintWorkspace.hintPointerPreflightScript(candidate, viewWidth, viewHeight)
    }

    function hintFocusScript(candidate) {
        return window.hintWorkspace.hintFocusScript(candidate)
    }

    function hintClickScript(candidate) {
        return window.hintWorkspace.hintClickScript(candidate)
    }

    function hintSetScrollTargetScript(elementId, framePath) {
        return window.hintWorkspace.hintSetScrollTargetScript(elementId, framePath)
    }

    function startHintCollection() {
        return window.hintWorkspace.startHintCollection()
    }

    function pollHintRefresh() {
        return window.hintWorkspace.pollHintRefresh()
    }

    function applyHintInteraction(action, text) {
        return window.hintWorkspace.applyHintInteraction(action, text)
    }

    function beginHintPointerConfirmation() {
        return window.hintWorkspace.beginHintPointerConfirmation()
    }

    function rotateHintCollision() {
        return window.hintWorkspace.rotateHintCollision()
    }

    function closeHints() {
        return window.hintWorkspace.closeHints()
    }

    function stopHintTracking() {
        return window.hintWorkspace.stopHintTracking()
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
            if (tabViews.indexOf(view) >= 0) return true
            var index = sourceUi ? sourceUi.tab_index_for_id(view.stableTabId) : -1
            if (index < 0 || index > tabViews.length) {
                index = tabViews.length
            }
            if (tabViewAt(index) && tabViewAt(index).stableTabId === view.stableTabId)
                window.removeTabViewAt(index, null)
            view.viewUi = sourceUi
            view.viewHost = window
            view.viewTabs = tabs
            tabViews = tabViews.slice(0, index).concat([view], tabViews.slice(index))
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
        var sourceTabId = view.stableTabId
        var sourceUrl = view.url ? view.url.toString() : "about:blank"
        var contentHistory = sourceUi.content_history_export(view.tabIndex >= 0 ? view.tabIndex : sourceUi.active_tab_index)
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
            return false
        }
        window.removeTabViewAt(index, null)
        if (contentHistory.length > 0) browserUi.content_history_import(browserUi.active_tab_index, contentHistory)
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
        tabViews = tabViews.slice(0, index).concat([view], tabViews.slice(index))
        for (var reindex = index; reindex < tabViews.length; ++reindex) {
            tabViews[reindex].tabIndex = reindex
            tabViews[reindex].viewModel = tabs.get(reindex)
        }
        window.updateBrowserWindowView(browserUi, view)
        if (!window.completeDetachedSource(sourceUi, sourceHost, payload)) {
            tabViews = tabViews.slice(0, index).concat(tabViews.slice(index + 1))
            view.parent = null
            view.visible = false
            view.stableTabId = sourceTabId
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
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
            if (operationId.length > 0)
                sourceUi.complete_transfer_operation(operationId, false)
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
                tabViews = tabViews.concat([fallback])
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
        return window.hintWorkspace.showHintActions(label)
    }

    function openHintActionPalette(label, capabilities, workspace) {
        workspace = workspace || window.hintWorkspace
        var browserUi = workspace.browserUi
        var host = workspace.hostWindow
        var view = workspace.activeWebView()
        if (!view) {
            workspace.closeHints()
            return
        }
        var selected = null
        for (var candidateIndex = 0; candidateIndex < workspace.hintResults.length; ++candidateIndex) {
            if (String(workspace.hintResults[candidateIndex].label) === String(label)) {
                selected = workspace.hintResults[candidateIndex]
                break
            }
        }
        if (!selected) return
        var items = []
        var hasUrl = selected.href !== null && String(selected.href || "").length > 0
        if (Array.isArray(capabilities)) {
            for (var capabilityIndex = 0; capabilityIndex < capabilities.length; ++capabilityIndex) {
                var capability = capabilities[capabilityIndex]
                if (!capability || typeof capability.id !== "string"
                        || typeof capability.label !== "string") {
                    continue
                }
                items.push(window.contextMenuItem(
                               capability.label, "hint-userscript-action", label,
                               "hint.capability." + capability.id,
                               capability.enabled === true))
            }
        } else {
            items.push(window.contextMenuItem("Activate / open current", "hint-userscript-action", label, "hint.current"))
        }
        if (hasUrl && !Array.isArray(capabilities)) {
            items.push(window.contextMenuItem("Open in foreground tab", "hint-userscript-action", label, "hint.tab"))
            items.push(window.contextMenuItem("Open in background tab", "hint-userscript-action", label, "hint.tab-bg"))
            items.push(window.contextMenuItem("Open in new window", "hint-userscript-action", label, "hint.window"))
            items.push(window.contextMenuItem("Yank URL", "hint-userscript-action", label, "hint.yank"))
            items.push(window.contextMenuItem("Clean-yank URL", "hint-userscript-action", label, "hint.clean-yank"))
            items.push(window.contextMenuItem("Download", "hint-userscript-action", label, "hint.download"))
            items.push(window.contextMenuItem("Open in ephemeral window", "hint-userscript-action", label, "hint.ephemeral"))
        }
        if (hasUrl && !Array.isArray(capabilities)
                && browserUi.select_userscript_action_subject("link")) {
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
        window.pendingContextMenuHost = host
        window.contextMenuItems = items
        window.captureOverlayFocus(host, view)
        contextMenu.openAt(host, Math.round(host.width / 2),
                           Math.round(host.height / 2))
        browserUi.status_text = "Hint actions"
    }

    function activateHint(label, actionId, localSelection) {
        return window.hintWorkspace.activateHint(label, actionId, localSelection)
    }

}
