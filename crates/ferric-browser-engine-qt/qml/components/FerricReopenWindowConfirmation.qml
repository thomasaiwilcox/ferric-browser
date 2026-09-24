import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: confirmation
    required property var browserWindow
    signal confirmRequested()
    signal cancelRequested()

    anchors.centerIn: parent
    width: Math.min(560 * browserWindow.chromeScale, parent.width - 32)
    height: Math.min(260 * browserWindow.chromeScale, parent.height - 32)
    z: 55
    visible: browserWindow.reopenWindowConfirmationVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Reopen tab in window confirmation"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            confirmation.cancelRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Reopen tab in a same-profile window?"
            color: confirmation.browserWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: "This opens a safe URL descriptor in a new window. Live page state, forms, media, and in-progress engine work will not be preserved."
            color: confirmation.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Reopen"
                Accessible.name: "Confirm reopen tab in window"
                onClicked: confirmation.confirmRequested()
            }
            Button {
                text: "Cancel"
                Accessible.name: "Cancel reopen tab in window"
                onClicked: confirmation.cancelRequested()
            }
        }
    }
}
