import QtQuick
import "../scripts/ChromePresentation.js" as ChromePresentation

// Each window presents its own profile session. The native registry retains
// the captured transport owner; saved history never grants control authority.
Item {
    id: workspace
    required property var browserWindow
    required property var hostWindow
    required property var browserUi
    required property var rootWindow
    property var state: workspace
    property var downloadsModel: rows
    property bool downloadManagerVisible: false
    property string historyStatus: "ready"
    property var rateSamples: ({})
    readonly property bool active: state.downloadManagerVisible
    signal opening()
    signal closed()
    anchors.fill: parent
    z: 40
    ListModel { id: rows }
    FerricDownloadInitiator {
        id: downloadInitiator
        rootWindow: workspace.rootWindow
        hostWindow: workspace.hostWindow
        browserUi: workspace.browserUi
    }
    function dispatchRequest() {
        if (!downloadInitiator.busy && browserUi.take_download_request())
            downloadInitiator.start(browserUi.download_request_token, browserUi.download_request_url)
    }
    function shutdown() { downloadInitiator.shutdown() }

    function show() {
        opening()
        state.downloadManagerVisible = true
        refresh()
    }
    function close() {
        state.downloadManagerVisible = false
        closed()
    }
    function metrics(id, download) {
        var requested = String(id)
        if (!download) {
            delete rateSamples[requested]
            return {received:0,total:-1,speed:0,reason:""}
        }
        var bytes = Math.max(0, Number(download.receivedBytes || 0))
        var sample = ChromePresentation.downloadRateSample(rateSamples[requested],bytes,Date.now())
        rateSamples[requested] = sample
        var total = Number(download.totalBytes)
        return {received:bytes,total:isFinite(total) && total >= 0 ? total : -1,
                speed:sample.rate,reason:rootWindow.boundedPageDialogText(download.interruptReasonString)}
    }
    function refresh() {
        var values = browserUi.download_rows()
        historyStatus = String(browserUi.download_history_status())
        if (values.length === 0 && historyStatus === "loading" && downloadsModel.count > 0) return
        var visibleIds = values.map(function(row) { return String(row.id) })
        Object.keys(rateSamples).forEach(function(id) { if (visibleIds.indexOf(id) < 0) delete workspace.rateSamples[id] })
        for (var i = 0; i < values.length; ++i) {
            var value = values[i]
            var live = rootWindow.profileResourceRegistry.downloadFor(browserUi, value.id)
            if (!live) delete rateSamples[String(value.id)]
            var currentMetrics = live ? workspace.metrics(value.id, live)
                : {received:Number(value.bytes)||0,total:Number(value.total === undefined ? -1 : value.total),speed:0,reason:String(value.reason||"")}
            var row = {
                id: String(value.id), state: String(value.state),
                bytes: live ? String(currentMetrics.received) : String(value.bytes),
                destination: String(value.destination), displayName: String(value.displayName),
                active: !!value.active, retryPending: !!value.retryPending, retryAvailable: !!value.retryAvailable, fileReady: !!value.fileReady, finishing: !!value.finishing,
                total: currentMetrics.total, speed: currentMetrics.speed, reason: currentMetrics.reason
            }
            // Preserve focus and Details disclosure while progress changes.
            var previous = i
            while (previous < downloadsModel.count && downloadsModel.get(previous).id !== row.id) ++previous
            if (previous < downloadsModel.count) {
                if (previous !== i) downloadsModel.move(previous, i, 1)
                downloadsModel.set(i, row)
            } else downloadsModel.insert(i, row)
        }
        if (downloadsModel.count > values.length)
            downloadsModel.remove(values.length, downloadsModel.count - values.length)
    }
    function openFile(id, reveal) {
        if (browserUi.download_desktop_action(id, reveal) && browserUi.download_desktop_uri.length > 0)
            rootWindow.openExternalUri(browserUi, browserUi.download_desktop_uri)
    }
    function requestAction(id, action) {
        var actionIds = {pause: "browser.download.pause", resume: "browser.download.resume", cancel: "browser.download.cancel", retry: "browser.download.retry"}
        if (actionIds[action] && browserUi.execute_ui_action(actionIds[action], id)) {
            if (hostWindow === rootWindow) rootWindow.executePendingEngineAction()
            else hostWindow.applySwitcherEngineAction()
        }
        dispatchRequest()
        refresh()
    }
    FerricDownloadManager {
        objectName: "downloadManager"
        browserWindow: workspace.browserWindow
        hostWindow: workspace.hostWindow
        visible: workspace.active
        downloadsModel: workspace.downloadsModel
        sessionOnly: workspace.historyStatus === "not-stored"
        listPending: workspace.historyStatus === "loading"
        preparingDownload: downloadInitiator.busy
        onCancelRequestRequested: downloadInitiator.cancel()
        listFailed: workspace.historyStatus === "unavailable"
        onRefreshRequested: {
            workspace.browserUi.refresh_download_history()
            workspace.refresh()
        }
        onCloseRequested: workspace.close()
        onOpenRequested: function(id, reveal) { workspace.openFile(id, reveal) }
        onActionRequested: function(id, action) { workspace.requestAction(id, action) }
    }
    Timer {
        id: downloadMetricsTimer
        interval: 1000
        repeat: true
        running: workspace.active
        onTriggered: workspace.refresh()
    }
    Connections {
        target: workspace.browserUi
        function onLibrary_pendingChanged() { if (workspace.active) workspace.refresh() }
        function onLibrary_errorChanged() { if (workspace.active) workspace.refresh() }
        function onProfile_bootstrap_pendingChanged() {
            if (workspace.browserUi.profile_bootstrap_pending) {
                workspace.state.downloadManagerVisible = false
                workspace.downloadsModel.clear()
                workspace.rateSamples = ({})
            }
        }
    }
}
