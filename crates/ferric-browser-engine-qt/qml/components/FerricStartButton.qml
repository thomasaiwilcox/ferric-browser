import QtQuick
import QtQuick.Controls

Button {
    id: control
    required property var browserWindow
    property bool prominent: false
    property bool quiet: false
    property bool wrapText: false
    readonly property color accent: browserWindow.accentColor
    readonly property color surface: browserWindow.surfaceColor
    implicitHeight: Math.max(38, contentItem.implicitHeight + topPadding + bottomPadding)
    leftPadding: 14
    rightPadding: 14
    topPadding: 8
    bottomPadding: 8
    hoverEnabled: true
    Accessible.name: text
    Keys.onShortcutOverride: function(event) {
        if (event.modifiers === Qt.NoModifier && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) event.accepted = true
    }
    function activateEnter(event) {
        event.accepted = event.modifiers === Qt.NoModifier
        if (event.accepted && !event.isAutoRepeat && control.enabled) control.click()
    }
    Keys.onReturnPressed: function(event) { control.activateEnter(event) }
    Keys.onEnterPressed: function(event) { control.activateEnter(event) }
    Keys.onReleased: function(event) {
        if (event.modifiers === Qt.NoModifier && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) event.accepted = true
    }
    contentItem: Label {
        text: control.text
        font.pointSize: control.browserWindow.chromeFontPointSize || 10
        font.weight: control.prominent || control.checked ? Font.DemiBold : Font.Normal
        color: control.prominent || control.checked ? control.accent : control.browserWindow.primaryTextColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        opacity: control.enabled ? 1 : 0.45
        textFormat: Text.PlainText
        wrapMode: control.wrapText ? Text.Wrap : Text.NoWrap
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
