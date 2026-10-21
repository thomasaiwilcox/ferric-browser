import QtQuick
import QtQuick.Controls

Button {
    id: control
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
    width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
    contentItem: Label {
        text: control.text
        font: control.font
        color: control.palette.buttonText
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
}
