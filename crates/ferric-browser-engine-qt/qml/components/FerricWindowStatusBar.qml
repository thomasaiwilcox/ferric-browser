import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Compact status chrome for secondary windows. The host computes the bounded
// status snapshot; this component does not inspect runtime or engine objects.
Rectangle {
    id: statusBar
    required property var browserWindow
    required property bool statusVisible
    required property string mode
    required property string displayUrl
    required property string statusText
    required property string profileLabel
    required property color statusColor
    required property color profileColor
    required property string accessibleDetails

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: browserWindow.statusBarHeight
    z: 10
    visible: statusVisible
    color: browserWindow.surfaceColor
    opacity: browserWindow.chromeOpacity

    Accessible.name: "Browser status bar. " + mode + ". " + displayUrl
        + ". " + statusText + accessibleDetails
    Accessible.role: Accessible.StatusBar

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: modeLabel.implicitWidth + 16
            color: statusBar.mode === "insert"
                   ? statusBar.browserWindow.modeInsertColor
                   : statusBar.browserWindow.panelColor

            Label {
                id: modeLabel
                anchors.centerIn: parent
                text: statusBar.mode.toUpperCase()
                color: statusBar.mode === "normal"
                       ? statusBar.browserWindow.primaryTextColor
                       : statusBar.browserWindow.backgroundColor
                font.bold: true
                Accessible.ignored: true
            }
        }

        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            text: statusBar.browserWindow.addressPresentation(
                      statusBar.displayUrl,
                      width / Math.max(1, font.pixelSize * 0.56))
            color: /^https:/i.test(statusBar.displayUrl)
                   ? statusBar.browserWindow.successColor
                   : statusBar.browserWindow.primaryTextColor
            elide: Text.ElideMiddle
            Accessible.ignored: true
        }

        Label {
            Layout.maximumWidth: Math.max(120, parent.width * 0.32)
            Layout.rightMargin: 8
            text: statusBar.statusText
            color: statusBar.statusColor
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignRight
            Accessible.ignored: true
        }

        Label {
            Layout.rightMargin: 8
            text: statusBar.profileLabel
            color: statusBar.profileColor
            font.bold: true
            Accessible.ignored: true
        }
    }
}
