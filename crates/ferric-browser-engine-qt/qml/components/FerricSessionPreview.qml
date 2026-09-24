import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: preview
    required property var browserWindow
    signal closeRequested()
    signal loadRequested(bool append)

    anchors.centerIn: parent
    width: Math.min(620 * browserWindow.chromeScale, parent.width - 32)
    height: Math.min(430 * browserWindow.chromeScale, parent.height - 32)
    z: 50
    visible: browserWindow.sessionPreviewVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Session preview"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.backgroundColor
    border.color: browserWindow.accentColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            preview.closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Load session: " + preview.browserWindow.sessionPreviewName
            color: preview.browserWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: preview.browserWindow.sessionPreviewAppend
                  ? "Append these validated descriptors to the current tabs?"
                  : "Replace the current tabs with these validated descriptors?"
            color: preview.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        Text {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: preview.browserWindow.sessionPreviewText
            color: preview.browserWindow.primaryTextColor
            wrapMode: Text.Wrap
            elide: Text.ElideRight
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Replace"
                enabled: !preview.browserWindow.sessionPreviewLoading
                         && !preview.browserWindow.sessionPreviewAppend
                         && !preview.browserWindow.sessionPreviewError
                onClicked: preview.loadRequested(false)
            }
            Button {
                text: "Append"
                enabled: !preview.browserWindow.sessionPreviewLoading
                         && !preview.browserWindow.sessionPreviewError
                onClicked: preview.loadRequested(true)
            }
            Button {
                text: "Cancel"
                Accessible.name: "Cancel session preview"
                onClicked: preview.closeRequested()
            }
        }
    }
}
