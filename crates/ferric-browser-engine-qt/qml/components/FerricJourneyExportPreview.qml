import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: preview
    signal chooseFileRequested()
    signal cancelRequested()

    visible: browserWindow.journeyExportPreviewVisible
    commandText: ":journey-export"
    title: "Review journey export"
    message: "Review the sanitized export before choosing its destination."
    dialogWidth: 760 * scale
    dialogHeight: 520 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 82
    actions: [
        { id: "cancel", key: "n", shortcuts: ["N"], label: "Cancel export", safe: true },
        { id: "choose", key: "y", shortcuts: ["Y"], label: "Choose destination" }
    ]
    onActionRequested: function(action) {
        if (action === "choose") {
            preview.chooseFileRequested()
        } else {
            preview.cancelRequested()
        }
    }

    ScrollView {
        anchors.fill: parent
        clip: true

        TextArea {
            width: parent.width
            text: preview.browserWindow.journeyExportPreviewText
            readOnly: true
            wrapMode: TextEdit.Wrap
            color: preview.browserWindow.primaryTextColor
            background: Rectangle { color: preview.browserWindow.surfaceColor }
            Accessible.name: "Journey export preview"
        }
    }
}
