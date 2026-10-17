import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: preview
    property var state: browserWindow
    signal confirmRequested()
    signal cancelRequested()
    property bool previewPending: true
    property string folderText: ""
    onVisibleChanged: foldersButton.checked = false

    visible: state.profileDeletePreviewVisible
    commandText: ":profile-delete"
    title: "Delete profile: " + state.profileDeleteName
    message: "Removes the profile record and Ferric's saved profile data, sessions and cache. Cookies and website storage stay on disk."
    dialogWidth: 680 * scale
    dialogHeight: 480 * scale
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 50
    actions: [
        { id: "cancel", key: "n", shortcuts: ["N"], label: "Keep profile", safe: true },
        { id: "confirm", key: "y", shortcuts: ["Y"], label: "Delete profile metadata", destructive: true,
          enabled: !previewPending && folderText.length > 0 }
    ]
    onActionRequested: function(action) {
        if (action === "confirm") {
            preview.confirmRequested()
        } else {
            preview.cancelRequested()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        Label {
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: preview.previewPending ? "Checking the profile folders…"
                : "This action cannot be undone. You can inspect the affected folders below."
            color: preview.browserWindow.secondaryTextColor
            wrapMode: Text.Wrap
        }
        Button {
            id: foldersButton
            Layout.fillWidth: true
            contentItem: Label {
                text: foldersButton.text
                font: foldersButton.font
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: preview.browserWindow.primaryTextColor
            }
            text: checked ? "Hide affected folders" : "Show affected folders"
            checkable: true
            enabled: !preview.previewPending
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: foldersButton.checked
            clip: true
            TextArea {
                text: preview.folderText
                textFormat: TextEdit.PlainText
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                wrapMode: TextEdit.Wrap
                color: preview.browserWindow.primaryTextColor
                background: Rectangle { color: preview.browserWindow.surfaceColor }
                Accessible.name: "Profile deletion folders"
            }
        }
        Item { Layout.fillHeight: true; visible: !foldersButton.checked }
    }
}
