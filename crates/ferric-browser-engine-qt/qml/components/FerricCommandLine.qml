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
    property string feedbackText: ""
    property bool feedbackError: false
    property alias commandText: commandLine.text
    property alias cursorPosition: commandLine.cursorPosition
    signal completionUpdateRequested(string text, int cursorPosition)
    signal submitted(string text)
    signal escapeRequested()
    signal completionMoveRequested(int delta)
    signal completionSelectRequested(int index)
    signal historyMoveRequested(int delta, string current)
    signal inputEdited()

    function revealSelectedCompletion() {
        // Model replacement resets ListView's native current row even when
        // the bridge's selected index and candidate count stay unchanged.
        var selected = completionSelected >= 0 && completionSelected < completionList.count
            ? completionSelected : -1
        completionList.currentIndex = selected
        if (completionPopup.visible && selected >= 0 && completionList.height > 0) {
            completionList.forceLayout()
            completionList.positionViewAtIndex(selected, ListView.Contain)
        }
    }

    onCompletionSelectedChanged: Qt.callLater(revealSelectedCompletion)

    function focusInput() {
        commandLine.forceActiveFocus()
    }

    function clearInput() {
        commandLine.text = ""
        clearFeedback()
    }

    function selectAllInput() {
        commandLine.selectAll()
    }

    function recallHistory(text) {
        if (text === commandLine.text) return
        // A history choice is an editor operation. Assigning text clears
        // Qt's undo stack, including the draft typed before browsing history.
        commandLine.cursorPosition = 0
        commandLine.cursorPosition = commandLine.text.length
        commandLine.remove(0, commandLine.text.length)
        commandLine.insert(0, text)
        commandLine.cursorPosition = text.length
    }

    function showFeedback(text, isError) {
        feedbackText = String(text || "")
        feedbackError = !!isError
    }

    function clearFeedback() {
        feedbackText = ""
        feedbackError = false
    }

    function completionEdit(index) {
        var values = completionValues.length ? completionValues.split("\n") : []
        if (index < 0 || index >= values.length
                || completionStart < 0 || completionEnd < completionStart
                || completionEnd > commandLine.text.length) {
            return null
        }
        return {start: completionStart, end: completionEnd, value: values[index],
            text: commandLine.text.slice(0, completionStart)
                + values[index] + commandLine.text.slice(completionEnd),
            cursor: completionStart + values[index].length}
    }

    function commandWithCompletion(index) {
        var edit = completionEdit(index)
        return edit ? edit.text : commandLine.text
    }

    function applyCompletion(index) {
        // Text and cursor notifications synchronously refresh the bridge's
        // candidate snapshot. Capture the entire edit before changing either.
        var edit = completionEdit(index)
        if (!edit) return false
        if (edit.text !== commandLine.text) {
            // Moving to the token edge separates this replacement from the
            // text the user just typed in Qt's native undo history.
            commandLine.cursorPosition = edit.start === edit.end && edit.start > 0
                ? edit.start - 1 : edit.start
            commandLine.cursorPosition = edit.end
            commandLine.remove(edit.start, edit.end)
            commandLine.insert(edit.start, edit.value)
        }
        commandLine.cursorPosition = edit.cursor
        return true
    }

    function chooseCompletion(index) {
        // Applying an edit can synchronously destroy the clicked delegate.
        // Finish the gesture in the input owner's surviving context.
        completionSelectRequested(index)
        applyCompletion(index)
        focusInput()
    }

    function canInsertCompletion() {
        return completionPopup.visible && completionSelected >= 0
            && !commandLine.inputMethodComposing && commandLine.selectedText.length === 0
            && commandLine.cursorPosition === commandLine.text.length
            && completionEnd === commandLine.cursorPosition
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

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            hoverEnabled: true
            onWheel: function(wheel) { wheel.accepted = true }
        }

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: commandPrefix.implicitWidth + 16
                color: commandSurface.browserWindow.accentColor

                Label {
                    textFormat: Text.PlainText
                    id: commandPrefix
                    anchors.centerIn: parent
                    text: ":"
                    color: commandSurface.browserWindow.contrastText(parent.color)
                    font.bold: true
                    Accessible.ignored: true
                }
            }

            FerricTextField {
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
                    } else {
                        commandSurface.clearFeedback()
                    }
                }
                onTextChanged: {
                    commandSurface.clearFeedback()
                    commandSurface.inputEdited()
                    commandSurface.completionUpdateRequested(text, cursorPosition)
                }
                onCursorPositionChanged: commandSurface.completionUpdateRequested(text, cursorPosition)
                onAccepted: {
                    if (!commandLine.inputMethodComposing)
                        commandSurface.submitted(commandSurface.commandWithCompletion(commandSurface.completionSelected))
                }
                Keys.onPressed: function(event) {
                    // Composition keys belong to the input method and native editor.
                    if (commandLine.inputMethodComposing) {
                        event.accepted = false
                        return
                    }
                    var modifiers = event.modifiers & ~Qt.KeypadModifier
                    if (event.key === Qt.Key_Escape) {
                        commandSurface.escapeRequested()
                        event.accepted = true
                    } else if (modifiers === Qt.NoModifier
                               && (event.key === Qt.Key_Right || event.key === Qt.Key_Space)
                               && commandSurface.canInsertCompletion()) {
                        var inserted = commandSurface.applyCompletion(commandSurface.completionSelected)
                        // Let the original Space event add the next token's
                        // separator through the native editor after insertion.
                        event.accepted = inserted && event.key === Qt.Key_Right
                    } else if ((event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                               && (modifiers === Qt.NoModifier || modifiers === Qt.ShiftModifier)) {
                        commandSurface.completionMoveRequested(
                                    event.key === Qt.Key_Backtab
                                    || modifiers === Qt.ShiftModifier ? -1 : 1)
                        event.accepted = true
                    } else if (modifiers === Qt.ControlModifier && event.key === Qt.Key_N) {
                        commandSurface.completionMoveRequested(1)
                        event.accepted = true
                    } else if (modifiers === Qt.ControlModifier && event.key === Qt.Key_P) {
                        commandSurface.completionMoveRequested(-1)
                        event.accepted = true
                    } else if (modifiers === Qt.NoModifier
                               && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
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
        objectName: "commandCompletionPopup"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.top
        height: {
            var row = completionList.currentItem || completionList.itemAtIndex(0)
            var rowHeight = row ? row.height : Math.max(42, commandSurface.browserWindow.chromeRowHeight * 1.8)
            return Math.min(Math.max(0, commandSurface.parent.height - commandSurface.height),
                completionList.contentHeight + completionHelpBar.height + 2,
                Math.max(commandSurface.parent.height * 0.5, rowHeight + completionHelpBar.height + 2))
        }
        z: 1
        visible: commandSurface.commandVisible
                 && commandSurface.completionVisible
                 && commandSurface.feedbackText.length === 0
        color: commandSurface.browserWindow.panelColor
        opacity: 1.0
        border.color: commandSurface.browserWindow.borderColor
        border.width: 1
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "Command completion popup"
        Accessible.description: "Use Tab or Shift-Tab to choose a suggestion. At the end of the input, Right inserts it for editing and Space inserts it before the next word. Return runs the command."
        onVisibleChanged: Qt.callLater(commandSurface.revealSelectedCompletion)

        ListView {
            id: completionList
            objectName: "commandCompletionList"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: completionHelpBar.top
            anchors.margins: 1
            clip: true
            currentIndex: -1
            onModelChanged: Qt.callLater(commandSurface.revealSelectedCompletion)
            onCurrentItemChanged: Qt.callLater(commandSurface.revealSelectedCompletion)
            onCountChanged: Qt.callLater(commandSurface.revealSelectedCompletion)
            onHeightChanged: Qt.callLater(commandSurface.revealSelectedCompletion)
            onWidthChanged: Qt.callLater(commandSurface.revealSelectedCompletion)
            ScrollBar.vertical: ScrollBar { id: completionScrollBar; policy: ScrollBar.AsNeeded }
            Accessible.role: Accessible.List
            Accessible.name: "Command completion"
            model: commandSurface.completionText.length
                   ? commandSurface.completionText.split("\n") : []
            delegate: Rectangle {
                id: completionRow
                readonly property var parts: String(modelData).split("\t")
                width: Math.max(0, completionList.width
                    - (completionScrollBar.visible ? completionScrollBar.width : 0))
                height: Math.max(42, commandSurface.browserWindow.chromeRowHeight * 1.8,
                    completionTitle.implicitHeight
                    + (completionDetail.visible ? completionDetail.implicitHeight + 2 : 0) + 8)
                onHeightChanged: if (index === commandSurface.completionSelected)
                    Qt.callLater(commandSurface.revealSelectedCompletion)
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
                    objectName: "commandCompletionTitle"
                    font: commandSurface.browserWindow.font
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
                          ? completionRow.parts[1]
                          : modelData
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    Accessible.ignored: true
                }

                Text {
                    id: completionDetail
                    font: commandSurface.browserWindow.font
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: completionTitle.bottom
                    anchors.topMargin: 2
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    color: index === commandSurface.completionSelected
                           ? commandSurface.browserWindow.selectionTextColor
                           : commandSurface.browserWindow.readableTextColor(
                                 commandSurface.browserWindow.mutedTextColor,
                                 completionRow.color)
                    text: completionRow.parts.length >= 3
                          ? completionRow.parts[0] + (completionRow.parts.slice(2).join(" ").length
                            ? " · " + completionRow.parts.slice(2).join(" ") : "") : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    visible: text.length > 0
                    Accessible.ignored: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: commandSurface.chooseCompletion(index)
                }
            }
        }

        Rectangle {
            id: completionHelpBar
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 1
            height: completionHelp.implicitHeight + 8
            color: commandSurface.browserWindow.panelColor
            readonly property string helpText: "Tab / Shift+Tab choose · → insert · Space next word · Enter run"
            TextMetrics {
                id: completionHelpMetrics
                font: commandSurface.browserWindow.font
                text: completionHelpBar.helpText
            }
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                height: 1
                color: commandSurface.browserWindow.borderColor
            }
            Label {
                id: completionHelp
                objectName: "commandCompletionHelp"
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.margins: 8
                font: commandSurface.browserWindow.font
                color: commandSurface.browserWindow.mutedTextColor
                text: completionHelpMetrics.width <= width ? completionHelpBar.helpText : "→ insert · ↵ run"
                textFormat: Text.PlainText
                elide: Text.ElideRight
                Accessible.name: completionPopup.Accessible.description
            }
        }
    }

    Rectangle {
        id: feedbackPopup
        objectName: "commandFeedbackPopup"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.top
        height: feedbackLabel.implicitHeight + 16
        z: 2
        visible: commandSurface.commandVisible
                 && commandSurface.feedbackText.length > 0
        color: commandSurface.browserWindow.panelColor
        border.color: commandSurface.feedbackError
                      ? commandSurface.browserWindow.errorColor
                      : commandSurface.browserWindow.warningColor
        border.width: 1
        Accessible.role: Accessible.StaticText
        Accessible.name: feedbackLabel.text

        Label {
            id: feedbackLabel
            anchors.fill: parent
            anchors.margins: 8
            color: commandSurface.feedbackError
                   ? commandSurface.browserWindow.errorColor
                   : commandSurface.browserWindow.warningColor
            text: commandSurface.feedbackText
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }
    }
}
