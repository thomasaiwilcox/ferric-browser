import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Shows runtime-provided capture state. Stopping or dismissing a capture is
// emitted as an intent for the composition root to dispatch.
Rectangle {
    id: indicator

    required property var browserWindow
    property var hostWindow: null

    signal dismissRequested(string sessionId)
    signal stopRequested(string sessionId)

    width: Math.min(520 * browserWindow.chromeScale,
                    hostWindow ? hostWindow.width - 32 : 488)
    height: Math.min(90 * Math.max(1, browserWindow.chromeScale)
                     + (browserWindow.captureSessions.length * 42),
                     hostWindow ? hostWindow.height - 32 : 560)
    anchors.top: parent.top
    anchors.right: parent.right
    anchors.topMargin: 8
    anchors.rightMargin: 8
    z: 120
    visible: hostWindow !== null && browserWindow.captureSessions.some(function(session) {
        return session && session.host === hostWindow
    })
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    border.width: 2
    radius: 4
    // Persistent capture state is an indicator, not an interaction-blocking
    // modal. It must not steal keyboard focus from the active page.
    Accessible.role: Accessible.StatusBar
    Accessible.name: "Active capture indicator"
    focus: false

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 4

        Label {
            Layout.fillWidth: true
            text: "Capture indicator"
            color: indicator.browserWindow.warningColor
            font.bold: true
            Accessible.name: "Capture indicator title"
        }

        ListView {
            id: captureIndicatorList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: indicator.hostWindow
                   ? indicator.browserWindow.captureSessions.filter(function(session) {
                       return session && session.host === indicator.hostWindow
                   }) : []
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: modelData.origin + " · "
                          + (modelData.status === "active"
                             ? "capture active"
                             : modelData.status === "stop-requested"
                               ? "stop requested"
                               : "ended by navigation")
                    color: indicator.browserWindow.primaryTextColor
                    elide: Text.ElideMiddle
                    Accessible.name: "Capture origin and state"
                }

                Button {
                    text: modelData.status === "ended-by-navigation"
                          ? "Dismiss" : "Stop"
                    Accessible.name: text + " capture for " + modelData.origin
                    onClicked: {
                        if (modelData.status === "ended-by-navigation") {
                            indicator.dismissRequested(modelData.id)
                        } else {
                            indicator.stopRequested(modelData.id)
                        }
                    }
                }
            }
        }
    }
}
