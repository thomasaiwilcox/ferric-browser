import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Renders a bounded browser-status snapshot. Navigation, profile, and engine
// policy remain at the composition root; this component only derives chrome.
Rectangle {
    id: statusBar
    required property var browserWindow
    required property bool statusVisible
    required property string mode
    required property string displayUrl
    required property string statusText
    property bool statusError: false
    required property string contextName
    required property color contextColor
    required property string profileName
    required property bool temporaryProfile
    required property bool ephemeralProfile
    required property int blockingSiteCount
    required property int activeTabIndex
    required property int tabCount
    required property string engineUpdateNotice
    required property string macroStatusText
    required property var activeView
    required property string accessibleDetails

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: browserWindow.statusBarHeight
    z: 10
    color: browserWindow.surfaceColor
    opacity: browserWindow.chromeOpacity
    visible: statusVisible

    Accessible.name: "Browser status bar. " + mode + ". " + displayUrl
        + ". " + statusText + accessibleDetails
    Accessible.role: Accessible.StatusBar

    function modeColor() {
        if (mode === "insert") return browserWindow.modeInsertColor
        if (mode === "hint") return browserWindow.warningColor
        if (mode === "caret") return browserWindow.accentColor
        return browserWindow.panelColor
    }

    function profileLabel() {
        return (ephemeralProfile ? "EPHEMERAL "
                : temporaryProfile ? "PRIVATE " : "")
            + profileName + (contextName ? ":" + contextName : "")
    }

    function activityLabel() {
        var load = activeView && activeView.loading
                ? " " + Math.round(Number(activeView.loadProgress || 0)) + "%" : ""
        var media = activeView && activeView.audioMuted ? " M" : ""
        return (blockingSiteCount > 0 ? " B" : "") + media + load + "  "
            + (activeTabIndex + 1) + "/" + tabCount
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: statusModeLabel.implicitWidth + 16
            color: statusBar.modeColor()

            Label {
                id: statusModeLabel
                anchors.centerIn: parent
                text: statusBar.mode.toUpperCase()
                color: statusBar.mode === "normal"
                       ? statusBar.browserWindow.primaryTextColor
                       : statusBar.browserWindow.contrastText(parent.color)
                font.bold: true
                Accessible.ignored: true
            }
        }

        Label {
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            Layout.maximumWidth: Math.max(90, statusBar.width * 0.18)
            text: statusBar.profileLabel()
            color: statusBar.contextColor
            elide: Text.ElideRight
            Accessible.ignored: true
        }

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 1
            color: statusBar.browserWindow.borderColor
        }

        Label {
            id: statusUrl
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            text: statusBar.browserWindow.addressPresentation(
                      statusBar.displayUrl,
                      width / Math.max(1, font.pixelSize * 0.56))
            color: /^https:/i.test(statusBar.displayUrl)
                   ? statusBar.browserWindow.successColor
                   : (/^http:/i.test(statusBar.displayUrl)
                      ? statusBar.browserWindow.warningColor
                      : statusBar.browserWindow.primaryTextColor)
            elide: Text.ElideMiddle
            Accessible.ignored: true
        }

        Label {
            Layout.maximumWidth: Math.max(120, statusBar.width * 0.32)
            Layout.rightMargin: 10
            visible: text.length > 0
            text: statusBar.engineUpdateNotice.length > 0
                  ? statusBar.engineUpdateNotice
                  : statusBar.statusText + statusBar.macroStatusText
            color: statusBar.engineUpdateNotice.length > 0
                   ? statusBar.browserWindow.warningColor
                   : (statusBar.statusError
                      ? statusBar.browserWindow.errorColor
                      : statusBar.browserWindow.mutedTextColor)
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignRight
            Accessible.ignored: true
        }

        Label {
            Layout.rightMargin: 8
            text: statusBar.activityLabel()
            color: statusBar.browserWindow.secondaryTextColor
            font.bold: true
            Accessible.ignored: true
        }
    }
}
