import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: preview
    signal confirmRequested()
    signal cancelRequested()

    visible: browserWindow.profileDeletePreviewVisible
    commandText: ":profile-delete"
    title: "Delete profile: " + browserWindow.profileDeleteName
    message: "This removes the exact Ferric Browser metadata roots below. QtWebEngine storage is not removed."
    dialogWidth: 680 * scale
    dialogHeight: 480 * scale
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 50
    actions: [
        { id: "cancel", key: "n", shortcuts: ["N"], label: "Keep profile", safe: true },
        { id: "confirm", key: "y", shortcuts: ["Y"], label: "Delete profile", destructive: true }
    ]
    onActionRequested: function(action) {
        if (action === "confirm") {
            preview.confirmRequested()
        } else {
            preview.cancelRequested()
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true

        TextArea {
            width: parent.width
            text: preview.browserWindow.profileDeletePreviewText
            readOnly: true
            wrapMode: TextEdit.Wrap
            color: preview.browserWindow.primaryTextColor
            background: Rectangle { color: preview.browserWindow.surfaceColor }
            Accessible.name: "Profile deletion paths"
        }
    }
}
