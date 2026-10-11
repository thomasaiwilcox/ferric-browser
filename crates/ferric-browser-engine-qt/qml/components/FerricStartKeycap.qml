import QtQuick
import QtQuick.Controls

Rectangle {
    id: keycap
    required property var browserWindow
    property string text: ""
    property real maximumWidth: 240
    implicitWidth: Math.min(maximumWidth, Math.max(26, label.implicitWidth + 16))
    implicitHeight: Math.max(25, label.implicitHeight + 8)
    radius: 6
    color: browserWindow.backgroundColor
    border.color: browserWindow.borderColor
    border.width: 1
    Accessible.role: Accessible.StaticText
    Accessible.name: "Keyboard shortcut " + text
    Label {
        id: label
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        text: keycap.text
        color: keycap.browserWindow.primaryTextColor
        font.family: "monospace"
        font.pointSize: Math.max(9, (keycap.browserWindow.chromeFontPointSize || 10) - 1)
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        textFormat: Text.PlainText
    }
}
