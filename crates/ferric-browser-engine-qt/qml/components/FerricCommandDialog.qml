import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: dialog

    default property alias detailData: details.data

    property var actions: []
    property string cancelAction: "cancel"
    property int currentIndex: 0
    property bool decisionTaken: false
    readonly property bool choiceKeysActive: {
        var item = hostWindow ? hostWindow.activeFocusItem : null;
        while (item && item !== dialog) {
            if (item.commandDialogChoice === true)
                return true;
            item = item.parent;
        }
        // Keep the fallback for WebEngine focus, but let text inspection and
        // controls inside the dialog receive their own navigation/Enter keys.
        return !item || (hostWindow && hostWindow.activeFocusItem === dialog);
    }

    signal actionRequested(string action)

    keyHelp: "j/k or ↑/↓ select  ·  enter run  ·  esc cancel"
    compactKeyHelp: "esc cancel"
    dismissOnEscape: false
    initialFocusItem: actionRepeater.itemAt(currentIndex)
    minimumDialogHeight: Math.max(160 * scale, choiceMetrics.height * 16)
    minimumBodyHeight: Math.max(100 * scale, choiceMetrics.height * 4)

    FontMetrics { id: choiceMetrics; font: dialog.browserWindow.font }

    readonly property real actionHeightFraction: detailScroll.visible ? 0.65 : 1

    function fittedCommandDialogHeight() {
        // Layouts round each track separately. Round the measured content and
        // gap up too, so fractional font scaling doesn't create a tiny scrollbar.
        var detailHeight = detailScroll.visible
                ? Math.ceil(details.implicitHeight) + Math.ceil(bodyColumn.spacing) : 0
        var actionHeight = Math.ceil(actionScroll.contentLayout.implicitHeight)
        // The action area reserves a share of the body for context. Include
        // that constraint so a short detail section cannot clip the last row.
        return fittedDialogHeight(Math.max(detailHeight + actionHeight,
            Math.ceil(actionHeight / actionHeightFraction)))
    }

    function actionEnabled(index) {
        if (index < 0 || index >= actions.length) {
            return false;
        }
        return actions[index].enabled === undefined || actions[index].enabled;
    }

    function actionShortcuts(action) {
        var sequences = action.shortcuts || (action.key ? [action.key] : []);
        // The dialog owns these keys. Registering them again on a row makes
        // Qt report an ambiguous shortcut instead of dismissing or navigating.
        var navigation = ["esc", "escape", "enter", "return", "up", "down", "j", "k"];
        return sequences.filter(function (sequence) {
            return navigation.indexOf(String(sequence).trim().toLowerCase()) < 0;
        });
    }

    function firstEnabledIndex() {
        for (var i = 0; i < actions.length; ++i) {
            if (actionEnabled(i)) {
                return i;
            }
        }
        return -1;
    }

    function indexForAction(actionId) {
        for (var i = 0; i < actions.length; ++i) {
            if (actions[i].id === actionId && actionEnabled(i)) {
                return i;
            }
        }
        return -1;
    }

    function focusChoice(index) {
        if (!actionEnabled(index)) {
            return;
        }
        currentIndex = index;
        choiceFocusTimer.restart();
    }

    function moveChoice(delta) {
        if (actions.length === 0) {
            return;
        }
        var index = currentIndex;
        for (var step = 0; step < actions.length; ++step) {
            index = (index + delta + actions.length) % actions.length;
            if (actionEnabled(index)) {
                focusChoice(index);
                return;
            }
        }
    }

    function requestAction(actionId) {
        if (!interactive || decisionTaken || indexForAction(actionId) < 0) {
            return;
        }
        decisionTaken = true;
        actionRequested(actionId);
    }

    function activateCurrent() {
        if (actionEnabled(currentIndex)) {
            requestAction(actions[currentIndex].id);
        }
    }

    function cancel() {
        var index = indexForAction(cancelAction);
        if (index >= 0) {
            requestAction(cancelAction);
        }
    }

    function resetChoice() {
        decisionTaken = false;
        var safeIndex = firstEnabledIndex();
        for (var i = 0; i < actions.length; ++i) {
            if (actionEnabled(i) && actions[i].safe === true) {
                safeIndex = i;
                break;
            }
        }
        focusChoice(safeIndex);
    }
    onActionsChanged: if (visible)
                          resetChoice()
    Component.onCompleted: if (visible)
                               resetChoice()
    Timer {
        id: choiceFocusTimer
        interval: 0
        onTriggered: {
            var item = actionRepeater.itemAt(dialog.currentIndex);
            if (dialog.interactive && item) {
                item.forceActiveFocus();
                actionScroll.reveal(item);
            }
        }
    }

    Connections {
        target: dialog

        function onVisibleChanged() {
            if (!dialog.visible) {
                return;
            }
            dialog.resetChoice();
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: dialog.interactive
        autoRepeat: false
        onActivated: dialog.cancel()
    }
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
        enabled: dialog.interactive && dialog.choiceKeysActive
        autoRepeat: false
        onActivated: dialog.activateCurrent()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && dialog.choiceKeysActive
        autoRepeat: false
        onActivated: dialog.activateCurrent()
    }

    ColumnLayout {
        id: bodyColumn
        anchors.fill: parent
        spacing: 8 * dialog.scale

        FerricScrollColumn {
            id: detailScroll
            objectName: "commandDialogDetailScroll"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            visible: details.children.length > 0
            hostWindow: dialog.hostWindow
            Item {
                id: details
                Layout.fillWidth: true
                Layout.fillHeight: true
                implicitHeight: children.reduce(function (height, child) {
                    return Math.max(height, child.implicitHeight);
                }, 0)
            }
        }

        FerricScrollColumn {
            id: actionScroll
            objectName: "commandDialogActionScroll"
            Layout.fillWidth: true
            Layout.fillHeight: !detailScroll.visible
            Layout.minimumHeight: 0
            Layout.preferredHeight: Math.min(contentLayout.implicitHeight,
                                             dialog.bodyItem.height * dialog.actionHeightFraction)
            hostWindow: dialog.hostWindow
            spacing: 8 * dialog.scale
            Repeater {
                id: actionRepeater
                model: dialog.actions

                FerricCommandAction {
                    required property var modelData
                    required property int index
                    readonly property bool commandDialogChoice: true
                    objectName: "commandDialogAction-" + modelData.id
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.max(42, implicitContentHeight + 14)
                    browserWindow: dialog.browserWindow
                    keyHint: modelData.key || ""
                    actionLabel: modelData.label || modelData.id
                    selected: index === dialog.currentIndex
                    destructive: modelData.destructive === true
                    safe: modelData.safe === true
                    enabled: dialog.actionEnabled(index)
                    onSelectionRequested: dialog.currentIndex = index
                    onClicked: dialog.requestAction(modelData.id)

                    Shortcut {
                        sequences: dialog.actionShortcuts(modelData)
                        context: Qt.WindowShortcut
                        enabled: !!dialog && dialog.interactive && dialog.choiceKeysActive && parent.enabled
                                 && sequences.length > 0
                        autoRepeat: false
                        onActivated: dialog.requestAction(modelData.id)
                    }
                }
            }
        }
    }
}
