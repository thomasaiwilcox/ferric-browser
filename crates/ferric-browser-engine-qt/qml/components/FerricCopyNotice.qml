import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Displays the already-sanitized copied-value notice. Clipboard policy remains
// with the composition root.
Rectangle {
    required property var browserWindow

    signal copyAgainRequested()
    signal dismissalRequested()

    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: browserWindow.bottomChromeHeight + 8
    width: Math.min(parent.width - 32, 720)
    height: Math.max(copyContent.implicitHeight + 20, browserWindow.chromeRowHeight * 2)
    z: 30
    visible: browserWindow.copyNoticeVisible
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1
    radius: 3

    RowLayout {
        id: copyContent
        anchors.fill: parent
        anchors.margins: 10
        spacing: 8

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 3
            Label {
                Layout.fillWidth: true
                text: "Copied to clipboard"
                textFormat: Text.PlainText
                color: browserWindow.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: browserWindow.copiedText
                textFormat: Text.PlainText
                color: browserWindow.secondaryTextColor
                elide: Text.ElideMiddle
                Accessible.name: browserWindow.copyNoticeSensitive ? "Copied content is hidden" : "Copied address"
            }
        }

        Button {
            text: "Copy again"
            onClicked: copyAgainRequested()
        }
        ToolButton {
            text: "×"
            onClicked: dismissalRequested()
            Accessible.name: "Dismiss copy notice"
        }
    }
}
