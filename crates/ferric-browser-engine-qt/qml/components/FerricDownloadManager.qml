import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
    required property var downloadsModel
    signal closeRequested()
    signal openRequested(string downloadId, bool reveal)
    signal actionRequested(string downloadId, string action)

    visible: browserWindow.downloadManagerVisible
    commandText: ":downloads"
    title: "Downloads"
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 800 * scale
    dialogHeight: 520 * scale
    stackingOrder: 40
    initialFocusItem: downloadList
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Close"
                Accessible.name: "Close download manager"
                onClicked: manager.closeRequested()
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Downloads are saved to the user Downloads directory with validated names and collision-safe renaming."
            color: manager.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }

        ListView {
            id: downloadList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: manager.downloadsModel
            delegate: ColumnLayout {
                required property string id
                required property string state
                required property string bytes
                required property real total
                required property real speed
                required property string destination
                required property string reason
                width: downloadList.width
                spacing: 2
                Label {
                    Layout.fillWidth: true
                    text: state + " · " + bytes + " bytes"
                          + (Number(total) >= 0 ? " / " + total + " bytes" : "")
                          + (manager.browserWindow.formatDownloadRate(speed).length > 0
                             ? " · " + manager.browserWindow.formatDownloadRate(speed) : "")
                          + " · " + id
                    color: manager.browserWindow.primaryTextColor
                }
                Label {
                    Layout.fillWidth: true
                    text: destination.length > 0 ? destination : "Selecting destination"
                    color: manager.browserWindow.mutedTextColor
                    elide: Text.ElideMiddle
                }
                Label {
                    Layout.fillWidth: true
                    visible: reason.length > 0
                    text: "Failure reason: " + reason
                    color: manager.browserWindow.warningColor
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    visible: state === "completed"
                    Button {
                        text: "Open"
                        Accessible.name: "Open completed download"
                        onClicked: manager.openRequested(id, false)
                    }
                    Button {
                        text: "Show"
                        Accessible.name: "Reveal completed download"
                        onClicked: manager.openRequested(id, true)
                    }
                }
                RowLayout {
                    visible: state === "in-progress" || state === "paused"
                    Button {
                        text: state === "paused" ? "Resume" : "Pause"
                        Accessible.name: state === "paused" ? "Resume download" : "Pause download"
                        onClicked: manager.actionRequested(id, state === "paused" ? "resume" : "pause")
                    }
                    Button {
                        text: "Cancel"
                        Accessible.name: "Cancel download"
                        onClicked: manager.actionRequested(id, "cancel")
                    }
                }
                RowLayout {
                    visible: state === "interrupted" || state === "cancelled"
                    Button {
                        text: "Retry"
                        Accessible.name: "Retry download"
                        onClicked: manager.actionRequested(id, "retry")
                    }
                }
            }
        }
    }
}
