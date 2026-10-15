import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
    required property var sessionsModel
    property bool listPending: false
    property string listError: ""
    property var saveFeedback: ({ id: "", state: "idle", message: "", detail: "" })
    property var deleteFeedback: ({ id: "", state: "idle", message: "", detail: "" })
    readonly property var nameField: sessionList.headerItem ? sessionList.headerItem.nameInput : null
    readonly property bool privateSession: browserWindow.temporaryProfile === true || browserWindow.ephemeralProfile === true
    readonly property bool operationPending: saveFeedback.state === "pending" || deleteFeedback.state === "pending"
    property string submittedSaveId: ""
    property string submittedName: ""
    property string pendingDeleteName: ""
    property string rejectionMessage: ""
    property string rejectionDetail: ""
    signal closeRequested()
    signal refreshRequested()
    signal saveRequested(string name)
    signal previewRequested(string name)
    signal deleteRequested(string name, bool confirmed)

    function saveAccepted(id, name) {
        submittedSaveId = id; submittedName = name
        rejectionMessage = ""; rejectionDetail = ""
        applySaveFeedback()
    }
    function deleteAccepted() { pendingDeleteName = ""; rejectionMessage = ""; rejectionDetail = "" }
    function requestRejected(message, detail) { rejectionMessage = message; rejectionDetail = detail }
    function resetDeleteConfirmation() { pendingDeleteName = "" }
    function applySaveFeedback() {
        if (!submittedSaveId || saveFeedback.id !== submittedSaveId || (saveFeedback.state !== "succeeded" && saveFeedback.state !== "failed")) return
        if (saveFeedback.state === "succeeded" && nameField && nameField.text === submittedName) nameField.text = ""
        submittedSaveId = ""
    }
    onSaveFeedbackChanged: applySaveFeedback()
    onOperationPendingChanged: if (operationPending) pendingDeleteName = ""
    onVisibleChanged: {
        pendingDeleteName = ""
        if (visible && sessionList) sessionList.positionViewAtBeginning()
    }

    visible: browserWindow.sessionManagerVisible
    commandText: ":sessions"
    title: "Named sessions"
    message: privateSession
             ? "Private browsing doesn't save named sessions. Use a regular profile to save or restore tabs."
             : "Save groups of tabs and return to them later."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 620 * scale
    dialogHeight: 520 * scale
    stackingOrder: 40
    initialFocusItem: nameField
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * manager.scale
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button { objectName: "sessionRefreshButton"; text: "Refresh"; enabled: !manager.privateSession && !manager.listPending; onClicked: { manager.resetDeleteConfirmation(); manager.refreshRequested() } }
            Button { text: "Close"; Accessible.name: "Close session manager"; onClicked: manager.closeRequested() }
        }
        FerricManagerList {
            id: sessionList
            objectName: "sessionList"
            hostWindow: manager.hostWindow
            Layout.fillWidth: true
            Layout.fillHeight: true
            model: manager.sessionsModel
            spacing: 10 * manager.scale
            header: ColumnLayout {
                property alias nameInput: sessionNameInput
                width: sessionList.width
                spacing: 8 * manager.scale
                FerricTextField {
                    id: sessionNameInput
                    objectName: "sessionNameInput"
                    Layout.fillWidth: true
                    placeholderText: "Save current session as…"
                    enabled: !manager.privateSession
                    readOnly: manager.operationPending
                    Accessible.name: "New session name"
                }
                Button {
                    objectName: "sessionSaveButton"
                    text: "Save session"
                    enabled: !manager.privateSession && !manager.operationPending && sessionNameInput.text.trim().length > 0
                    onClicked: manager.saveRequested(sessionNameInput.text)
                }
                FerricManagerFeedback {
                    objectName: "sessionRequestFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.rejectionMessage
                    detail: manager.rejectionDetail
                    failed: true
                }
                FerricManagerFeedback {
                    objectName: "sessionSaveFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.saveFeedback.message || ""
                    detail: manager.saveFeedback.detail || ""
                    failed: manager.saveFeedback.state === "failed"
                    pending: manager.saveFeedback.state === "pending"
                }
                FerricManagerFeedback {
                    objectName: "sessionDeleteFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.deleteFeedback.message || ""
                    detail: manager.deleteFeedback.detail || ""
                    failed: manager.deleteFeedback.state === "failed"
                    pending: manager.deleteFeedback.state === "pending"
                }
                FerricManagerFeedback {
                    objectName: "sessionListFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.listPending ? "Loading saved sessions…" : manager.listError.length > 0 ? "Saved sessions couldn't be loaded. Refresh to try again." : ""
                    detail: manager.listError
                    failed: manager.listError.length > 0 && !manager.listPending
                    pending: manager.listPending
                }
                Item { implicitHeight: 6 * manager.scale }
            }
            delegate: ColumnLayout {
                required property string name
                width: sessionList.width
                spacing: 6 * manager.scale
                Label {
                    Layout.fillWidth: true
                    text: name
                    textFormat: Text.PlainText
                    color: manager.browserWindow.primaryTextColor
                    wrapMode: Text.Wrap
                    Accessible.name: "Named session " + name
                }
                Label {
                    Layout.fillWidth: true
                    visible: manager.pendingDeleteName === name
                    text: "Delete this saved session? Open tabs will stay open."
                    textFormat: Text.PlainText
                    color: manager.browserWindow.warningColor
                    wrapMode: Text.Wrap
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6 * manager.scale
                    Button { text: "Preview"; onClicked: manager.previewRequested(name) }
                    Button {
                        text: manager.pendingDeleteName === name ? "Confirm delete" : "Delete"
                        enabled: !manager.operationPending && !manager.privateSession
                        onClicked: {
                            if (manager.pendingDeleteName === name) manager.deleteRequested(name, true)
                            else manager.pendingDeleteName = name
                        }
                    }
                    Button { text: "Keep session"; visible: manager.pendingDeleteName === name; onClicked: manager.resetDeleteConfirmation() }
                }
            }
            footer: Label {
                width: sessionList.width
                visible: sessionList.count === 0 && !manager.listPending && manager.listError.length === 0
                text: manager.privateSession ? "Nothing from this private session will be saved here." : "Save a session above to keep a group of tabs together."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                wrapMode: Text.Wrap
            }
        }
    }
}
