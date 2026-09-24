import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: confirmation
    required property var browserWindow
    signal continueRequested()
    signal cancelRequested()

    anchors.centerIn: parent
    width: Math.min(560, parent.width - 80)
    height: Math.min(170 * browserWindow.chromeScale, parent.height - 32)
    z: 80
    visible: browserWindow.rapidHintConfirmationVisible
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    border.width: 2

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Rapid hint has opened 20 background tabs. Continue?"
            color: confirmation.browserWindow.primaryTextColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            text: "Confirming grants one additional bounded batch of 20 tabs."
            color: confirmation.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 8

            Button {
                text: "Continue"
                Accessible.name: "Confirm another rapid hint tab batch"
                onClicked: confirmation.continueRequested()
            }

            Button {
                text: "Cancel hints"
                Accessible.name: "Cancel rapid hints"
                onClicked: confirmation.cancelRequested()
            }
        }
    }
}
