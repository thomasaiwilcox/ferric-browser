import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Browser-owned permission decisions use the same command-first contract as
// shutdown and recovery prompts. Policy remains with the composition root.
FerricCommandDialog {
    id: prompt

    signal decisionRequested(bool allow, string lifetime)

    visible: browserWindow.permissionPromptVisible
             && browserWindow.pendingPermissionHost === hostWindow
    commandText: ":permission"
    title: "Permission request"
    message: browserWindow.pendingPermissionOrigin + " wants to "
             + browserWindow.permissionDisplayName(
                   browserWindow.pendingPermissionName) + "."
    dialogWidth: 600 * scale
    dialogHeight: 390 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 90
    cancelAction: "deny"
    actions: {
        var choices = [{
            id: "deny", key: "n", label: "Deny", safe: true,
            shortcuts: ["N"]
        }]
        if (browserWindow.pendingPermissionName !== "screen-capture") {
            choices.push({
                id: "session", key: "s", label: "Allow for session",
                shortcuts: ["S"]
            })
        }
        choices.push({
            id: "once", key: "y", label: "Allow once",
            shortcuts: ["Y"]
        })
        if (browserWindow.permissionCanRememberForSite(
                    browserWindow.pendingPermissionOrigin,
                    browserWindow.pendingPermissionName)) {
            choices.push({
                id: "site", key: "a", label: "Always allow for this site",
                shortcuts: ["A"]
            })
        }
        return choices
    }

    onActionRequested: function(action) {
        if (action === "deny") {
            prompt.decisionRequested(false, "")
        } else if (action === "session") {
            prompt.decisionRequested(true, "session")
        } else if (action === "site") {
            prompt.decisionRequested(true, "site")
        } else {
            prompt.decisionRequested(true, "")
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * prompt.scale

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
                  ? "A remembered rule applies only to this exact site."
                  : "This decision applies to this request only."
            color: prompt.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
    }
}
