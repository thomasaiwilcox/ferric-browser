import QtQuick
import QtWebEngine
import "../scripts/DownloadController.js" as DownloadController

// Adapter for the common download surface contract. Rust owns the selected
// destination, bounded writes, operation admission and terminal session state.
QtObject {
    id: transfer
    required property var rootWindow
    required property var browserUi
    required property var initiator
    required property var helper
    required property var profileGroup
    property string downloadId: ""
    property bool streamTransport: true
    property bool streamSaveFailure: false
    property string url: ""
    property string suggestedFileName: "download"
    property string downloadDirectory: ""
    property string downloadFileName: ""
    property int savePageFormat: WebEngineDownloadRequest.UnknownSaveFormat
    property int state: WebEngineDownloadRequest.DownloadRequested
    property bool isFinished: false
    property bool isPaused: false
    property real totalBytes: -1
    property real receivedBytes: 0
    property int interruptReason: WebEngineDownloadRequest.NoReason
    property string interruptReasonString: ""
    property bool scriptPending: false
    property bool scheduled: false
    property bool eof: false
    property var queuedChunk: null
    property int pollDelay: 2
    property bool immediateRetry: true
    property Timer retryTimer: Timer {
        interval: transfer.pollDelay
        onTriggered: transfer.schedulePump()
    }
    property Connections writerEvents: Connections {
        target: transfer.browserUi
        function onStream_download_ready(id) {
            if (String(id) === transfer.downloadId) transfer.schedulePump()
        }
    }
    function schedulePump() {
        if (scheduled || isFinished || isPaused) return
        scheduled = true
        Qt.callLater(function() { transfer.scheduled = false; transfer.pump() })
    }
    function retry() {
        if (isFinished || isPaused) return
        retryTimer.restart()
    }
    function accept() {
        if (isFinished || state !== WebEngineDownloadRequest.DownloadRequested) return
        state = WebEngineDownloadRequest.DownloadInProgress
        initiator.transferStarted(helper)
        Qt.callLater(function() {
            if (transfer.isFinished) return
            if (!browserUi.begin_stream_download(downloadId)) {
                transfer.interrupt("The file couldn't be prepared. Choose a save location and retry.", WebEngineDownloadRequest.FileFailed)
                return
            }
            transfer.schedulePump()
        })
    }
    function pause() {
        if (!isFinished && state === WebEngineDownloadRequest.DownloadInProgress) {
            isPaused = true
            retryTimer.stop()
        }
    }
    function resume() {
        if (!isFinished && state === WebEngineDownloadRequest.DownloadInProgress) {
            isPaused = false
            schedulePump()
        }
    }
    function cancel() {
        if (isFinished) return
        if (browserUi) browserUi.cancel_stream_download(downloadId)
        finish(WebEngineDownloadRequest.DownloadCancelled, "", WebEngineDownloadRequest.UserCanceled)
    }
    function interrupt(message, reason) {
        if (isFinished) return
        if (browserUi) browserUi.cancel_stream_download(downloadId)
        finish(WebEngineDownloadRequest.DownloadInterrupted, message, reason || WebEngineDownloadRequest.NetworkFailed)
    }
    function finish(nextState, message, reason) {
        if (isFinished) return
        retryTimer.stop()
        queuedChunk = null
        interruptReasonString = message || ""
        interruptReason = reason || WebEngineDownloadRequest.NoReason
        isPaused = false
        state = nextState
        isFinished = true
        // Complete synchronously so a finished transport never creates a gap
        // in the window's close warning before file preparation is admitted.
        if (rootWindow && !rootWindow.runtimeDisposing)
            rootWindow.profileResourceRegistry.handleDownloadFinished(profileGroup, transfer)
        if (initiator && helper) initiator.release(helper)
        if (browserUi) browserUi.release_stream_download(downloadId)
        Qt.callLater(function() { transfer.destroy() })
    }
    function pump() {
        if (isFinished || isPaused || state !== WebEngineDownloadRequest.DownloadInProgress) return
        if (!browserUi || !helper || helper.viewRetired || !initiator.liveTransfer(helper)) { cancel(); return }
        var phase = browserUi.stream_download_state(downloadId)
        receivedBytes = browserUi.stream_download_bytes(downloadId)
        if (phase === 5) { finish(WebEngineDownloadRequest.DownloadCompleted, "", 0); return }
        if (phase === 6) { cancel(); return }
        if (phase === 7) { streamSaveFailure = true; interrupt(browserUi.stream_download_error(downloadId), WebEngineDownloadRequest.FileFailed); return }
        if (phase !== 2) { retry(); return }
        if (queuedChunk !== null) {
            if (browserUi.write_stream_download(downloadId, queuedChunk)) {
                queuedChunk = null
                pollDelay = 2
                immediateRetry = true
            }
            retry()
            return
        }
        if (eof) { browserUi.finish_stream_download(downloadId); retry(); return }
        if (scriptPending) return
        scriptPending = true
        var controller = helper.controllerUrl
        var admitted = rootWindow.runBrowserScript(helper, DownloadController.chunk(controller), function(value) {
            transfer.scriptPending = false
            if (transfer.isFinished || !transfer.helper || transfer.helper.controllerUrl !== controller) return
            if (value instanceof ArrayBuffer) {
                transfer.eof = value.byteLength === 0
                transfer.queuedChunk = transfer.eof ? null : value
                transfer.pollDelay = 2
                transfer.immediateRetry = true
                transfer.schedulePump()
            } else if (value === false) {
                if (transfer.immediateRetry) { transfer.immediateRetry = false; transfer.schedulePump() }
                else { transfer.pollDelay = Math.min(50, Math.max(2, transfer.pollDelay * 2)); transfer.retry() }
            } else {
                transfer.interrupt("The download stopped. Check the connection and retry from Downloads.", WebEngineDownloadRequest.NetworkFailed)
            }
        })
        if (!admitted) {
            scriptPending = false
            interrupt("The download stopped. Retry it from Downloads.", WebEngineDownloadRequest.NetworkFailed)
        }
    }
    Component.onDestruction: {
        if (browserUi) browserUi.release_stream_download(downloadId)
    }
}
