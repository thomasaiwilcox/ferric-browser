import QtQuick

// Deferred focus work belongs to this controller and is cancelled with it.
QtObject {
    id: controller
    required property var browserWindow
    property var focusReturnStack: []
    property var pendingFocusReturns: []
    property var modeFocusReturnTarget: null
    property bool modeFocusCaptured: false
    property var pendingModeFocusTarget: null
    property Timer overlayTimer: Timer {
        interval: 0
        onTriggered: controller.applyOverlayFocus()
    }
    property Timer modeTimer: Timer {
        interval: 0
        onTriggered: controller.applyModeFocus()
    }

    function focusTargetAvailable(target) {
        return !!target && !!target.forceActiveFocus
                && target.visible !== false && target.enabled !== false
    }
    function captureOverlayFocus(hostWindow, fallbackTarget) {
        if (browserWindow.runtimeDisposing === true) return
        var host = hostWindow || browserWindow
        // A newer overlay owns this host. Discard a queued return from the
        // surface it replaced, while retaining returns for other windows.
        pendingFocusReturns = pendingFocusReturns.filter(function(entry) { return entry.host !== host })
        var target = host ? host.activeFocusItem : null
        if (!focusTargetAvailable(target)) target = fallbackTarget || null
        var stack = focusReturnStack.slice(0)
        stack.push({ host: host, target: target, fallback: fallbackTarget || null })
        focusReturnStack = stack
    }
    function restoreOverlayFocus() {
        if (browserWindow.runtimeDisposing === true) {
            focusReturnStack = []; pendingFocusReturns = []
            overlayTimer.stop(); modeTimer.stop()
            return
        }
        var stack = focusReturnStack.slice(0)
        var entry = stack.length > 0 ? stack.pop() : null
        focusReturnStack = stack
        if (!entry || !entry.host || entry.host.runtimeReleased === true) return
        var pending = pendingFocusReturns.slice(0)
        pending.push(entry)
        pendingFocusReturns = pending
        overlayTimer.restart()
    }
    function applyOverlayFocus() {
        var pending = pendingFocusReturns
        pendingFocusReturns = []
        if (browserWindow.runtimeDisposing === true) return
        for (var i = 0; i < pending.length; ++i) {
            var entry = pending[i]
            if (!entry.host || entry.host.runtimeReleased === true) continue
            if (focusTargetAvailable(entry.target)) entry.target.forceActiveFocus()
            else if (focusTargetAvailable(entry.fallback)) entry.fallback.forceActiveFocus()
            else if (entry.host === browserWindow) {
                var view = browserWindow.activeWebView()
                if (focusTargetAvailable(view)) view.forceActiveFocus()
            }
        }
    }
    function captureModeFocus() {
        if (modeFocusCaptured || browserWindow.runtimeDisposing === true) return
        var target = browserWindow.activeFocusItem
        if (!focusTargetAvailable(target)) target = browserWindow.activeWebView()
        modeFocusReturnTarget = target
        modeFocusCaptured = true
    }
    function restoreModeFocus() {
        pendingModeFocusTarget = modeFocusCaptured ? modeFocusReturnTarget : browserWindow.activeWebView()
        modeFocusReturnTarget = null
        modeFocusCaptured = false
        if (browserWindow.runtimeDisposing !== true) modeTimer.restart()
    }
    function applyModeFocus() {
        var target = pendingModeFocusTarget
        pendingModeFocusTarget = null
        // A command can open a modal immediately before leaving command mode.
        if (browserWindow.runtimeDisposing === true || focusReturnStack.length > 0) return
        if (focusTargetAvailable(target)) target.forceActiveFocus()
        else {
            var view = browserWindow.activeWebView()
            if (focusTargetAvailable(view)) view.forceActiveFocus()
        }
    }
}
