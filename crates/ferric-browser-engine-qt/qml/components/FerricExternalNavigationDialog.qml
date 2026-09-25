import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presents a runtime decision; it never opens an external application itself.
FerricCommandDialog {
    id: dialog

    required property var browserUi
    property var confirmedActionHandler: function() {
        browserWindow.executePendingEngineAction()
    }

    visible: browserUi.external_navigation_visible
    commandText: ":open-external"
    title: "Open with system handler?"
    message: "This URI will leave Ferric Browser and may launch another application."
    dialogWidth: 620 * scale
    dialogHeight: 360 * scale
    dialogBorderColor: browserWindow.warningColor
    cancelAction: "cancel"
    actions: [
        { id: "cancel", key: "n", label: "Stay in Ferric", safe: true,
          shortcuts: ["N"] },
        { id: "open", key: "y", label: "Open with system handler",
          shortcuts: ["Y"] }
    ]

    function confirm() {
        if (browserUi.confirm_external_navigation()) {
            confirmedActionHandler()
        }
    }

    onActionRequested: function(action) {
        if (action === "open") {
            dialog.confirm()
        } else {
            browserUi.cancel_external_navigation()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Scheme: " + dialog.browserUi.external_navigation_scheme
            color: dialog.browserWindow.mutedTextColor
        }
        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: dialog.browserUi.external_navigation_uri
            color: dialog.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 8
            elide: Text.ElideRight
            Accessible.name: "External URI"
        }
    }
}
