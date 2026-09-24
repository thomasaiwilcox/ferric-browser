import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Renders a permission request for a supplied host. Decisions are emitted as
// intents; the composition root owns permission policy and dispatch.
Rectangle {
    id: prompt

    required property var browserWindow
    property var hostWindow: null

    signal decisionRequested(bool allow, string lifetime)

    width: Math.min(600, hostWindow ? hostWindow.width - 80 : 520)
    height: Math.min(280 * browserWindow.chromeScale, hostWindow.height - 32)
    anchors.centerIn: parent
    z: 90
    visible: browserWindow.permissionPromptVisible
             && browserWindow.pendingPermissionHost === hostWindow
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Permission request"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 2

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Permission request"
            color: prompt.browserWindow.primaryTextColor
            font.bold: true
            Accessible.name: "Permission request"
        }
        Label {
            Layout.fillWidth: true
            text: prompt.browserWindow.pendingPermissionOrigin + " wants to "
                  + prompt.browserWindow.permissionDisplayName(
                      prompt.browserWindow.pendingPermissionName) + "."
            color: prompt.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
            Accessible.name: "Permission request for "
                             + prompt.browserWindow.pendingPermissionOrigin
        }
        Label {
            Layout.fillWidth: true
            text: (prompt.browserWindow.pendingPermissionPrivate
                   ? "Private profile · " : "")
                  + "Scope: current document"
                  + (prompt.browserWindow.pendingPermissionGroupCount > 1
                     ? " · " + prompt.browserWindow.pendingPermissionGroupCount
                       + " identical requests grouped"
                     : "")
            color: prompt.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
            Accessible.name: "Permission scope and grouped request count"
        }
        Label {
            Layout.fillWidth: true
            text: prompt.browserWindow.permissionCanRemember(
                      prompt.browserWindow.pendingPermissionOrigin,
                      prompt.browserWindow.pendingPermissionName)
                  ? "Allow once, or remember an allow rule for this exact site."
                  : "This decision applies to this request only."
            color: prompt.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 8
            Button {
                text: "Deny"
                Accessible.name: "Deny permission request"
                onClicked: prompt.decisionRequested(false, "")
            }
            Button {
                text: "Allow for session"
                Accessible.name: "Allow permission for this session"
                visible: prompt.browserWindow.pendingPermissionName !== "screen-capture"
                onClicked: prompt.decisionRequested(true, "session")
            }
            Button {
                text: "Allow once"
                Accessible.name: "Allow permission request once"
                onClicked: prompt.decisionRequested(true, "")
            }
            Button {
                visible: prompt.browserWindow.permissionCanRememberForSite(
                    prompt.browserWindow.pendingPermissionOrigin,
                    prompt.browserWindow.pendingPermissionName)
                text: "Allow for site"
                Accessible.name: "Allow permission for this site"
                onClicked: prompt.decisionRequested(true, "site")
            }
        }
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            prompt.decisionRequested(false, "")
            event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            event.accepted = true
        }
    }
}
