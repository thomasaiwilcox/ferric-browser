import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: preview
    signal closeRequested()
    signal loadRequested(bool append)

    visible: browserWindow.sessionPreviewVisible
    commandText: ":session-load"
    title: "Load session: " + browserWindow.sessionPreviewName
    message: browserWindow.sessionPreviewAppend
             ? "Append these validated descriptors to the current tabs?"
             : "Replace the current tabs with these validated descriptors?"
    dialogWidth: 680 * scale
    dialogHeight: 540 * scale
    stackingOrder: 50
    actions: [
        { id: "cancel", key: "esc", label: "Cancel", safe: true },
        { id: "append", key: "a", shortcuts: ["A"], label: "Append tabs",
          enabled: !browserWindow.sessionPreviewLoading
                   && !browserWindow.sessionPreviewError },
        { id: "replace", key: "r", shortcuts: ["R"], label: "Replace current tabs",
          destructive: true,
          enabled: !browserWindow.sessionPreviewLoading
                   && !browserWindow.sessionPreviewAppend
                   && !browserWindow.sessionPreviewError }
    ]
    onActionRequested: function(action) {
        if (action === "append") {
            preview.loadRequested(true)
        } else if (action === "replace") {
            preview.loadRequested(false)
        } else {
            preview.closeRequested()
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true

        TextArea {
            width: parent.width
            text: preview.browserWindow.sessionPreviewText
            readOnly: true
            wrapMode: TextEdit.Wrap
            color: preview.browserWindow.primaryTextColor
            background: Rectangle { color: preview.browserWindow.surfaceColor }
            Accessible.name: "Validated session descriptors"
        }
    }
}
