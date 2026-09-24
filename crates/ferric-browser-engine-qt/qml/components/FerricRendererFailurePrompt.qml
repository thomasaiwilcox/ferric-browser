import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Renders a renderer-failure report and emits recovery intents. The owner
// remains responsible for reload, close, diagnostics, and restart policy.
Rectangle {
    id: prompt

    required property var browserWindow
    property var hostWindow: null

    signal reloadRequested()
    signal closeRequested()
    signal copyUrlRequested(string url)
    signal diagnosticsRequested()
    signal softwareRestartRequested()
    signal dismissalRequested()

    width: Math.min(680, hostWindow ? hostWindow.width - 80 : 600)
    height: Math.min(260 * browserWindow.chromeScale, hostWindow.height - 32)
    anchors.centerIn: parent
    z: 85
    visible: browserWindow.rendererFailureVisible
             && browserWindow.rendererFailureHost === hostWindow
             && (hostWindow !== browserWindow
                 || browserWindow.rendererFailureView === browserWindow.activeWebView())
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Page renderer failure"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.errorColor
    border.width: 2

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: "Page renderer stopped"
            color: prompt.browserWindow.errorColor
            font.bold: true
            Accessible.name: "Page renderer failure"
        }
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
        Label {
            Layout.fillWidth: true
            text: prompt.browserWindow.rendererFailureCount > 1
                  ? "This renderer has failed repeatedly. Ferric Browser will not auto-reload it into a loop."
                  : "Other tabs remain usable. Choose an explicit recovery action."
            color: prompt.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Reload"
                Accessible.name: "Reload failed page"
                onClicked: prompt.reloadRequested()
            }
            Button {
                text: prompt.hostWindow === prompt.browserWindow ? "Close tab" : "Close window"
                Accessible.name: text
                onClicked: prompt.closeRequested()
            }
            Button {
                text: "Copy safe URL"
                Accessible.name: "Copy safe URL from renderer failure"
                onClicked: prompt.copyUrlRequested(prompt.browserWindow.rendererFailureSafeUrl)
            }
            Button {
                text: "Diagnostics"
                Accessible.name: "Open renderer diagnostics"
                onClicked: prompt.diagnosticsRequested()
            }
            Button {
                text: "Restart software"
                Accessible.name: "Restart with software rendering"
                enabled: !prompt.browserWindow.softwareRendering
                onClicked: prompt.softwareRestartRequested()
            }
        }
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            prompt.dismissalRequested()
            event.accepted = true
        }
    }
}
