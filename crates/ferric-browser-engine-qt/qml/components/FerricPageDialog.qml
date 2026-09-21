import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
    id: popup

    // The composition root owns dialog policy and request resolution. This
    // component only renders the prompt and returns the entered values.
    required property var browserWindow
    property alias inputText: pageDialogInput.text
    property alias usernameText: pageDialogUsername.text
    property alias passwordText: pageDialogPassword.text

    parent: Overlay.overlay
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    width: Math.min(620, browserWindow.width - 48)
    padding: 14
    x: Math.round((browserWindow.width - width) / 2)
    y: Math.round((browserWindow.height - height) / 2)

    background: Rectangle {
        color: popup.browserWindow.panelColor
        border.color: popup.browserWindow.accentColor
        radius: 4
    }

    contentItem: ColumnLayout {
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: "Page dialog"
        spacing: 10

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                popup.browserWindow.rejectPageDialog()
                event.accepted = true
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                popup.browserWindow.acceptPageDialog()
                event.accepted = true
            }
        }

        Label {
            Layout.fillWidth: true
            text: popup.browserWindow.pageDialogTitle
            color: popup.browserWindow.primaryTextColor
            font.bold: true
            elide: Text.ElideRight
            Accessible.name: "Page dialog title"
        }

        Label {
            Layout.fillWidth: true
            text: "Origin: " + (popup.browserWindow.pageDialogOrigin || "opaque or unavailable origin")
            color: popup.browserWindow.mutedTextColor
            elide: Text.ElideMiddle
            Accessible.name: "Page dialog origin"
        }

        Label {
            Layout.fillWidth: true
            text: popup.browserWindow.pageDialogMessage
            color: popup.browserWindow.primaryTextColor
            wrapMode: Text.WordWrap
            maximumLineCount: 12
            elide: Text.ElideRight
            Accessible.name: "Page dialog message"
        }

        TextField {
            id: pageDialogInput
            Layout.fillWidth: true
            visible: popup.browserWindow.pageDialogKind() === "prompt"
            Accessible.name: "Page dialog response"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            onVisibleChanged: if (visible) forceActiveFocus()
        }

        TextField {
            id: pageDialogUsername
            Layout.fillWidth: true
            visible: popup.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Username"
            Accessible.name: "Authentication username"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            onVisibleChanged: if (visible) forceActiveFocus()
        }

        TextField {
            id: pageDialogPassword
            Layout.fillWidth: true
            visible: popup.browserWindow.pageDialogKind() === "authentication"
            placeholderText: "Password"
            echoMode: TextInput.Password
            Accessible.name: "Authentication password"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
        }

        CheckBox {
            Layout.fillWidth: true
            text: "Suppress future dialogs from this page"
            checked: popup.browserWindow.pageDialogSuppressChecked
            visible: popup.browserWindow.pendingPageDialogRequest !== null
            onToggled: popup.browserWindow.pageDialogSuppressChecked = checked
            Accessible.name: text
        }

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                visible: popup.browserWindow.pageDialogKind() !== "alert"
                text: popup.browserWindow.pageDialogKind() === "beforeunload"
                      ? "Stay"
                      : "Cancel"
                Accessible.name: text
                onClicked: popup.browserWindow.rejectPageDialog()
            }
            Button {
                text: popup.browserWindow.pageDialogKind() === "beforeunload"
                      ? "Leave page"
                      : "OK"
                Accessible.name: text
                onClicked: popup.browserWindow.acceptPageDialog()
            }
        }
    }
}
