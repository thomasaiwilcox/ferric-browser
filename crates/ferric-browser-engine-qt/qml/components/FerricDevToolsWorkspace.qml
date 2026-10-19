import QtQuick

// Inspector resources and focus belong to the window that requested them.
QtObject {
    id: workspace
    required property var browserWindow
    required property var hostWindow
    required property var browserUi
    required property var activeView
    required property var windowFactory
    property var attachedInspector: null
    property var sourceView: null
    property var sourceProfile: null
    property string sourceWindowId: ""
    property var detachedWindow: null
    property bool visible: false
    property bool detached: false
    readonly property bool sourceCurrent: !!sourceView && !!hostWindow && !!browserUi
        && hostWindow.runtimeReleased !== true && hostWindow.runtimeDisposing !== true
        && !browserUi.profile_bootstrap_pending
        && String(browserUi.core_window_id) === sourceWindowId
        && sourceView.viewHost === hostWindow && sourceView.viewUi === browserUi
        && sourceView.viewRetired !== true && sourceView.profile === sourceProfile
        && sourceView.ownsTab()
    readonly property bool attachedVisible: visible && !detached && sourceCurrent
                                           && activeView === sourceView
    readonly property bool inputActive: {
        if (!attachedVisible || !attachedInspector) return false
        var focus = hostWindow.activeFocusItem
        while (focus && focus !== attachedInspector) focus = focus.parent
        return focus === attachedInspector
    }
    readonly property var inspector: detached && detachedWindow
        ? detachedWindow.inspectorView : attachedInspector
    property QtObject focusReturn: FerricFocusOverlayController { browserWindow: workspace.hostWindow }
    property Timer retireTimer: Timer {
        interval: 0
        onTriggered: if (workspace.visible && !workspace.sourceCurrent) workspace.close()
    }
    property Timer focusTimer: Timer {
        interval: 0
        onTriggered: {
            if (!workspace.visible || !workspace.sourceCurrent) return
            var scope=workspace.hostWindow.modalInputScope
            if (scope && scope.activeSurface) return
            if (workspace.detachedWindow) workspace.detachedWindow.requestActivate()
            if (workspace.inspector) workspace.inspector.forceActiveFocus()
        }
    }
    property Connections windowConnections: Connections {
        target: workspace.detachedWindow
        function onClosing() { workspace.detachedClosed() }
    }
    onSourceCurrentChanged: if (visible && !sourceCurrent) retireTimer.restart()
    onAttachedInspectorChanged: if (attachedInspector && visible) focusTimer.restart()
    Component.onDestruction: close()

    function inspectorFor(view) {
        return visible && sourceCurrent && view === sourceView ? inspector : null
    }
    function close() {
        var returning=visible && sourceCurrent
        var previous=detachedWindow
        detachedWindow=null
        visible=false;detached=false
        sourceView=null;sourceProfile=null;sourceWindowId=""
        retireTimer.stop();focusTimer.stop()
        if (returning) focusReturn.restoreOverlayFocus(hostWindow)
        else focusReturn.discardHost(hostWindow)
        if (previous) { previous.close();previous.destroy() }
    }
    function detachedClosed() {
        var previous=detachedWindow
        if (!previous) return
        var notify=sourceCurrent
        detachedWindow=null
        close()
        previous.destroy()
        if (notify) browserUi.status_text="Developer tools closed"
    }
    function toggle(detach, targetView) {
        var requestedView = targetView || activeView
        if (browserUi.profile_bootstrap_pending) {
            browserUi.status_text="This window is still starting. Try again in a moment."
            return false
        }
        if (!requestedView || hostWindow.runtimeReleased === true || !requestedView.ownsTab()) {
            browserUi.status_text="The page is no longer available. Select a tab and try again."
            return false
        }
        if (visible && detached === detach && sourceView === requestedView) {
            close()
            browserUi.status_text="Developer tools closed"
            return true
        }
        close()
        sourceView=requestedView
        sourceProfile=sourceView.profile
        sourceWindowId=String(browserUi.core_window_id)
        focusReturn.captureOverlayFocus(hostWindow,sourceView)
        detached=detach
        if (detach) {
            detachedWindow=windowFactory.createObject(hostWindow,{
                browserWindow:browserWindow,ownerWindow:hostWindow,inspectView:sourceView,
                inspectionProfile:sourceProfile})
            if (!detachedWindow) {
                close()
                browserUi.status_text="Couldn't open developer tools. Try again."
                return false
            }
        }
        visible=true
        if (detachedWindow) detachedWindow.show()
        focusTimer.restart()
        browserUi.status_text=detach ? "Developer tools opened in a separate window" : "Developer tools opened"
        return true
    }
}
