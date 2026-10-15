import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
    required property var sessionsModel
    readonly property bool privateSession: browserWindow.temporaryProfile === true
                                            || browserWindow.ephemeralProfile === true
    signal closeRequested()
    signal saveRequested(string name)
    signal previewRequested(string name)
    signal deleteRequested(string name, bool confirmed)

    visible: browserWindow.sessionManagerVisible
    commandText: ":sessions"
    title: "Named sessions"
    message: privateSession
             ? "Private browsing doesn't save named sessions. Use a regular profile to save or restore tabs."
             : "Save groups of tabs and return to them later."
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
            FerricTextField {
                id: sessionNameInput
                Layout.fillWidth: true
                placeholderText: "Save current session as..."
                enabled: !manager.privateSession
                Accessible.name: "New session name"
            }
            Button {
                objectName: "sessionSaveButton"
                text: "Save"
                enabled: !manager.privateSession && sessionNameInput.text.trim().length > 0
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
            ScrollBar.vertical: ScrollBar {}
            delegate: RowLayout {
                required property string name
                width: sessionList.width
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: name
                    textFormat: Text.PlainText
                    color: manager.browserWindow.primaryTextColor
                    elide: Text.ElideRight
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
            Label {
                anchors.centerIn: parent
                width: Math.max(0, parent.width - 24)
                visible: sessionList.count === 0
                text: manager.privateSession
                      ? "Nothing from this private session will be saved here."
                      : "Save a session above to keep a group of tabs together."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }
    }
}
