import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: transfer
    required property var browserWindow
    required property var profilesModel
    required property bool profilesLoading
    signal reopenRequested(string profileName)
    signal bookmarkRequested(string profileName, string profileLabel)
    signal cancelRequested()

    anchors.fill: parent
    z: 12
    visible: browserWindow.privateHistoryTransferVisible
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor
    Accessible.role: Accessible.Dialog
    Accessible.name: "Private history transfer preview"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Reopen private history in a named profile"
            color: transfer.browserWindow.primaryTextColor
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: "Preview only: this transfers the safe URL below. Private history, title, permissions, cookies, sessions, and marks remain in the transient profile."
            color: transfer.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
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
