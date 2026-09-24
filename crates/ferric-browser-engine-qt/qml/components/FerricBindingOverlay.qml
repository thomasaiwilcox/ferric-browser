import QtQuick
import QtQuick.Controls

// Pure presentation for the current keyboard-binding continuation.
Rectangle {
    required property var browserWindow
    required property var browserUi

    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: browserWindow.statusBarHeight + 8
    width: Math.min(620, parent.width - 40)
    height: Math.max(34, label.implicitHeight + 14)
    z: 30
    visible: browserWindow.bindingOverlayVisible
    enabled: false
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1
    radius: 3

    Label {
        id: label
        anchors.fill: parent
        anchors.margins: 7
        text: browserUi.binding_overlay
        color: browserWindow.primaryTextColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
        Accessible.name: "Keyboard binding continuation"
        Accessible.role: Accessible.StatusBar
    }
}
