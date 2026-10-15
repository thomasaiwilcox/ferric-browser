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
    commandText: "From this page"
    title: browserWindow.pageDialogTitle
    message: "From: " + (browserWindow.pageDialogOrigin || "Origin unavailable") + "\n\n"
             + browserWindow.pageDialogMessage
    keyHelp: "tab/shift-tab controls  ·  enter activate  ·  esc cancel"
    compactKeyHelp: "esc cancel"
    dialogWidth: 620 * scale
    dialogHeight: 470 * scale
    initialFocusItem: browserWindow.pageDialogKind() === "prompt" ? pageDialogInput : (
                                                                        browserWindow.pageDialogKind(
                                                                            ) === "authentication"
                                                                        ? pageDialogUsername : (
                                                                              browserWindow.pageDialogKind(
                                                                                  ) === "alert"
                                                                              ? acceptButton :
                                                                                cancelButton))

    function open(host) {
        hostWindow = host || browserWindow;
        parent = hostWindow.contentItem;
        visible = true;
    }
    function close() {
        visible = false;
        parent = browserWindow.contentItem;
        hostWindow = browserWindow;
    }

    onDismissRequested: browserWindow.rejectPageDialog()

    function activateDefault() {
        if (browserWindow.pageDialogKind() === "beforeunload" || browserWindow.pageDialogKind()
                === "confirm")
            browserWindow.rejectPageDialog();
        else
            browserWindow.acceptPageDialog();
    }

    Shortcut {
        sequence: "Return"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && !dialog.containsActiveFocus
        autoRepeat: false
        onActivated: dialog.activateDefault()
    }
    Shortcut {
        sequence: "Enter"
        context: Qt.WindowShortcut
        enabled: dialog.interactive && !dialog.containsActiveFocus
        autoRepeat: false
        onActivated: dialog.activateDefault()
    }

    FerricScrollColumn {
        anchors.fill: parent
        hostWindow: dialog.hostWindow
        spacing: 8 * dialog.scale
        FerricTextField {
            id: pageDialogInput
            objectName: "pageDialogInput"
            Keys.onReturnPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Keys.onEnterPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "prompt"
            Accessible.name: "Page dialog response"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        FerricTextField {
            id: pageDialogUsername
            objectName: "pageDialogUsername"
            Keys.onReturnPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Keys.onEnterPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Username"
            Accessible.name: "Authentication username"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        FerricTextField {
            id: pageDialogPassword
            objectName: "pageDialogPassword"
            Keys.onReturnPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Keys.onEnterPressed: function (event) {
                if (dialog.interactive && !event.isAutoRepeat && event.modifiers === Qt.NoModifier)
                    dialog.browserWindow.acceptPageDialog();
            }
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Password"
            echoMode: TextInput.Password
            Accessible.name: "Authentication password"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }
        CheckBox {
            id: suppressDialogs
            objectName: "pageDialogSuppress"
            Layout.fillWidth: true
            text: "Suppress future dialogs from this page"
            checked: dialog.browserWindow.pageDialogSuppressChecked
            visible: dialog.browserWindow.pendingPageDialogRequest !== null
            onToggled: dialog.browserWindow.pageDialogSuppressChecked = checked
            Accessible.name: text
            contentItem: Label {
                text: suppressDialogs.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                leftPadding: suppressDialogs.indicator.width + suppressDialogs.spacing
                color: dialog.browserWindow.primaryTextColor
                verticalAlignment: Text.AlignVCenter
            }
        }
        Item {
            Layout.fillHeight: true
        }
        FerricCommandAction {
            id: cancelButton
            objectName: "pageDialogCancel"
            Layout.fillWidth: true
            visible: dialog.browserWindow.pageDialogKind() !== "alert"
            browserWindow: dialog.browserWindow
            keyHint: "esc"
            actionLabel: dialog.browserWindow.pageDialogKind() === "beforeunload" ? "Stay" :
                                                                                    "Cancel"
            safe: true
            selected: activeFocus
            onClicked: dialog.browserWindow.rejectPageDialog()
        }
        FerricCommandAction {
            id: acceptButton
            objectName: "pageDialogAccept"
            Layout.fillWidth: true
            browserWindow: dialog.browserWindow
            keyHint: "enter"
            actionLabel: dialog.browserWindow.pageDialogKind() === "beforeunload" ? "Leave page" : dialog.browserWindow.pageDialogKind() === "authentication" ? "Sign in" : "OK"
            selected: activeFocus
            onClicked: dialog.browserWindow.acceptPageDialog()
        }
    }
}
