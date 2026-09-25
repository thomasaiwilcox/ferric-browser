import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presentation for command editing and completion. The composition root owns
// command execution and forwards the typed completion requests to the bridge.
Item {
    id: commandSurface
    objectName: "commandSurface"
    required property var browserWindow
    required property bool commandVisible
    required property bool completionVisible
    required property string completionText
    required property string completionValues
    required property int completionStart
    required property int completionEnd
    required property int completionSelected
    property alias commandText: commandLine.text
    property alias cursorPosition: commandLine.cursorPosition
    signal completionUpdateRequested(string text, int cursorPosition)
    signal submitted(string text)
    signal escapeRequested()
    signal completionMoveRequested(int delta)
    signal completionSelectRequested(int index)
    signal historyMoveRequested(int delta, string current)

    function focusInput() {
        commandLine.forceActiveFocus()
    }

    function clearInput() {
        commandLine.text = ""
    }

    function selectAllInput() {
        commandLine.selectAll()
    }

    function commandWithCompletion(index) {
        var values = completionValues.length ? completionValues.split("\n") : []
        if (index < 0 || index >= values.length
                || completionStart < 0 || completionEnd < completionStart
                || completionEnd > commandLine.text.length) {
            return commandLine.text
        }
        return commandLine.text.slice(0, completionStart)
            + values[index] + commandLine.text.slice(completionEnd)
    }

    function applyCompletion(index) {
        var text = commandWithCompletion(index)
        if (text !== commandLine.text) {
            commandLine.text = text
            commandLine.cursorPosition = completionStart
                + completionValues.split("\n")[index].length
        }
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
                objectName: "commandLineInput"
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
                onAccepted: commandSurface.submitted(
                    commandSurface.commandWithCompletion(commandSurface.completionSelected))
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
                    } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                        commandSurface.historyMoveRequested(
                            event.key === Qt.Key_Up ? -1 : 1, text)
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
        height: Math.min(commandSurface.parent.height * 0.5,
                         completionList.contentHeight + 8)
        z: 1
        visible: commandSurface.commandVisible && commandSurface.completionVisible
        color: commandSurface.browserWindow.panelColor
        opacity: 1.0
        border.color: commandSurface.browserWindow.borderColor
        border.width: 1
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "Command completion popup"
        Accessible.description: "Use Tab or Shift-Tab to choose a suggestion, then Return to use it"

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
                readonly property var parts: String(modelData).split("\t")
                width: completionList.width
                height: Math.max(42, commandSurface.browserWindow.chromeRowHeight * 1.8)
                color: index === commandSurface.completionSelected
                       ? commandSurface.browserWindow.selectionColor
                       : commandSurface.browserWindow.panelColor
                opacity: 1.0
                Accessible.role: Accessible.ListItem
                Accessible.name: completionRow.parts.length >= 3
                                 ? completionRow.parts[0] + ": "
                                   + completionRow.parts[1] + ", "
                                   + completionRow.parts.slice(2).join(" ")
                                 : modelData
                Accessible.selected: index === commandSurface.completionSelected
                Accessible.focusable: false

                Text {
                    id: completionTitle
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    anchors.topMargin: 4
                    color: index === commandSurface.completionSelected
                           ? commandSurface.browserWindow.selectionTextColor
                           : commandSurface.browserWindow.readableTextColor(
                                 commandSurface.browserWindow.primaryTextColor,
                                 completionRow.color)
                    text: completionRow.parts.length >= 3
                          ? "[" + completionRow.parts[0] + "] " + completionRow.parts[1]
                          : modelData
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    Accessible.ignored: true
                }

                Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: completionTitle.bottom
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    color: index === commandSurface.completionSelected
                           ? commandSurface.browserWindow.selectionTextColor
                           : commandSurface.browserWindow.readableTextColor(
                                 commandSurface.browserWindow.mutedTextColor,
                                 completionRow.color)
                    text: completionRow.parts.length >= 3
                          ? completionRow.parts.slice(2).join(" ") : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    visible: text.length > 0
                    Accessible.ignored: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        commandSurface.completionSelectRequested(index)
                        commandSurface.applyCompletion(index)
                        commandSurface.focusInput()
                    }
                }
            }
        }
    }
}
