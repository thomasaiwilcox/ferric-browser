import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: preview
    required property var browserWindow
    signal chooseFileRequested()
    signal cancelRequested()

    anchors.fill: parent
    z: 10
    visible: browserWindow.journeyExportPreviewVisible
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    Accessible.role: Accessible.Dialog
    Accessible.name: "Journey export preview"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: "Review journey export"
            color: preview.browserWindow.primaryTextColor
            font.bold: true
        }
        Text {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: preview.browserWindow.journeyExportPreviewText
            color: preview.browserWindow.primaryTextColor
            wrapMode: Text.WordWrap
            Accessible.name: text
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Choose file"
                Accessible.name: "Choose journey export file"
                onClicked: preview.chooseFileRequested()
            }
            Button {
                text: "Cancel"
                Accessible.name: "Cancel journey export"
                onClicked: preview.cancelRequested()
            }
        }
    }
}
