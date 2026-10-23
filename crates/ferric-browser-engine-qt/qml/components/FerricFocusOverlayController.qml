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
    property bool followTabFocus: false
    readonly property var activeView: followTabFocus ? browserWindow.activeWebView() : null
    readonly property string activeNativeUrl: activeView ? String(activeView.nativeUrl || "") : ""
    onActiveViewChanged: pageTimer.restart()
    onActiveNativeUrlChanged: pageTimer.restart()
    property Timer pageTimer: Timer {
        interval: 0
        onTriggered: controller.applyPageFocus()
    }
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
    function contentFocusTarget(target) {
        if (!target || target.nativeUrl === undefined || target.nativeUrl.length > 0
                || typeof target.nextItemInFocusChain !== "function") return target
        // WebEngine is a focus scope. Returning to its parent can leave the
        // document unfocused after native controls owned that scope.
        var next = target.nextItemInFocusChain(true)
        if (!focusTargetAvailable(next)) return target
        for (var parent = next; parent; parent = parent.parent) {
            if (parent === target) return next
        }
        return target
    }
    function applyPageFocus() {
        // Tab identity can change before the layout hides the old renderer.
        // Resolve the current page after that change, while keeping editors
        // and browser surfaces in charge of their own input.
        if (!followTabFocus || browserWindow.runtimeDisposing === true || browserWindow.runtimeReleased === true
                || browserWindow.browserKeyFocusActive === false
                || focusReturnStack.some(function(entry) { return entry.host === browserWindow })) return
        var view = activeView
        if (focusTargetAvailable(view) && focusPermitted(browserWindow, view)) contentFocusTarget(view).forceActiveFocus()
    }
    function captureOverlayFocus(hostWindow, fallbackTarget, previousTarget) {
        if (browserWindow.runtimeDisposing === true) return
        var host = hostWindow || browserWindow
        // A newer overlay owns this host. Discard a queued return from the
        // surface it replaced, while retaining returns for other windows.
        pendingFocusReturns = pendingFocusReturns.filter(function(entry) { return entry.host !== host })
        var target = arguments.length > 2 ? previousTarget : (host ? host.activeFocusItem : null)
        // A newly shown window can report its root container as focused.
        // Returning there reselects a hidden control rather than page content.
        var container = host ? host.contentItem : null
        while (target && container) {
            if (target === container) { target = null; break }
            container = container.parent
        }
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
            if (focusTargetAvailable(entry.target) && focusPermitted(entry.host, entry.target)) contentFocusTarget(entry.target).forceActiveFocus()
            else if (focusTargetAvailable(entry.fallback) && focusPermitted(entry.host, entry.fallback)) contentFocusTarget(entry.fallback).forceActiveFocus()
            else if (typeof entry.host.activeWebView === "function") {
                var view = entry.host.activeWebView()
                if (focusTargetAvailable(view) && focusPermitted(entry.host, view)) contentFocusTarget(view).forceActiveFocus()
            }
        }
    }
    function captureModeFocus() {
        if (modeFocusCaptured || browserWindow.runtimeDisposing === true
                || browserWindow.runtimeReleased === true) return
        var target = browserWindow.activeFocusItem
        if (!focusTargetAvailable(target)) target = browserWindow.activeWebView()
        modeFocusReturnTarget = target
        modeFocusCaptured = true
    }
    function restoreModeFocus() {
        pendingModeFocusTarget = modeFocusCaptured ? modeFocusReturnTarget : browserWindow.activeWebView()
        modeFocusReturnTarget = null
        modeFocusCaptured = false
        if (browserWindow.runtimeDisposing !== true && browserWindow.runtimeReleased !== true) modeTimer.restart()
    }
    function applyModeFocus() {
        var target = pendingModeFocusTarget
        pendingModeFocusTarget = null
        // A command can open a modal immediately before leaving command mode.
        if (browserWindow.runtimeDisposing === true || browserWindow.runtimeReleased === true
                || (browserWindow.modalInputScope && browserWindow.modalInputScope.activeSurface)
                || focusReturnStack.some(function(entry) { return entry.host === browserWindow })) return
        if (focusTargetAvailable(target)) contentFocusTarget(target).forceActiveFocus()
        else {
            var view = browserWindow.activeWebView()
            if (focusTargetAvailable(view)) view.forceActiveFocus()
        }
    }
}
