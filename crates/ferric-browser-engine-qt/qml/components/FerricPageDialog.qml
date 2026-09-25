import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Page-provided dialogs retain editable controls but use the browser's modal
// frame and window-scoped dismissal so WebEngine can never retain input.
FerricModalSurface {
    id: dialog

    property alias inputText: pageDialogInput.text
    property alias usernameText: pageDialogUsername.text
    property alias passwordText: pageDialogPassword.text

    visible: false
    commandText: ":page-dialog"
    title: browserWindow.pageDialogTitle
    message: browserWindow.pageDialogMessage
    keyHelp: "tab/shift-tab controls  ·  enter accept  ·  esc reject"
    dialogWidth: 620 * scale
    dialogHeight: 470 * scale
    initialFocusItem: browserWindow.pageDialogKind() === "prompt"
                      ? pageDialogInput
                      : (browserWindow.pageDialogKind() === "authentication"
                         ? pageDialogUsername : acceptButton)

    function open() { visible = true }
    function close() { visible = false }

    onDismissRequested: browserWindow.rejectPageDialog()

    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.browserWindow.acceptPageDialog()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        enabled: dialog.visible
        onActivated: dialog.browserWindow.acceptPageDialog()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Origin: " + (dialog.browserWindow.pageDialogOrigin
                                || "opaque or unavailable origin")
            color: dialog.browserWindow.mutedTextColor
            elide: Text.ElideMiddle
            Accessible.name: "Page dialog origin"
        }
        TextField {
            id: pageDialogInput
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "prompt"
            Accessible.name: "Page dialog response"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        TextField {
            id: pageDialogUsername
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Username"
            Accessible.name: "Authentication username"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        TextField {
            id: pageDialogPassword
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Password"
            echoMode: TextInput.Password
            Accessible.name: "Authentication password"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        CheckBox {
            Layout.fillWidth: true
            text: "Suppress future dialogs from this page"
            checked: dialog.browserWindow.pageDialogSuppressChecked
            visible: dialog.browserWindow.pendingPageDialogRequest !== null
            onToggled: dialog.browserWindow.pageDialogSuppressChecked = checked
            Accessible.name: text
        }
        Item { Layout.fillHeight: true }
        FerricCommandAction {
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() !== "alert"
            browserWindow: dialog.browserWindow
            keyHint: "esc"
            actionLabel: dialog.browserWindow.pageDialogKind() === "beforeunload"
                         ? "Stay" : "Cancel"
            safe: true
            onClicked: dialog.browserWindow.rejectPageDialog()
        }
        FerricCommandAction {
            id: acceptButton
            Layout.fillWidth: true
            browserWindow: dialog.browserWindow
            keyHint: "enter"
            actionLabel: dialog.browserWindow.pageDialogKind() === "beforeunload"
                         ? "Leave page" : "OK"
            selected: true
            onClicked: dialog.browserWindow.acceptPageDialog()
        }
    }
}
