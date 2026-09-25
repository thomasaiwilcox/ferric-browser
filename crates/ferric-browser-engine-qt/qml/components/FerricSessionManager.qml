import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
    required property var sessionsModel
    signal closeRequested()
    signal saveRequested(string name)
    signal previewRequested(string name)
    signal deleteRequested(string name, bool confirmed)

    visible: browserWindow.sessionManagerVisible
    commandText: ":session-list"
    title: "Named sessions"
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 620 * scale
    dialogHeight: 480 * scale
    stackingOrder: 40
    initialFocusItem: sessionNameInput
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
                Accessible.name: "Close session manager"
                onClicked: manager.closeRequested()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            TextField {
                id: sessionNameInput
                Layout.fillWidth: true
                placeholderText: "Save current session as..."
                Accessible.name: "New session name"
            }
            Button {
                text: "Save"
                enabled: sessionNameInput.text.length > 0
                onClicked: {
                    manager.saveRequested(sessionNameInput.text)
                    sessionNameInput.text = ""
                }
            }
        }

        ListView {
            id: sessionList
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: manager.sessionsModel
            delegate: RowLayout {
                required property string name
                width: sessionList.width
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: name
                    color: manager.browserWindow.primaryTextColor
                    Accessible.name: "Named session " + name
                }
                Button {
                    text: "Preview"
                    onClicked: manager.previewRequested(name)
                }
                Button {
                    text: manager.browserWindow.pendingDeleteName === name ? "Confirm delete" : "Delete"
                    onClicked: manager.deleteRequested(
                        name,
                        manager.browserWindow.pendingDeleteName === name
                    )
                }
            }
        }
    }
}
