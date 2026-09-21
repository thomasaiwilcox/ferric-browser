import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
    id: popup

    // This component presents a runtime decision; it never opens an external
    // application itself.
    required property var browserWindow
    required property var browserUi

    parent: Overlay.overlay
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    visible: browserUi.external_navigation_visible
    width: Math.min(620, browserWindow.width - 48)
    padding: 14
    x: Math.round((browserWindow.width - width) / 2)
    y: Math.round((browserWindow.height - height) / 2)

    function confirm() {
        if (browserUi.confirm_external_navigation()) {
            browserWindow.executePendingEngineAction()
        }
    }

    background: Rectangle {
        color: popup.browserWindow.panelColor
        border.color: popup.browserWindow.warningColor
        radius: 4
    }

    contentItem: ColumnLayout {
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: "External URI confirmation"
        spacing: 10

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                popup.browserUi.cancel_external_navigation()
                event.accepted = true
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                popup.confirm()
                event.accepted = true
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Open with system handler?"
            color: popup.browserWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: "This URI will leave Ferric Browser and may launch another application."
            color: popup.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        Label {
            Layout.fillWidth: true
            text: "Scheme: " + popup.browserUi.external_navigation_scheme
            color: popup.browserWindow.mutedTextColor
        }
        Label {
            Layout.fillWidth: true
            text: popup.browserUi.external_navigation_uri
            color: popup.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 8
            elide: Text.ElideRight
            Accessible.name: "External URI"
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Cancel"
                Accessible.name: "Cancel external URI"
                onClicked: popup.browserUi.cancel_external_navigation()
            }
            Button {
                text: "Open with system handler"
                Accessible.name: "Confirm external URI"
                onClicked: popup.confirm()
            }
        }
    }
}
