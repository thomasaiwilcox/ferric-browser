import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Passive copy feedback. The requesting host owns placement, appearance and
// input; clipboard destination and lifetime remain with the request owner.
Rectangle {
    id: notice
    required property var browserWindow
    required property var hostWindow
    required property bool noticeVisible
    required property string copiedText
    required property bool sensitive
    property bool primarySelection: false
    readonly property string confirmationText: primarySelection ? "Copied to primary selection" : "Copied to clipboard"
    readonly property bool inputActive: visible && (copyAgain.activeFocus || dismiss.activeFocus)
    readonly property bool hovered: noticeHover.hovered
    readonly property real spacingScale: Math.min(2, browserWindow.chromeScale || 1)
    readonly property real padding: 10 * spacingScale
    signal copyAgainRequested()
    signal dismissalRequested()
    objectName: "copyNotice"

    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    readonly property real bottomChromeInset: Number((parent === hostWindow.contentItem
        ? hostWindow.bottomChromeHeight : hostWindow.bottomControlsHeight) || 0)
    anchors.bottomMargin: bottomChromeInset + 8
    width: Math.min(Math.max(0, parent.width - 24), 720 * spacingScale)
    readonly property real minimumActionHeight: Math.ceil(Math.max(copyAgain.implicitHeight, dismiss.implicitHeight) + 2 * padding)
    readonly property real maximumHeight: Math.min(Math.max(1, parent.height - bottomChromeInset - 16), Math.max(parent.height * 0.4, minimumActionHeight))
    height: Math.min(maximumHeight, Math.ceil(copyContent.implicitHeight + 2 * padding))
    z: 35
    visible: noticeVisible
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1
    radius: 3
    Accessible.role: Accessible.AlertMessage
    Accessible.name: confirmationText
    Keys.onEscapePressed: dismissalRequested()
    HoverHandler { id: noticeHover }
    FontMetrics { id: controlMetrics; font: notice.browserWindow.font }
    // Feedback covering a page control must not activate that control when the
    // user clicks its message. Buttons above this area handle their own clicks.
    MouseArea { anchors.fill: parent; acceptedButtons: Qt.LeftButton | Qt.RightButton }

    FerricScrollColumn {
        id: body
        objectName: "copyNoticeBody"
        anchors.fill: parent
        anchors.margins: notice.padding
        hostWindow: notice.hostWindow
        GridLayout {
            id: copyContent
            Layout.fillWidth: true
            Layout.fillHeight: false
            Layout.alignment: Qt.AlignTop
            readonly property bool wide: width >= 620 * notice.spacingScale
            readonly property bool actionsStacked: !wide && body.contentWidth < controlMetrics.advanceWidth(copyAgain.text)
                + copyAgain.leftPadding + copyAgain.rightPadding + dismiss.implicitWidth + columnSpacing
            columns: wide ? 3 : (actionsStacked ? 1 : 2)
            columnSpacing: 8 * notice.spacingScale
            rowSpacing: 8 * notice.spacingScale
            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.columnSpan: copyContent.wide || copyContent.actionsStacked ? 1 : 2
                spacing: 3 * notice.spacingScale
                Label {
                    Layout.fillWidth: true
                    objectName: "copyNoticeTitle"
                    text: notice.confirmationText
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.family: notice.browserWindow.font.family
                    font.pointSize: notice.browserWindow.font.pointSize
                    font.bold: true
                    color: notice.browserWindow.primaryTextColor
                }
                Label {
                    objectName: "copyNoticeText"
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: notice.copiedText
                    textFormat: Text.PlainText
                    font: notice.browserWindow.font
                    color: notice.browserWindow.secondaryTextColor
                    elide: Text.ElideMiddle
                    Accessible.name: notice.sensitive ? "Copied content is hidden" : "Copied content"
                }
            }
            Button {
                id: copyAgain
                objectName: "copyNoticeAgain"
                Layout.fillWidth: !copyContent.wide
                Layout.maximumWidth: copyContent.actionsStacked ? body.contentWidth
                    : Math.max(0, body.contentWidth - dismiss.implicitWidth - copyContent.columnSpacing)
                text: "Copy again"
                Accessible.description: notice.primarySelection ? "Copy this content again to the primary selection" : "Copy this content again to the clipboard"
                font: notice.browserWindow.font
                contentItem: Label {
                    text: copyAgain.text
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font: copyAgain.font
                    horizontalAlignment: Text.AlignHCenter
                    color: notice.browserWindow.primaryTextColor
                }
                onClicked: notice.copyAgainRequested()
                Keys.onReturnPressed: notice.copyAgainRequested()
                Keys.onEnterPressed: notice.copyAgainRequested()
            }
            ToolButton {
                id: dismiss
                objectName: "copyNoticeDismiss"
                text: "×"
                font: notice.browserWindow.font
                onClicked: notice.dismissalRequested()
                Keys.onReturnPressed: notice.dismissalRequested()
                Keys.onEnterPressed: notice.dismissalRequested()
                Accessible.name: "Dismiss copy notice"
            }
        }
    }
}
