import QtQuick
import QtQuick.Controls

FerricCommandDialog {
    id: surface
    objectName: "formResubmissionSurface"

    required property var browserUi
    property var reloadRequest: null
    property bool returnFocus: true
    readonly property bool ownerCurrent: {
        var epoch = browserUi.input_epoch
        var state = browserUi.load_state
        return targetCurrent()
    }
    signal finished(bool resubmit)

    function targetCurrent() {
        var pending = reloadRequest
        if (!pending || !pending.view || !pending.view.ownsTab()) return false
        var target = pending.target
        return pending.ui === browserUi && pending.view.viewUi === pending.ui
            && pending.host === hostWindow && pending.view.viewHost === pending.host
            && hostWindow.activeWebView() === pending.view
            && pending.view.tabIndex === pending.ui.active_tab_index
            && pending.view.documentRequestOwner === pending.documentOwner
            && pending.view.url.toString() === pending.url
            && (!pending.content || pending.content.navigationSerial === pending.serial)
            && pending.ui.request_target_current(target.tabId, target.generation, target.documentId)
    }
    function finish(resubmit) {
        var current = targetCurrent()
        returnFocus = current
        finished(resubmit && current)
    }

    visible: !!reloadRequest
    enabled: !visible || ownerCurrent
    restoreFocus: returnFocus
    commandText: ":reload"
    title: "Resend form data?"
    message: "Loading this page again will resend form data and may repeat its action."
    dialogWidth: 640 * scale
    dialogHeight: 320 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 65
    actions: [
        { id: "cancel", key: "n", label: "Keep page", safe: true },
        { id: "resubmit", key: "y", label: "Resend form data", enabled: surface.ownerCurrent }
    ]
    onOpening: {
        returnFocus = true
        // A just-closed site dialog can still own Quick focus until its
        // deferred return runs. Never retain its hidden control as our target.
        var target = previousFocusItem
        var container = hostWindow ? hostWindow.contentItem : null
        while (target && container) {
            if (target === container) { target = null; break }
            container = container.parent
        }
        if (!target || target.visible === false || target.enabled === false)
            previousFocusItem = reloadRequest ? reloadRequest.view : null
    }
    onActionRequested: function(action) { finish(action === "resubmit") }
    onOwnerCurrentChanged: if (visible && !ownerCurrent && staleTimer) staleTimer.restart()
    Timer {
        id: staleTimer
        interval: 0
        onTriggered: if (surface.visible && !surface.ownerCurrent) surface.finish(false)
    }
}
