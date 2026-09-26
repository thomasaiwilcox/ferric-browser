import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Explicit recovery choices for a failed renderer. Recovery policy remains in
// the owner; this component only emits bounded intents.
FerricCommandDialog {
    id: prompt

    signal reloadRequested()
    signal closeRequested()
    signal copyUrlRequested(string url)
    signal diagnosticsRequested()
    signal softwareRestartRequested()
    signal dismissalRequested()

    visible: browserWindow.rendererFailureVisible
             && browserWindow.rendererFailureHost === hostWindow
             && (hostWindow !== browserWindow
                 || browserWindow.rendererFailureView === browserWindow.activeWebView())
    commandText: ":renderer-recover"
    title: "Page renderer stopped"
    message: browserWindow.rendererFailureCount > 1
             ? "This renderer has failed repeatedly. Ferric will not auto-reload it into a loop."
             : "Other tabs remain usable. Choose an explicit recovery action."
    dialogWidth: 680 * scale
    dialogHeight: 530 * scale
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 85
    cancelAction: "dismiss"
    actions: [
        { id: "dismiss", key: "n", label: "Leave failed page open", safe: true,
          shortcuts: ["N"] },
        { id: "reload", key: "r", label: "Reload page", shortcuts: ["R"] },
        { id: "copy", key: "y", label: "Copy safe URL", shortcuts: ["Y"] },
        { id: "diagnostics", key: "d", label: "Open diagnostics", shortcuts: ["D"] },
        { id: "software", key: "s", label: "Restart with software rendering",
          enabled: !browserWindow.softwareRendering, shortcuts: ["S"] },
        { id: "close", key: "x",
          label: hostWindow === browserWindow ? "Close tab" : "Close window",
          destructive: true, shortcuts: ["X"] }
    ]

    onActionRequested: function(action) {
        if (action === "reload") {
            prompt.reloadRequested()
        } else if (action === "close") {
            prompt.closeRequested()
        } else if (action === "copy") {
            prompt.copyUrlRequested(prompt.browserWindow.rendererFailureSafeUrl)
        } else if (action === "diagnostics") {
            prompt.diagnosticsRequested()
        } else if (action === "software") {
            prompt.softwareRestartRequested()
        } else {
            prompt.dismissalRequested()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * prompt.scale

        Label {
            Layout.fillWidth: true
            text: "Safe URL: " + prompt.browserWindow.rendererFailureSafeUrl
            color: prompt.browserWindow.primaryTextColor
            elide: Text.ElideMiddle
            Accessible.name: "Safe URL for failed page"
        }
        Label {
            Layout.fillWidth: true
            text: "Reason: " + prompt.browserWindow.rendererFailureReason
                  + " · exit code " + prompt.browserWindow.rendererFailureExitCode
            color: prompt.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
    }
}
