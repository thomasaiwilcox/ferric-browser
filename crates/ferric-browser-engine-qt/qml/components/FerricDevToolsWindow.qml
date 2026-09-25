import QtQuick
import QtQuick.Controls
import QtWebEngine

// Detached developer tools are presentation infrastructure. Close handling is
// emitted to the composition root, which owns window and browser lifecycle.
ApplicationWindow {
    id: devToolsWindow

    required property var browserWindow
    property var inspectView: null
    property var ownerWindow: null
    property alias inspectorView: detachedDevToolsView

    signal ownerWindowClosed()
    signal inspectedViewClosed()

    width: 980
    height: 620
    visible: false
    title: "Ferric Browser · DevTools"
    color: ownerWindow ? ownerWindow.backgroundColor : "#1e1e2e"
    palette: browserWindow.palette

    FerricWebEngineSurfaceRecovery {
        hostWindow: devToolsWindow
        enabled: browserWindow.nativeWayland && !browserWindow.softwareRendering
        views: [detachedDevToolsView]
    }

    onClosing: {
        if (ownerWindow) {
            ownerWindowClosed()
        } else if (inspectView) {
            inspectedViewClosed()
        }
    }

    WebEngineView {
        id: detachedDevToolsView
        anchors.fill: parent
        profile: devToolsWindow.inspectView ? devToolsWindow.inspectView.profile : null
        inspectedView: devToolsWindow.inspectView
        Accessible.name: "Detached developer tools"
    }
}
