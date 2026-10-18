import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    required property var browserWindow
    property var hostWindow: browserWindow
    property string scriptName: browserWindow.pendingUserscriptRemoval || ""
    readonly property bool active: dialog.visible

    signal confirmed()
    signal cancelled()

    anchors.fill: parent
    z: 90

    function open() {
        dialog.visible = true
    }

    FerricCommandDialog {
        id: dialog
        objectName: "userscriptRemovalConfirmation"
        browserWindow: root.browserWindow
        hostWindow: root.hostWindow
        visible: false
        commandText: ":userscript-remove"
        title: "Remove userscript?"
        message: "Remove userscript '" + root.scriptName
                 + "' and its copied assets?"
        dialogWidth: 600 * scale
        dialogHeight: 270 * scale
        dialogBorderColor: root.browserWindow.errorColor
        stackingOrder: 90
        actions: [
            { id: "cancel", key: "n", shortcuts: ["N"], label: "Keep userscript", safe: true },
            { id: "confirm", key: "y", shortcuts: ["Y"], label: "Remove userscript", destructive: true }
        ]
        onActionRequested: function(action) {
            dialog.visible = false
            if (action === "confirm") {
                root.confirmed()
            } else {
                root.cancelled()
            }
        }
    }
}
