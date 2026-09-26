import QtQuick
import QtWebEngine

// Recovers QtWebEngine's imported compositor surface after an accelerated
// native-Wayland window is reactivated. A QQuickWindow repaint alone does not
// make Chromium reattach the stale surface, so the active WebEngine items are
// given a synchronous hide/show notification after the first returned frame.
// Completing both notifications in one signal turn prevents Qt Quick from
// presenting the detached state as an intermediate black frame.
Item {
    id: controller

    required property var hostWindow
    property var frameSource: hostWindow
    property var views: []
    readonly property int recoveryCount: state.completedCount

    signal recoveryStarted()
    signal recoveryCompleted()

    visible: false
    enabled: false
    width: 0
    height: 0

    QtObject {
        id: state
        property bool hasPresentedFrame: false
        property bool inactiveSincePresentation: false
        property bool armed: false
        property int completedCount: 0
    }

    function requestFrame() {
        try {
            if (controller.hostWindow
                    && typeof controller.hostWindow.requestUpdate === "function") {
                controller.hostWindow.requestUpdate()
            }
        } catch (error) {
            // The host may have been destroyed during window teardown.
        }
    }

    function eligibleViews() {
        var eligible = []
        var candidates = controller.views || []
        for (var index = 0; index < candidates.length; ++index) {
            var view = candidates[index]
            try {
                if (!view || !view.visible
                        || view.lifecycleState !== WebEngineView.LifecycleState.Active
                        || eligible.indexOf(view) >= 0) {
                    continue
                }
                eligible.push(view)
            } catch (error) {
                // A view can disappear while a tab, popup, or DevTools closes.
            }
        }
        return eligible
    }

    function cancelRecovery() {
        state.armed = false
    }

    function armRecovery() {
        state.armed = true
        controller.requestFrame()
    }

    function recoverAfterReturnedFrame() {
        if (!controller.enabled || !state.armed || !controller.hostWindow
                || !controller.hostWindow.active) {
            return
        }
        state.armed = false
        var views = controller.eligibleViews()
        if (views.length === 0) {
            return
        }
        for (var index = 0; index < views.length; ++index) {
            try {
                views[index].visible = false
            } catch (error) {
                // A view can disappear during tab or window teardown.
            }
        }
        controller.recoveryStarted()
        for (var restoreIndex = 0; restoreIndex < views.length; ++restoreIndex) {
            try {
                views[restoreIndex].visible = true
            } catch (error) {
                // Deleted QObjects do not need restoring.
            }
        }
        state.completedCount += 1
        controller.recoveryCompleted()
        controller.requestFrame()
    }

    onEnabledChanged: {
        if (!enabled) {
            state.inactiveSincePresentation = false
            cancelRecovery()
        }
    }

    Connections {
        target: controller.hostWindow
        ignoreUnknownSignals: true

        function onActiveChanged() {
            if (!controller.enabled || !controller.hostWindow) {
                controller.cancelRecovery()
                return
            }
            if (!controller.hostWindow.active) {
                state.inactiveSincePresentation = state.hasPresentedFrame
                controller.cancelRecovery()
                return
            }
            if (state.inactiveSincePresentation) {
                state.inactiveSincePresentation = false
                controller.armRecovery()
            }
        }
    }

    Connections {
        target: controller.frameSource
        ignoreUnknownSignals: true

        function onFrameSwapped() {
            if (!state.hasPresentedFrame) {
                state.hasPresentedFrame = true
                return
            }
            controller.recoverAfterReturnedFrame()
        }
    }

    Component.onDestruction: cancelRecovery()
}
