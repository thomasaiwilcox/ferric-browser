import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    required property var browserWindow

    signal confirmed()
    signal cancelled()

    anchors.fill: parent

    function open() {
        dialog.visible = true
    }

    FerricCommandDialog {
        id: dialog
        browserWindow: root.browserWindow
        visible: false
        commandText: ":userscript-remove"
        title: "Remove userscript?"
        message: "Remove userscript '" + root.browserWindow.pendingUserscriptRemoval
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
