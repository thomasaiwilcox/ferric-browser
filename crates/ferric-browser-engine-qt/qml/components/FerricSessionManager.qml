import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: manager
    required property var browserWindow
    required property var sessionsModel
    signal closeRequested()
    signal saveRequested(string name)
    signal previewRequested(string name)
    signal deleteRequested(string name, bool confirmed)

    anchors.centerIn: parent
    width: Math.min(560 * browserWindow.chromeScale, parent.width - 32)
    height: Math.min(420 * browserWindow.chromeScale, parent.height - 32)
    z: 40
    visible: browserWindow.sessionManagerVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Session manager"
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
                text: "Named sessions"
                color: manager.browserWindow.primaryTextColor
                font.bold: true
            }
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
