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
    function focusPermitted(hostWindow, target) {
        var scope = hostWindow ? hostWindow.modalInputScope : null
        return !scope || scope.allowsFocus(target)
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
    function restoreOverlayFocus(hostWindow) {
        if (browserWindow.runtimeDisposing === true) {
            focusReturnStack = []; pendingFocusReturns = []
            overlayTimer.stop(); modeTimer.stop()
            return
        }
        var host = hostWindow || browserWindow
        var stack = focusReturnStack.slice(0)
        var entry = null
        // Windows dismiss overlays independently. Keep nesting within each
        // host, without consuming another window's pending focus return.
        for (var index = stack.length - 1; index >= 0; --index) {
            if (stack[index].host === host) {
                entry = stack.splice(index, 1)[0]
                break
            }
        }
        focusReturnStack = stack
        if (!entry || !entry.host || entry.host.runtimeReleased === true) return
        var pending = pendingFocusReturns.slice(0)
        pending.push(entry)
        pendingFocusReturns = pending
        overlayTimer.restart()
    }
    function discardHost(hostWindow) {
        focusReturnStack = focusReturnStack.filter(function(entry) { return entry.host !== hostWindow })
        pendingFocusReturns = pendingFocusReturns.filter(function(entry) { return entry.host !== hostWindow })
        if (pendingFocusReturns.length === 0) overlayTimer.stop()
    }
    function applyOverlayFocus() {
        var pending = pendingFocusReturns
        pendingFocusReturns = []
        if (browserWindow.runtimeDisposing === true) return
        for (var i = 0; i < pending.length; ++i) {
            var entry = pending[i]
            if (!entry.host || entry.host.runtimeReleased === true) continue
            if (focusTargetAvailable(entry.target) && focusPermitted(entry.host, entry.target)) entry.target.forceActiveFocus()
            else if (focusTargetAvailable(entry.fallback) && focusPermitted(entry.host, entry.fallback)) entry.fallback.forceActiveFocus()
            else if (entry.host === browserWindow) {
                var view = browserWindow.activeWebView()
                if (focusTargetAvailable(view) && focusPermitted(entry.host, view)) view.forceActiveFocus()
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
        if (browserWindow.runtimeDisposing === true
                || (browserWindow.modalInputScope && browserWindow.modalInputScope.activeSurface)
                || focusReturnStack.some(function(entry) { return entry.host === browserWindow })) return
        if (focusTargetAvailable(target)) target.forceActiveFocus()
        else {
            var view = browserWindow.activeWebView()
            if (focusTargetAvailable(view)) view.forceActiveFocus()
        }
    }
}
