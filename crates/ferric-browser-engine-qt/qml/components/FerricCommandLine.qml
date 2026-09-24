import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presentation for command editing and completion. The composition root owns
// command execution and forwards the typed completion requests to the bridge.
Item {
    id: commandSurface
    required property var browserWindow
    required property bool commandVisible
    required property bool completionVisible
    required property string completionText
    required property int completionSelected
    property alias commandText: commandLine.text
    property alias cursorPosition: commandLine.cursorPosition
    signal completionUpdateRequested(string text, int cursorPosition)
    signal submitted(string text)
    signal escapeRequested()
    signal completionMoveRequested(int delta)
    signal completionSelectRequested(int index)

    function focusInput() {
        commandLine.forceActiveFocus()
    }

    function clearInput() {
        commandLine.text = ""
    }

    function selectAllInput() {
        commandLine.selectAll()
    }

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: browserWindow.inputBarHeight
    z: 20
    visible: commandVisible

    Rectangle {
        id: commandBar
        anchors.fill: parent
        color: commandSurface.browserWindow.surfaceColor
        opacity: commandSurface.browserWindow.chromeOpacity

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: commandPrefix.implicitWidth + 16
                color: commandSurface.browserWindow.accentColor

                Label {
                    id: commandPrefix
                    anchors.centerIn: parent
                    text: ":"
                    color: commandSurface.browserWindow.contrastText(parent.color)
                    font.bold: true
                    Accessible.ignored: true
                }
            }

            TextField {
                id: commandLine
                Layout.fillWidth: true
                Layout.fillHeight: true
                leftPadding: 8
                rightPadding: 8
                topPadding: 0
                bottomPadding: 0
                color: commandSurface.browserWindow.primaryTextColor
                selectionColor: commandSurface.browserWindow.selectionColor
                selectedTextColor: commandSurface.browserWindow.selectionTextColor
                placeholderText: "command"
                placeholderTextColor: commandSurface.browserWindow.mutedTextColor
                background: Rectangle { color: "transparent" }
                Accessible.name: "Command line"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                focus: commandSurface.commandVisible

                onVisibleChanged: {
                    if (visible) {
                        forceActiveFocus()
                        commandSurface.completionUpdateRequested(text, cursorPosition)
                    }
                }
                onTextChanged: commandSurface.completionUpdateRequested(text, cursorPosition)
                onCursorPositionChanged: commandSurface.completionUpdateRequested(text, cursorPosition)
                onAccepted: commandSurface.submitted(text)
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        commandSurface.escapeRequested()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Tab) {
                        commandSurface.completionMoveRequested(
                                    event.modifiers & Qt.ShiftModifier ? -1 : 1)
                        event.accepted = true
                    } else if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_N) {
                        commandSurface.completionMoveRequested(1)
                        event.accepted = true
                    } else if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_P) {
                        commandSurface.completionMoveRequested(-1)
                        event.accepted = true
                    }
                }
            }
        }
    }

    Rectangle {
        id: completionPopup
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.top
        height: Math.min(220, completionList.contentHeight + 8)
        z: 1
        visible: commandSurface.commandVisible && commandSurface.completionVisible
        color: commandSurface.browserWindow.panelColor
        opacity: 1.0
        border.color: commandSurface.browserWindow.borderColor
        border.width: 1
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "Command completion popup"
        Accessible.description: "Use Tab or Shift-Tab to move through command completion values"

        ListView {
            id: completionList
            anchors.fill: parent
            anchors.margins: 1
            clip: true
            Accessible.role: Accessible.List
            Accessible.name: "Command completion"
            model: commandSurface.completionText.length
                   ? commandSurface.completionText.split("\n") : []
            delegate: Rectangle {
                id: completionRow
                width: completionList.width
                height: commandSurface.browserWindow.chromeRowHeight
                color: index === commandSurface.completionSelected
                       ? commandSurface.browserWindow.selectionColor
                       : commandSurface.browserWindow.panelColor
                opacity: 1.0
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData
                Accessible.selected: index === commandSurface.completionSelected
                Accessible.focusable: false

                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    verticalAlignment: Text.AlignVCenter
                    color: index === commandSurface.completionSelected
                           ? commandSurface.browserWindow.selectionTextColor
                           : commandSurface.browserWindow.readableTextColor(
                                 commandSurface.browserWindow.primaryTextColor,
                                 completionRow.color)
                    text: modelData
                    elide: Text.ElideRight
                    Accessible.ignored: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        commandSurface.completionSelectRequested(index)
                        commandSurface.focusInput()
                    }
                }
            }
        }
    }
}
