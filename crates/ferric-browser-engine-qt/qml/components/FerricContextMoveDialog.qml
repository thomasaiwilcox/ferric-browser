import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: dialog

    visible: browserWindow.contextMoveVisible
    commandText: ":tab-move --context"
    title: "Move tab to context"
    message: "Only another live window in the same profile can receive the tab."
    keyHelp: "j/k or ↑/↓ select  ·  enter move  ·  esc cancel"
    dialogWidth: 500 * scale
    dialogHeight: 430 * scale
    initialFocusItem: cancelButton

    function dismiss() {
        browserWindow.contextMoveVisible = false;
        browserWindow.contextMoveTabId = "";
        browserWindow.contextMoveChoices = [];
        browserWindow.restoreOverlayFocus();
    }

    function moveChoice(delta) {
        if (choices.count === 0) {
            return;
        }
        var next = choices.currentIndex < 0 ? 0 : choices.currentIndex;
        for (var step = 0; step < choices.count; ++step) {
            next = (next + delta + choices.count) % choices.count;
            var candidate = browserWindow.contextMoveChoices[next];
            if (candidate && candidate.available) {
                choices.currentIndex = next;
                choices.positionViewAtIndex(next, ListView.Contain);
                choices.forceActiveFocus();
                return;
            }
        }
    }

    function activateCurrent() {
        var choice = browserWindow.contextMoveChoices[choices.currentIndex];
        if (choice && choice.available) {
            browserWindow.chooseContextMove(choice.name);
        }
    }

    onDismissRequested: dismiss()

    readonly property bool choiceKeysActive: !containsActiveFocus || focusWithin(choices)
                                             || cancelButton.activeFocus
    Shortcut {
        sequence: "Up"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && dialog.choiceKeysActive
        onActivated: dialog.moveChoice(-1)
    }
    Shortcut {
        sequence: "K"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && dialog.choiceKeysActive
        onActivated: dialog.moveChoice(-1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && dialog.choiceKeysActive
        onActivated: dialog.moveChoice(1)
    }
    Shortcut {
        sequence: "J"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && dialog.choiceKeysActive
        onActivated: dialog.moveChoice(1)
    }
    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        autoRepeat: false
        enabled: dialog.interactive && dialog.focusWithin(choices)
        onActivated: dialog.activateCurrent()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        autoRepeat: false
        enabled: dialog.interactive && dialog.focusWithin(choices)
        onActivated: dialog.activateCurrent()
    }

    ColumnLayout {
        anchors.fill: parent
        FerricManagerList {
            id: choices
            objectName: "contextMoveChoices"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: dialog.hostWindow
            spacing: 6 * dialog.scale
            model: dialog.browserWindow.contextMoveChoices
            currentIndex: 0
            Accessible.role: Accessible.List
            Accessible.name: "Destination contexts"

            delegate: FerricCommandAction {
                required property var modelData
                required property int index
                width: choices.width
                browserWindow: dialog.browserWindow
                keyHint: selected ? "enter" : ""
                actionLabel: modelData.label + (modelData.available ? "" :
                                                                      " (open with context-enter first)")
                selected: index === choices.currentIndex && dialog.focusWithin(choices)
                enabled: !!modelData.available
                onSelectionRequested: choices.currentIndex = index
                onClicked: dialog.browserWindow.chooseContextMove(modelData.name)
            }
            Label {
                textFormat: Text.PlainText
                anchors.centerIn: parent
                width: parent.width
                visible: choices.count === 0
                text: "No other contexts are configured."
                wrapMode: Text.Wrap
                color: dialog.browserWindow.mutedTextColor
            }
        }
        Button {
            id: cancelButton
            objectName: "contextMoveCancel"
            Layout.alignment: Qt.AlignRight
            text: "Cancel"
            onClicked: dialog.dismiss()
        }
    }
}
