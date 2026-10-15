import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricModalSurface {
    id: manager
    required property var downloadsModel
    signal closeRequested()
    signal openRequested(string downloadId, bool reveal)
    signal actionRequested(string downloadId, string action)

    visible: browserWindow.downloadManagerVisible
    commandText: ":downloads"
    title: "Downloads"
    message: "Your saved and active downloads."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 800 * scale
    dialogHeight: 560 * scale
    stackingOrder: 40
    initialFocusItem: downloadList
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * manager.scale

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Close"
                Accessible.name: "Close downloads"
                onClicked: manager.closeRequested()
            }
        }

        ListView {
            id: downloadList
            objectName: "downloadList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8 * manager.scale
            model: manager.downloadsModel
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: downloadRow
                required property string id
                required property string state
                required property string bytes
                required property real total
                required property real speed
                required property string destination
                required property string reason
                property bool detailsExpanded: false
                width: downloadList.width
                height: downloadContent.implicitHeight + 24 * manager.scale
                color: manager.browserWindow.surfaceColor
                radius: 4 * manager.scale

                ColumnLayout {
                    id: downloadContent
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12 * manager.scale
                    spacing: 6 * manager.scale

                    Label {
                        objectName: "downloadFileName"
                        Layout.fillWidth: true
                        text: ChromePresentation.fileName(downloadRow.destination)
                        textFormat: Text.PlainText
                        font.bold: true
                        color: manager.browserWindow.primaryTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download " + text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: ChromePresentation.downloadStateLabel(downloadRow.state)
                              + " · " + ChromePresentation.downloadProgress(downloadRow.bytes,
                                  downloadRow.total, downloadRow.speed, downloadRow.state)
                        textFormat: Text.PlainText
                        color: downloadRow.state === "interrupted"
                               ? manager.browserWindow.warningColor
                               : manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    ProgressBar {
                        objectName: "downloadProgressBar"
                        Layout.fillWidth: true
                        visible: downloadRow.state === "in-progress" || downloadRow.state === "paused"
                        from: 0
                        to: 1
                        value: downloadRow.total > 0
                               ? Math.max(0, Math.min(1, Number(downloadRow.bytes) / downloadRow.total)) : 0
                        indeterminate: downloadRow.state === "in-progress" && downloadRow.total <= 0
                        Accessible.name: "Download progress"
                    }
                    Label {
                        Layout.fillWidth: true
                        text: downloadRow.destination || "Choose a location to save this file."
                        textFormat: Text.PlainText
                        color: manager.browserWindow.mutedTextColor
                        elide: Text.ElideMiddle
                        Accessible.name: "Download location: " + text
                    }
                    Flow {
                        Layout.fillWidth: true
                        Layout.preferredHeight: childrenRect.height
                        spacing: 6 * manager.scale
                        Button {
                            objectName: "downloadOpenButton"
                            visible: downloadRow.state === "completed"
                            text: "Open"
                            Accessible.name: "Open " + ChromePresentation.fileName(downloadRow.destination)
                            onClicked: manager.openRequested(downloadRow.id, false)
                        }
                        Button {
                            visible: downloadRow.state === "completed"
                            text: "Show in folder"
                            onClicked: manager.openRequested(downloadRow.id, true)
                        }
                        Button {
                            visible: downloadRow.state === "in-progress" || downloadRow.state === "paused"
                            text: downloadRow.state === "paused" ? "Resume" : "Pause"
                            onClicked: manager.actionRequested(downloadRow.id,
                                downloadRow.state === "paused" ? "resume" : "pause")
                        }
                        Button {
                            visible: downloadRow.state === "in-progress" || downloadRow.state === "paused"
                            text: "Cancel"
                            onClicked: manager.actionRequested(downloadRow.id, "cancel")
                        }
                        Button {
                            visible: downloadRow.state === "interrupted" || downloadRow.state === "cancelled"
                            text: "Retry"
                            onClicked: manager.actionRequested(downloadRow.id, "retry")
                        }
                        ToolButton {
                            objectName: "downloadDetailsButton"
                            text: downloadRow.detailsExpanded ? "Hide details" : "Details"
                            onClicked: downloadRow.detailsExpanded = !downloadRow.detailsExpanded
                        }
                    }
                    Label {
                        objectName: "downloadDetails"
                        Layout.fillWidth: true
                        visible: downloadRow.detailsExpanded
                        text: "Download ID: " + downloadRow.id
                              + ((downloadRow.state === "interrupted" || downloadRow.state === "cancelled")
                                 && downloadRow.reason.length > 0 ? "\n" + downloadRow.reason : "")
                        textFormat: Text.PlainText
                        color: manager.browserWindow.secondaryTextColor
                        wrapMode: Text.WrapAnywhere
                    }
                }
            }

            Label {
                objectName: "downloadsEmptyState"
                anchors.centerIn: parent
                width: Math.max(0, parent.width - 24 * manager.scale)
                visible: downloadList.count === 0
                text: "No downloads yet.\nFiles you save will appear here."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }
    }
}
