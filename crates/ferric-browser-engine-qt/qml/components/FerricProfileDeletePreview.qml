import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: preview
    required property var browserWindow
    signal confirmRequested()
    signal cancelRequested()
    anchors.centerIn: parent
    width: Math.min(660 * browserWindow.chromeScale, parent.width - 32)
    height: Math.min(390 * browserWindow.chromeScale, parent.height - 32)
    z: 50
    visible: browserWindow.profileDeletePreviewVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Profile deletion preview"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.backgroundColor
    border.color: browserWindow.errorColor
    Keys.onPressed: function(event) { if (event.key === Qt.Key_Escape) { cancelRequested(); event.accepted = true } }
    ColumnLayout {
        anchors.fill: parent; anchors.margins: 14; spacing: 10
        Label { Layout.fillWidth: true; text: "Delete profile: " + preview.browserWindow.profileDeleteName; color: preview.browserWindow.primaryTextColor; font.bold: true }
        Label { Layout.fillWidth: true; text: "This removes the exact Ferric Browser metadata roots below. QtWebEngine storage is not removed."; color: preview.browserWindow.errorColor; wrapMode: Text.WordWrap }
        Text { Layout.fillWidth: true; Layout.fillHeight: true; text: preview.browserWindow.profileDeletePreviewText; color: preview.browserWindow.primaryTextColor; wrapMode: Text.Wrap; elide: Text.ElideRight }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button { text: "Confirm delete"; onClicked: preview.confirmRequested() }
            Button { text: "Cancel"; onClicked: preview.cancelRequested() }
        }
    }
}
