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
    readonly property real scale: browserWindow.chromeScale === undefined
                                  ? 1 : browserWindow.chromeScale

    signal selectionRequested()

    text: actionLabel
    hoverEnabled: true
    Accessible.name: actionLabel + (keyHint.length > 0 ? ". " + keyHint + "." : "")
    onActiveFocusChanged: if (activeFocus) selectionRequested()
    onHoveredChanged: if (hovered) selectionRequested()

    contentItem: RowLayout {
        spacing: 10 * action.scale

        Label {
            text: action.selected ? "›" : " "
            color: action.destructive
                   ? action.browserWindow.errorColor
                   : action.browserWindow.accentColor
            font.bold: true
        }
        Label {
            text: action.keyHint
            color: action.browserWindow.mutedTextColor
            font.family: action.browserWindow.font.family
            Layout.preferredWidth: Math.max(54 * action.scale, implicitWidth)
        }
        Label {
            Layout.fillWidth: true
            text: action.actionLabel
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
        radius: 4 * action.scale
    }
}
