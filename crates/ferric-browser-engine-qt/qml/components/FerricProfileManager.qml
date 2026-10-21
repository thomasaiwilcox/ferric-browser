import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: manager
    FontMetrics { id: formFont; font: manager.browserWindow.font }
    property var state: browserWindow
    required property var profilesModel
    property bool listPending: false
    property string listError: ""
    property var feedback: ({ id: "", state: "idle", message: "", detail: "" })
    readonly property var createNameField: profileList.headerItem ? profileList.headerItem.nameInput : null
    readonly property var createLabelField: profileList.headerItem ? profileList.headerItem.labelInput : null
    readonly property var renameField: profileList.headerItem ? profileList.headerItem.renameInput : null
    property string renameName: ""
    property string renameLabel: ""
    property Item renameReturnItem: null
    property string submittedId: ""
    property string submittedKind: ""
    property string submittedName: ""
    property string submittedLabel: ""
    property string rejectionMessage: ""
    property string rejectionDetail: ""
    readonly property bool operationPending: feedback.state === "pending"
    signal closeRequested()
    signal refreshRequested()
    signal createRequested(string name, string label)
    signal renameRequested(string name, string label)
    signal openRequested(string name, string label)
    signal deleteRequested(string name)

    function clearCreateInputs() {
        var hadFocus = createNameField.activeFocus || createLabelField.activeFocus || profileList.headerItem.createButton.activeFocus
        createNameField.text = ""; createLabelField.text = ""
        if (hadFocus && interactive) createNameField.forceActiveFocus()
    }
    function beginRename(name, label, returnItem) {
        renameReturnItem = returnItem || null
        renameName = name; renameLabel = label; renameField.text = label
        renameFocusTimer.restart()
    }
    function clearRename() {
        var hadFocus = focusWithin(profileList.headerItem.renameForm)
        var returnItem = renameReturnItem
        renameName = ""; renameLabel = ""; renameField.text = ""; renameReturnItem = null
        if (hadFocus && interactive) {
            if (returnItem && returnItem.visible && returnItem.enabled) returnItem.forceActiveFocus()
            else createNameField.forceActiveFocus()
        }
    }
    function ignoreRepeatedReturn(event) {
        if (event.isAutoRepeat && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) event.accepted = true
    }
    function mutationAccepted(id, kind, name, label) {
        submittedId = id; submittedKind = kind; submittedName = name; submittedLabel = label
        rejectionMessage = ""; rejectionDetail = ""
        applyFeedback()
    }
    function requestRejected(message, detail) { rejectionMessage = message; rejectionDetail = detail }
    function applyFeedback() {
        if (!submittedId || feedback.id !== submittedId || (feedback.state !== "succeeded" && feedback.state !== "failed")) return
        if (feedback.state === "succeeded") {
            if (submittedKind === "create" && createNameField.text === submittedName && createLabelField.text === submittedLabel) clearCreateInputs()
            if (submittedKind === "rename" && renameName === submittedName && renameField.text === submittedLabel) clearRename()
        }
        submittedId = ""
        rejectionMessage = ""; rejectionDetail = ""
    }
    onFeedbackChanged: applyFeedback()
    onVisibleChanged: if (visible && profileList) profileList.positionViewAtBeginning()
    Timer { id: renameFocusTimer; interval: 0; onTriggered: if (manager.interactive && manager.renameName.length > 0) renameField.forceActiveFocus() }

    visible: state.profileManagerVisible
    commandText: ":profiles"
    title: "Profiles"
    message: "Keep separate tabs, cookies and browsing history for each profile."
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc close"
    dialogWidth: 720 * scale
    dialogHeight: 560 * scale
    minimumBodyHeight: Math.max(100 * scale, formFont.height * 2 + 24 * scale)
    stackingOrder: 40
    initialFocusItem: createNameField
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * manager.scale
        FerricManagerList {
            id: profileList
            objectName: "profileList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            hostWindow: manager.hostWindow
            model: manager.profilesModel
            spacing: 10 * manager.scale
            header: ColumnLayout {
                property alias nameInput: profileNameInput
                property alias labelInput: profileLabelInput
                property alias renameInput: profileRenameInput
                property alias renameForm: profileRenameForm
                property alias createButton: createProfileButton
                width: profileList.width
                spacing: 8 * manager.scale
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6 * manager.scale
                    FerricWrappingButton { objectName: "profileRefreshButton"; text: "Refresh"; enabled: !manager.listPending; onClicked: manager.refreshRequested() }
                    FerricWrappingButton { objectName: "profileCloseButton"; text: "Close"; onClicked: manager.closeRequested() }
                }
                Label { Layout.fillWidth: true; text: "New profile"; color: manager.browserWindow.primaryTextColor; font.bold: true }
                FerricTextField {
                    id: profileNameInput
                    objectName: "profileNameInput"
                    Layout.fillWidth: true
                    placeholderText: "Profile name, e.g. work"
                    readOnly: manager.operationPending
                    Accessible.name: "New profile name"
                    Keys.onPressed: function(event) { manager.ignoreRepeatedReturn(event) }
                    onAccepted: if (manager.interactive && !manager.operationPending && text.trim().length > 0)
                        profileLabelInput.forceActiveFocus(Qt.TabFocusReason)
                }
                FerricTextField {
                    id: profileLabelInput
                    objectName: "profileLabelInput"
                    Layout.fillWidth: true
                    placeholderText: "Display name"
                    readOnly: manager.operationPending
                    Accessible.name: "New profile display name"
                    Keys.onPressed: function(event) { manager.ignoreRepeatedReturn(event) }
                    onAccepted: if (manager.interactive && createProfileButton.enabled)
                        manager.createRequested(profileNameInput.text, profileLabelInput.text)
                }
                FerricWrappingButton {
                    id: createProfileButton
                    Layout.maximumWidth: profileList.width
                    objectName: "profileCreateButton"
                    text: "Create profile"
                    enabled: !manager.operationPending && profileNameInput.text.trim().length > 0 && profileLabelInput.text.trim().length > 0
                    onClicked: manager.createRequested(profileNameInput.text, profileLabelInput.text)
                }
                ColumnLayout {
                    id: profileRenameForm
                    objectName: "profileRenameForm"
                    Layout.fillWidth: true
                    visible: manager.renameName.length > 0
                    Label {
                        Layout.fillWidth: true
                        text: "Rename " + manager.renameName
                        textFormat: Text.PlainText
                        color: manager.browserWindow.primaryTextColor
                        font.bold: true
                        wrapMode: Text.Wrap
                    }
                    FerricTextField {
                        id: profileRenameInput
                        objectName: "profileRenameInput"
                        Layout.fillWidth: true
                        placeholderText: "New display name"
                        readOnly: manager.operationPending
                        Accessible.name: "New profile display name"
                        Keys.onPressed: function(event) { manager.ignoreRepeatedReturn(event) }
                        onAccepted: if (manager.interactive && saveRenameButton.enabled)
                            manager.renameRequested(manager.renameName, text)
                    }
                    Flow {
                        Layout.fillWidth: true
                        Layout.preferredHeight: childrenRect.height
                        spacing: 6 * manager.scale
                        FerricWrappingButton {
                            id: saveRenameButton
                            objectName: "profileRenameButton"
                            text: "Save name"
                            enabled: !manager.operationPending && profileRenameInput.text.trim().length > 0
                            onClicked: manager.renameRequested(manager.renameName, profileRenameInput.text)
                        }
                        FerricWrappingButton { text: "Cancel rename"; enabled: !manager.operationPending; onClicked: manager.clearRename() }
                    }
                }
                FerricManagerFeedback {
                    objectName: "profileFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.rejectionMessage || manager.feedback.message || ""
                    detail: manager.rejectionMessage ? manager.rejectionDetail : manager.feedback.detail || ""
                    failed: manager.rejectionMessage.length > 0 || manager.feedback.state === "failed"
                    pending: manager.operationPending
                }
                FerricManagerFeedback {
                    objectName: "profileListFeedback"
                    Layout.fillWidth: true
                    browserWindow: manager.browserWindow
                    message: manager.listPending ? "Loading profiles…" : manager.listError.length > 0 ? "Profiles couldn't be loaded. Refresh to try again." : ""
                    detail: manager.listError
                    failed: manager.listError.length > 0 && !manager.listPending
                    pending: manager.listPending
                }
                Item { implicitHeight: 6 * manager.scale }
            }
            delegate: ColumnLayout {
                required property string name
                required property string label
                width: profileList.width
                spacing: 6 * manager.scale
                Label {
                    Layout.fillWidth: true
                    text: label + (name !== label ? " · " + name : "")
                    textFormat: Text.PlainText
                    color: manager.browserWindow.primaryTextColor
                    font.bold: true
                    wrapMode: Text.Wrap
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.preferredHeight: childrenRect.height
                    spacing: 6 * manager.scale
                    FerricWrappingButton { text: "Open window"; onClicked: manager.openRequested(name, label) }
                    FerricWrappingButton {
                        id: renameAction
                        objectName: "profileBeginRenameButton"
                        text: "Rename"
                        enabled: !manager.operationPending
                        onClicked: manager.beginRename(name, label, renameAction)
                    }
                    FerricWrappingButton { text: "Delete"; enabled: !manager.operationPending; onClicked: manager.deleteRequested(name) }
                }
            }
            footer: Label {
                width: profileList.width
                visible: profileList.count === 0 && !manager.listPending && manager.listError.length === 0
                text: "Create a profile above to separate work or personal browsing."
                textFormat: Text.PlainText
                color: manager.browserWindow.secondaryTextColor
                wrapMode: Text.Wrap
            }
        }
    }
}
