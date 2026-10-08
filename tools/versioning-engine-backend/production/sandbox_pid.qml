import QtQuick
import QtQuick.Window
import QtWebEngine

// Disposable, offscreen-only renderer witness for the Linux development tuple.
Window {
    id: root
    width: 320
    height: 240
    visible: true

    WebEngineProfilePrototype { id: profilePrototype; storageName: "" }
    property var privateProfile: null
    property bool reported: false

    function reportRenderer() {
        if (page.renderProcessPid > 0 && !reported) {
            reported = true
            console.log("FERRIC_OWNER_RENDERER_PID=" + page.renderProcessPid)
        }
    }

    WebEngineView {
        id: page
        anchors.fill: parent
        profile: root.privateProfile
        onLoadingChanged: function(request) {
            if (request.status === WebEngineView.LoadSucceededStatus)
                root.reportRenderer()
        }
        onRenderProcessPidChanged: root.reportRenderer()
    }

    Component.onCompleted: {
        privateProfile = profilePrototype.instance()
        page.url = "data:text/html,<html><body>pinned renderer</body></html>"
    }

    Timer {
        interval: 20000
        running: true
        onTriggered: Qt.quit()
    }
}
