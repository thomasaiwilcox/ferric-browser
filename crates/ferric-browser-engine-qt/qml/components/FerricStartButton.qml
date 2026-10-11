import QtQuick
import QtQuick.Controls

Button {
    id: control
    required property var browserWindow
    property bool prominent: false
    property bool quiet: false
    readonly property color accent: browserWindow.accentColor
    readonly property color surface: browserWindow.surfaceColor
    implicitHeight: Math.max(38, contentItem.implicitHeight + topPadding + bottomPadding)
    leftPadding: 14
    rightPadding: 14
    topPadding: 8
    bottomPadding: 8
    hoverEnabled: true
    Accessible.name: text
    contentItem: Label {
        text: control.text
        font.pointSize: control.browserWindow.chromeFontPointSize || 10
        font.weight: control.prominent || control.checked ? Font.DemiBold : Font.Normal
        color: control.prominent || control.checked ? control.accent : control.browserWindow.primaryTextColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        opacity: control.enabled ? 1 : 0.45
        textFormat: Text.PlainText
    }
    background: Rectangle {
        radius: 9
        color: control.prominent || control.checked
               ? Qt.tint(control.surface, Qt.rgba(control.accent.r, control.accent.g, control.accent.b, 0.12))
               : control.hovered || control.down ? control.surface
               : control.quiet ? "transparent" : control.browserWindow.backgroundColor
        border.width: control.activeFocus ? 2 : control.quiet || control.checked ? 0 : 1
        border.color: control.activeFocus ? control.accent : control.browserWindow.borderColor
    }
}
