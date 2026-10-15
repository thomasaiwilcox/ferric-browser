import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
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

    visible: browserWindow.profileManagerVisible
    commandText: ":profiles"
    title: "Profiles"
    message: "Keep separate tabs, cookies and browsing history for each profile."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 720 * scale
    dialogHeight: 520 * scale
    stackingOrder: 40
    initialFocusItem: profileNameInput
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
                Accessible.name: "Close profile manager"
                onClicked: manager.closeRequested()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            FerricTextField {
                id: profileNameInput
                Layout.fillWidth: true
                placeholderText: "Profile name, e.g. work"
                Accessible.name: "New profile name"
            }
            FerricTextField {
                id: profileLabelInput
                Layout.fillWidth: true
                placeholderText: "Display name"
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
            FerricTextField {
                Layout.fillWidth: true
                text: manager.renameName
                readOnly: true
                placeholderText: "Select a profile to rename"
                Accessible.name: "Profile being renamed"
            }
            FerricTextField {
                id: profileRenameInput
                Layout.fillWidth: true
                text: manager.renameLabel
                placeholderText: "New display label"
                Accessible.name: "New profile display label"
            }
            Button {
                text: "Rename"
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
            ScrollBar.vertical: ScrollBar {}
            spacing: 10 * manager.scale
            delegate: ColumnLayout {
                required property string name
                required property string label
                width: profileList.width
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: label + (name !== label ? " · " + name : "")
                    textFormat: Text.PlainText
                    color: manager.browserWindow.primaryTextColor
                    font.bold: true
                    elide: Text.ElideRight
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6 * manager.scale
                    Button {
                        text: "Open window"
                        onClicked: manager.openRequested(name, label)
                    }
                    Button {
                        text: "Rename"
                        onClicked: manager.beginRename(name, label)
                    }
                    Button {
                        text: "Delete"
                        onClicked: manager.deleteRequested(name)
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                width: Math.max(0, parent.width - 24)
                visible: profileList.count === 0
                text: "Create a profile above to separate work or personal browsing."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }
    }
}
