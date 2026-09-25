import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: transfer
    required property var profilesModel
    required property bool profilesLoading
    signal reopenRequested(string profileName)
    signal bookmarkRequested(string profileName, string profileLabel)
    signal cancelRequested()

    visible: browserWindow.privateHistoryTransferVisible
    commandText: ":history-transfer"
    title: "Reopen private history in a named profile"
    message: "Preview only: this transfers the safe URL. Private history, permissions, cookies, sessions, and marks remain transient."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc cancel"
    dialogWidth: 720 * scale
    dialogHeight: 500 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 82
    initialFocusItem: profileList
    onDismissRequested: cancelRequested()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Title: " + transfer.browserWindow.privateHistoryTransferTitle
            color: transfer.browserWindow.primaryTextColor
            elide: Text.ElideRight
        }
        Label {
            Layout.fillWidth: true
            text: "URL: " + transfer.browserWindow.privateHistoryTransferUrl
            color: transfer.browserWindow.secondaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            Layout.fillWidth: true
            text: "Choose a destination profile:"
            color: transfer.browserWindow.primaryTextColor
        }
        ListView {
            id: profileList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: transfer.profilesModel
            delegate: RowLayout {
                required property string name
                required property string label
                width: profileList.width
                spacing: 8
                Label {
                    Layout.fillWidth: true
                    text: name + " — " + label
                    color: transfer.browserWindow.primaryTextColor
                    elide: Text.ElideRight
                }
                Button {
                    text: "Confirm reopen"
                    Accessible.name: "Confirm private history reopen in " + label
                    onClicked: transfer.reopenRequested(name)
                }
                Button {
                    text: "Add bookmark"
                    Accessible.name: "Add private history entry as a bookmark in " + label
                    onClicked: transfer.bookmarkRequested(name, label)
                }
            }
            Label {
                anchors.centerIn: parent
                visible: profileList.count === 0
                text: transfer.profilesLoading ? "Loading named profiles…" : "No named profiles available"
                color: transfer.browserWindow.mutedTextColor
            }
        }
        Button {
            Layout.alignment: Qt.AlignRight
            text: "Cancel"
            Accessible.name: "Cancel private history transfer"
            onClicked: transfer.cancelRequested()
        }
    }
}
