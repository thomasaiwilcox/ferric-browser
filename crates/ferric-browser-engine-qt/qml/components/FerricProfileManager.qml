import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: manager
    required property var browserWindow
    required property var profilesModel
    property string renameName: ""
    property string renameLabel: ""
    signal closeRequested()
    signal createRequested(string name, string label)
    signal renameRequested(string name, string label)
    signal openRequested(string name, string label)
    signal deleteRequested(string name)

    function clearCreateInputs() {
        profileNameInput.text = ""
        profileLabelInput.text = ""
    }

    function beginRename(name, label) {
        renameName = name
        renameLabel = label
        profileRenameInput.text = label
    }

    function clearRename() {
        renameName = ""
        renameLabel = ""
        profileRenameInput.text = ""
    }

    anchors.centerIn: parent
    width: Math.min(620 * browserWindow.chromeScale, parent.width - 32)
    height: Math.min(430 * browserWindow.chromeScale, parent.height - 32)
    z: 40
    visible: browserWindow.profileManagerVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Profile manager"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.borderColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            manager.closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Profiles"
                color: manager.browserWindow.primaryTextColor
                font.bold: true
            }
            Button {
                text: "Close"
                Accessible.name: "Close profile manager"
                onClicked: manager.closeRequested()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            TextField {
                id: profileNameInput
                Layout.fillWidth: true
                placeholderText: "new-profile-name"
                Accessible.name: "New profile name"
            }
            TextField {
                id: profileLabelInput
                Layout.fillWidth: true
                placeholderText: "Display label"
                Accessible.name: "New profile label"
            }
            Button {
                text: "Create"
                enabled: profileNameInput.text.length > 0 && profileLabelInput.text.length > 0
                onClicked: manager.createRequested(profileNameInput.text, profileLabelInput.text)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            TextField {
                Layout.fillWidth: true
                text: manager.renameName
                readOnly: true
                placeholderText: "Select a profile to rename"
                Accessible.name: "Profile being renamed"
            }
            TextField {
                id: profileRenameInput
                Layout.fillWidth: true
                text: manager.renameLabel
                placeholderText: "New display label"
                Accessible.name: "New profile display label"
            }
            Button {
                text: "Rename label"
                enabled: manager.renameName.length > 0 && profileRenameInput.text.length > 0
                onClicked: manager.renameRequested(manager.renameName, profileRenameInput.text)
            }
        }

        ListView {
            id: profileList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: manager.profilesModel
            delegate: RowLayout {
                required property string name
                required property string label
                width: profileList.width
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: name + " — " + label
                    color: manager.browserWindow.primaryTextColor
                }
                Button {
                    text: "Open window"
                    onClicked: manager.openRequested(name, label)
                }
                Button {
                    text: "Edit label"
                    onClicked: manager.beginRename(name, label)
                }
                Button {
                    text: "Delete"
                    onClicked: manager.deleteRequested(name)
                }
            }
        }
    }
}
