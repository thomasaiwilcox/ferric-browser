import QtQuick

// Owns focus capture for transient browser-owned overlays.  Surfaces ask the
// composition root to capture or restore focus; they never retain page items.
QtObject {
    id: controller

    required property var browserWindow
    property var focusReturnStack: []
    property var modeFocusReturnTarget: null
    property bool modeFocusCaptured: false

    function focusTargetAvailable(target) {
        return !!target && !!target.forceActiveFocus
                && target.visible !== false && target.enabled !== false
    }

    function captureOverlayFocus(hostWindow, fallbackTarget) {
        var host = hostWindow || controller.browserWindow
        var target = null
        try {
            target = host.activeFocusItem
        } catch (error) {
            target = null
        }
        if (!controller.focusTargetAvailable(target)) {
            target = fallbackTarget || null
        }
        var stack = (controller.focusReturnStack || []).slice(0)
        stack.push({ host: host, target: target, fallback: fallbackTarget || null })
        controller.focusReturnStack = stack
    }

    function restoreOverlayFocus() {
        var stack = (controller.focusReturnStack || []).slice(0)
        var entry = stack.length > 0 ? stack.pop() : null
        controller.focusReturnStack = stack
        if (!entry) {
            return
        }
        Qt.callLater(function() {
            if (controller.focusTargetAvailable(entry.target)) {
                entry.target.forceActiveFocus()
            } else if (controller.focusTargetAvailable(entry.fallback)) {
                entry.fallback.forceActiveFocus()
            } else if (entry.host === controller.browserWindow) {
                var view = controller.browserWindow.activeWebView()
                if (controller.focusTargetAvailable(view)) {
                    view.forceActiveFocus()
                }
            }
        })
    }

    function captureModeFocus() {
        if (controller.modeFocusCaptured) {
            return
        }
        var target = controller.browserWindow.activeFocusItem
        if (!controller.focusTargetAvailable(target)) {
            target = controller.browserWindow.activeWebView()
        }
        controller.modeFocusReturnTarget = target
        controller.modeFocusCaptured = true
    }

    function restoreModeFocus() {
        var target = controller.modeFocusCaptured
                ? controller.modeFocusReturnTarget
                : controller.browserWindow.activeWebView()
        controller.modeFocusReturnTarget = null
        controller.modeFocusCaptured = false
        Qt.callLater(function() {
            // A command may open an overlay immediately before leaving command
            // mode.  In that case the overlay owns focus until its stack entry
            // is restored; do not let the deferred mode restore steal focus
            // back for the page.
            if ((controller.focusReturnStack || []).length > 0) {
                return
            }
            if (controller.focusTargetAvailable(target)) {
                target.forceActiveFocus()
            } else {
                var view = controller.browserWindow.activeWebView()
                if (controller.focusTargetAvailable(view)) {
                    view.forceActiveFocus()
                }
            }
        })
    }
}
