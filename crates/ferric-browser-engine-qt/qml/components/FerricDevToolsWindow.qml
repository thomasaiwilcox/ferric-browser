import QtQuick
import QtQuick.Controls
import QtWebEngine

// Detached developer tools are presentation infrastructure. Close handling is
// emitted to the composition root, which owns window and browser lifecycle.
ApplicationWindow {
    id: devToolsWindow

    required property var browserWindow
    property var inspectView: null
    required property var inspectionProfile
    property var ownerWindow: null
    property alias inspectorView: detachedDevToolsView

    signal ownerWindowClosed()
    signal inspectedViewClosed()

    width: 980
    height: 620
    visible: false
    title: "Ferric Browser · DevTools"
    color: ownerWindow ? ownerWindow.color : browserWindow.backgroundColor
    palette: ownerWindow ? ownerWindow.palette : browserWindow.palette
    font: ownerWindow ? ownerWindow.font : browserWindow.font

    FerricWebEngineSurfaceRecovery {
        hostWindow: devToolsWindow
        enabled: browserWindow.nativeWayland && !browserWindow.softwareRendering
        views: [detachedDevToolsView]
    }

    onClosing: {
        browserWindow.profileResourceRegistry.releaseView(detachedDevToolsView)
        if (ownerWindow) {
            ownerWindowClosed()
        } else if (inspectView) {
            inspectedViewClosed()
        }
    }

    WebEngineView {
        id: detachedDevToolsView
        property var viewHost: devToolsWindow.inspectView ? devToolsWindow.inspectView.viewHost : devToolsWindow.ownerWindow
        property var viewUi: devToolsWindow.inspectView ? devToolsWindow.inspectView.viewUi : null
        anchors.fill: parent
        profile: devToolsWindow.inspectionProfile
        inspectedView: devToolsWindow.inspectView
        Accessible.name: "Detached developer tools"
        Component.onCompleted: browserWindow.profileResourceRegistry.trackView(detachedDevToolsView,viewHost,viewUi)
    }
    Component.onDestruction: {
        if (browserWindow && !browserWindow.runtimeDisposing)
            browserWindow.profileResourceRegistry.releaseView(detachedDevToolsView)
    }
}
