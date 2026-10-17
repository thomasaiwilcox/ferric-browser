import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Button {
    id: action

    required property var browserWindow
    property string keyHint: ""
    property string actionLabel: ""
    property bool selected: false
    property bool destructive: false
    property bool safe: false

    signal selectionRequested()

    text: actionLabel
    hoverEnabled: true
    Accessible.name: actionLabel + (keyHint.length > 0 ? ". " + keyHint + "." : "")
    onActiveFocusChanged: if (activeFocus) selectionRequested()
    onHoveredChanged: if (hovered) selectionRequested()

    contentItem: RowLayout {
        spacing: 10

        Label {
            textFormat: Text.PlainText
            text: action.selected ? "›" : " "
            color: action.destructive
                   ? action.browserWindow.errorColor
                   : action.browserWindow.accentColor
            font.bold: true
        }
        Label {
            textFormat: Text.PlainText
            text: action.keyHint
            color: action.browserWindow.mutedTextColor
            font.family: action.browserWindow.font.family
            // Natural text width grows with the font; the minimum gutter
            // stays in logical units instead of being scaled a second time.
            Layout.preferredWidth: Math.max(54, implicitWidth)
        }
        Label {
            objectName: "commandActionLabel"
            Layout.fillWidth: true
            text: action.actionLabel
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: action.selected
                   ? action.browserWindow.selectionTextColor
                   : action.browserWindow.primaryTextColor
            font.bold: action.selected
        }
    }

    background: Rectangle {
        color: action.selected
               ? action.browserWindow.selectionColor
               : (action.hovered ? action.browserWindow.surfaceColor : "transparent")
        border.color: action.selected
                      ? (action.destructive
                         ? action.browserWindow.errorColor
                         : action.browserWindow.accentColor)
                      : action.browserWindow.borderColor
        border.width: 1
        radius: 4
    }
}
