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

    signal actionRequested(string action)

    keyHelp: "j/k or ↑/↓ select  ·  enter run  ·  esc cancel"
    dismissOnEscape: false

    function actionEnabled(index) {
        if (index < 0 || index >= actions.length) {
            return false
        }
        return actions[index].enabled === undefined || actions[index].enabled
    }

    function firstEnabledIndex() {
        for (var i = 0; i < actions.length; ++i) {
            if (actionEnabled(i)) {
                return i
            }
        }
        return -1
    }

    function indexForAction(actionId) {
        for (var i = 0; i < actions.length; ++i) {
            if (actions[i].id === actionId && actionEnabled(i)) {
                return i
            }
        }
        return -1
    }

    function focusChoice(index) {
        if (!actionEnabled(index)) {
            return
        }
        currentIndex = index
        Qt.callLater(function() {
            var item = actionRepeater.itemAt(dialog.currentIndex)
            if (dialog.visible && item) {
                item.forceActiveFocus()
            }
        })
    }

    function moveChoice(delta) {
        if (actions.length === 0) {
            return
        }
        var index = currentIndex
        for (var step = 0; step < actions.length; ++step) {
            index = (index + delta + actions.length) % actions.length
            if (actionEnabled(index)) {
                focusChoice(index)
                return
            }
        }
    }

    function requestAction(actionId) {
        if (!visible || decisionTaken || indexForAction(actionId) < 0) {
            return
        }
        decisionTaken = true
        actionRequested(actionId)
    }

    function activateCurrent() {
        if (actionEnabled(currentIndex)) {
            requestAction(actions[currentIndex].id)
        }
    }

    function cancel() {
        var index = indexForAction(cancelAction)
        if (index >= 0) {
            requestAction(cancelAction)
        }
    }

    Connections {
        target: dialog

        function onVisibleChanged() {
            if (!dialog.visible) {
                return
            }
            dialog.decisionTaken = false
            var safeIndex = dialog.firstEnabledIndex()
            for (var i = 0; i < dialog.actions.length; ++i) {
                if (dialog.actionEnabled(i) && dialog.actions[i].safe === true) {
                    safeIndex = i
                    break
                }
            }
            dialog.focusChoice(safeIndex)
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.cancel()
    }
    Shortcut {
        sequence: "Up"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.moveChoice(-1)
    }
    Shortcut {
        sequence: "K"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.moveChoice(-1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.moveChoice(1)
    }
    Shortcut {
        sequence: "J"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.moveChoice(1)
    }
    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.activateCurrent()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.activateCurrent()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Item {
            id: details
            Layout.fillWidth: true
            Layout.fillHeight: true
        }

        Repeater {
            id: actionRepeater
            model: dialog.actions

            FerricCommandAction {
                required property var modelData
                required property int index
                objectName: "commandDialogAction-" + modelData.id
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(42 * scale,
                                                  implicitContentHeight + 14 * scale)
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
                    sequences: modelData.shortcuts
                               || (modelData.key ? [modelData.key] : [])
                    context: Qt.WindowShortcut
                    enabled: dialog.visible && parent.enabled && sequences.length > 0
                    onActivated: dialog.requestAction(modelData.id)
                }
            }
        }
    }
}
