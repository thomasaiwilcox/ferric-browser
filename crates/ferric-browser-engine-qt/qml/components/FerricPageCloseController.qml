import QtQuick
import QtWebEngine
import "../scripts/CloseRequests.js" as CloseRequests

// Trusted host-owned decisions. Only the captured page is probed for a local
// close. The process broker advances after the view/Core release callback.
Item {
    id: controller
    required property var host
    required property var ui
    required property var rootWindow
    anchors.fill: parent
    z: 100
    property string nonce: ""
    property var capturedView: null
    property int generation: 0
    property bool confirmationVisible: false
    property bool dirtyVisible: false
    property string dirtyReason: ""
    readonly property bool armed: nonce.length > 0

    function begin(action) {
        var fields = action.split("\t")
        if (fields.length !== 3) return
        if (armed) { ui.cancel_page_close(fields[1]); return }
        var index = ui.page_close_index(fields[1])
        var view = index >= 0 ? host.tabViewAt(index) : null
        var busy = host === rootWindow
                ? (host.shutdownConfirmationVisible || host.shutdownPromptVisible || host.shutdownPagePromptVisible || host.applicationShutdownInProgress)
                : (host.windowCloseConfirmationVisible || host.windowShutdownPromptVisible || host.windowShutdownPagePromptVisible || host.windowShutdownStoragePromptVisible)
        if (!view || view.stableTabId !== fields[2] || busy) {
            ui.cancel_page_close(fields[1]); return
        }
        nonce = fields[1]
        capturedView = view
        if (ui.tab_count === 1) {
            if (host === rootWindow) host.beginQuitRequest(":window-close")
            else host.beginQuitRequest()
        } else confirmationVisible = true
        // Share the existing activation serialization without superseding the
        // admitted chain. A newer reentrant intent keeps its priority.
        if (!rootWindow.presentPageCloseDecision(ui,host,nonce)) cancel()
    }
    function cancel() {
        var original = nonce
        nonce = ""
        capturedView = null
        generation += 1
        confirmationVisible = false
        dirtyVisible = false
        probeTimeout.stop()
        if (original.length > 0) {
            if (host === rootWindow) {
                host.shutdownConfirmationVisible = false
                host.shutdownPromptVisible = false
                host.shutdownPagePromptVisible = false
                host.shutdownPageProbeGeneration += 1
                host.shutdownPageProbeTimerObject.stop()
            } else {
                host.windowCloseConfirmationVisible = false
                host.windowShutdownPromptVisible = false
                host.windowShutdownStoragePromptVisible = false
                host.cancelPageStateProbe()
            }
            ui.cancel_page_close(original)
        }
    }
    function authorizeWindowClose() {
        if (!armed) return true
        if (ui.tab_count !== 1 || !ui.authorize_page_close(nonce)) { cancel(); return false }
        return true
    }
    function released() {
        if (!armed) return
        var original = nonce
        nonce = ""
        capturedView = null
        generation += 1
        confirmationVisible = false
        dirtyVisible = false
        probeTimeout.stop()
        ui.page_close_released(original)
    }
    function probe() {
        confirmationVisible = false
        var expected = ++generation
        probeTimeout.restart()
        CloseRequests.probePages([capturedView], function() { return expected === generation },
            function(view, reply) { return rootWindow.runBrowserScript(view,rootWindow.shutdownPageProbeScript(),reply,0) },
            function(result) {
                probeTimeout.stop()
                if (result.state === "clean") commit()
                else { dirtyReason = result.reason; dirtyVisible = true }
            })
    }
    function commit() {
        dirtyVisible = false
        var index = ui.page_close_index(nonce)
        if (ui.tab_count <= 1 || index < 0 || host.tabViewAt(index) !== capturedView || !ui.authorize_page_close(nonce)
                || !host.closeTabAtIndex(index)) { cancel(); return }
        rootWindow.profileResourceRegistry.releaseView(capturedView)
        released()
    }
    Timer {
        id: probeTimeout
        interval: 2000
        onTriggered: {
            generation += 1
            controller.dirtyReason = "Page state check timed out"
            controller.dirtyVisible = true
        }
    }
    Timer {
        interval: 500
        repeat: true
        running: controller.armed
        onTriggered: if (ui.page_close_index(controller.nonce) < 0) controller.cancel()
    }
    FerricShutdownDecisionDialog {
        hostWindow: controller.host
        themeWindow: controller.rootWindow
        promptVisible: controller.confirmationVisible
        title: "Close this page?"
        message: "The other pages in this window will remain open."
        commandText: ":tab-close"
        keepLabel: "Keep page open"
        proceedLabel: "Close page"
        onKeepRequested: controller.cancel()
        onProceedRequested: controller.probe()
    }
    FerricShutdownDecisionDialog {
        hostWindow: controller.host
        themeWindow: controller.rootWindow
        promptVisible: controller.dirtyVisible
        title: "Page state may be lost"
        message: controller.dirtyReason + " Close anyway may lose that state."
        commandText: ":tab-close"
        keepLabel: "Keep page open"
        proceedLabel: "Close anyway"
        onKeepRequested: controller.cancel()
        onProceedRequested: controller.commit()
    }
    Component.onDestruction: if (armed && ui) ui.cancel_page_close(nonce)
}
