import QtQuick
import QtQuick.Controls

TextField {
    id: field
    required property var browserWindow
    implicitHeight: Math.max(42, contentHeight + topPadding + bottomPadding)
    leftPadding: 12
    rightPadding: 12
    topPadding: 10
    bottomPadding: 10
    font.pointSize: browserWindow.chromeFontPointSize || 10
    color: browserWindow.primaryTextColor
    placeholderTextColor: browserWindow.mutedTextColor
    selectionColor: browserWindow.accentColor
    selectedTextColor: browserWindow.backgroundColor
    background: Rectangle {
        radius: 9
        color: field.browserWindow.backgroundColor
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? field.browserWindow.accentColor : field.browserWindow.borderColor
    }
}
