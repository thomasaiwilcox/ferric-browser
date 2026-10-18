import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Persistent profile-health feedback. The host reserves this height outside
// the page viewport, including when its optional status bar is hidden.
Rectangle {
    id: notice
    required property var browserWindow
    required property var hostWindow
    property string noticeText: ""
    signal detailsRequested()
    readonly property bool inputActive: visible && detailsButton.activeFocus
    objectName: "profileHealthNotice"
    visible: noticeText.length > 0
    readonly property real padding: 8 * Math.min(2, browserWindow.chromeScale || 1)
    readonly property real maximumHeight: Math.max(1, (hostWindow && hostWindow.contentItem ? hostWindow.contentItem.height : 0) * 0.4)
    implicitHeight: visible ? Math.min(maximumHeight, body.contentLayout.implicitHeight + 2 * padding) : 0
    height: implicitHeight
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    border.width: 1
    z: 15

    FerricScrollColumn {
        id: body
        anchors.fill: parent
        anchors.margins: notice.padding
        hostWindow: notice.hostWindow
        GridLayout {
            Layout.fillWidth: true
            columns: width >= 640 * Math.min(2, notice.browserWindow.chromeScale || 1) ? 2 : 1
            columnSpacing: notice.padding
            rowSpacing: notice.padding
            Label {
                objectName: "profileHealthMessage"
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: notice.noticeText
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font: notice.browserWindow.font
                color: notice.browserWindow.primaryTextColor
            }
            Button {
                id: detailsButton
                objectName: "profileHealthDetails"
                Layout.alignment: Qt.AlignTop | Qt.AlignLeft
                Layout.maximumWidth: body.contentWidth
                text: "Diagnostics"
                font: notice.browserWindow.font
                contentItem: Label {
                    text: detailsButton.text
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font: detailsButton.font
                    horizontalAlignment: Text.AlignHCenter
                    color: notice.browserWindow.primaryTextColor
                }
                Accessible.description: notice.noticeText
                onClicked: notice.detailsRequested()
                Keys.onReturnPressed: notice.detailsRequested()
                Keys.onEnterPressed: notice.detailsRequested()
            }
        }
    }
}
