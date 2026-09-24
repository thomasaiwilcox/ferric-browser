import QtQuick
import QtQuick.Controls

Item {
    id: root

    // The browser window owns removal policy and storage mutation. This dialog
    // renders the confirmation copy and reports the explicit user decision.
    required property var browserWindow

    signal confirmed()
    signal cancelled()

    function open() {
        dialog.open()
    }

    Dialog {
        id: dialog
        title: "Remove userscript"
        modal: true
        width: Math.min(520, root.browserWindow.width - 48)
        height: Math.min(180, root.browserWindow.height - 48)
        standardButtons: Dialog.Ok | Dialog.Cancel
        contentItem: Label {
            text: "Remove userscript '" + root.browserWindow.pendingUserscriptRemoval
                    + "' and its copied assets?"
            wrapMode: Text.WordWrap
            padding: 16
            color: root.browserWindow.primaryTextColor
        }
        onAccepted: root.confirmed()
        onRejected: root.cancelled()
    }
}
