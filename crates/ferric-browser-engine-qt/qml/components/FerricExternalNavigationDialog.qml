import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Presents a runtime decision; it never opens an external application itself.
FerricCommandDialog {
    id: dialog

    required property var browserUi
    property var confirmedActionHandler: function () {
        browserWindow.executePendingEngineAction();
    }

    visible: browserUi.external_navigation_visible
    commandText: "Open another application"
    title: "Open with system handler?"
    message: "This address will leave Ferric Browser and may launch another application."
    dialogWidth: 620 * scale
    dialogHeight: 360 * scale
    dialogBorderColor: browserWindow.warningColor
    cancelAction: "cancel"
    actions: [
        {
            id: "cancel",
            key: "n",
            label: "Stay in Ferric",
            safe: true,
            shortcuts: ["N"]
        },
        {
            id: "open",
            key: "y",
            label: "Open with system handler",
            shortcuts: ["Y"]
        }
    ]

    function confirm() {
        if (browserUi.confirm_external_navigation()) {
            confirmedActionHandler();
        }
    }

    onActionRequested: function (action) {
        if (action === "open") {
            dialog.confirm();
        } else {
            browserUi.cancel_external_navigation();
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Scheme: " + dialog.browserUi.external_navigation_scheme
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.browserWindow.mutedTextColor
        }
        TextArea {
            objectName: "externalNavigationAddress"
            Layout.fillWidth: true
            text: dialog.browserUi.external_navigation_uri
            textFormat: TextEdit.PlainText
            readOnly: true
            selectByMouse: true
            selectByKeyboard: true
            padding: 0
            background: null
            color: dialog.browserWindow.primaryTextColor
            wrapMode: TextEdit.Wrap
            Accessible.name: "Address to open in another application"
        }
    }
}
