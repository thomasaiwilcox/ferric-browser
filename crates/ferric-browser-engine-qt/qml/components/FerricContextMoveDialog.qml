import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
    id: popup

    // Context policy remains in the composition root; this is a reusable view.
    required property var browserWindow

    parent: Overlay.overlay
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    visible: browserWindow.contextMoveVisible
    width: Math.min(460, browserWindow.width - 48)
    padding: 14
    x: Math.round((browserWindow.width - width) / 2)
    y: Math.round((browserWindow.height - height) / 2)

    function dismiss() {
        browserWindow.contextMoveVisible = false
        browserWindow.contextMoveTabId = ""
        browserWindow.contextMoveChoices = []
        browserWindow.restoreOverlayFocus()
    }

    background: Rectangle {
        color: popup.browserWindow.panelColor
        border.color: popup.browserWindow.accentColor
        radius: 4
    }

    contentItem: ColumnLayout {
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: "Move tab to another context window"
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: "Move tab to context"
            color: popup.browserWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: "Only another live window in the same profile can receive the tab."
            color: popup.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        Repeater {
            model: popup.browserWindow.contextMoveChoices
            delegate: Button {
                Layout.fillWidth: true
                text: modelData.label + (modelData.available ? "" : " (open with context-enter first)")
                enabled: !!modelData.available
                Accessible.name: "Move tab to " + modelData.label
                onClicked: popup.browserWindow.chooseContextMove(modelData.name)
            }
        }
        Button {
            Layout.alignment: Qt.AlignRight
            text: "Cancel"
            Accessible.name: "Cancel context tab move"
            onClicked: popup.dismiss()
        }

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                popup.dismiss()
                event.accepted = true
            }
        }
    }
}
