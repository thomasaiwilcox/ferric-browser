import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

// Compact status chrome for secondary windows. The host computes the bounded
// status snapshot; this component does not inspect runtime or engine objects.
Rectangle {
    id: statusBar
    objectName: "browserStatusBar"
    required property var browserWindow
    required property bool statusVisible
    required property string mode
    required property string displayUrl
    property string hintPreviewText: ""
    signal copyUrlRequested()
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
        + ". " + hintPreviewText + ". " + statusText + accessibleDetails + ". " + ChromePresentation.inputModeDescription(mode)
    Accessible.role: Accessible.StatusBar

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        hoverEnabled: true
        onWheel: function(wheel) { wheel.accepted = true }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: modeLabel.implicitWidth + 16
            color: (statusBar.mode === "insert" || statusBar.mode === "insert-locked")
                   ? statusBar.browserWindow.modeInsertColor
                   : ChromePresentation.passThroughMode(statusBar.mode)
                     ? statusBar.browserWindow.accentColor : statusBar.browserWindow.panelColor
            HoverHandler { id: modeHover }
            ToolTip.visible: modeHover.hovered && ChromePresentation.passThroughMode(statusBar.mode)
            ToolTip.text: ChromePresentation.inputModeDescription(statusBar.mode)

            Label {
                textFormat: Text.PlainText
                id: modeLabel
                objectName: "inputModeLabel"
                anchors.centerIn: parent
                text: ChromePresentation.inputModeLabel(statusBar.mode)
                color: statusBar.mode === "normal"
                       ? statusBar.browserWindow.primaryTextColor
                       : statusBar.browserWindow.contrastText(parent.color)
                font.bold: true
                Accessible.ignored: true
            }
        }

        Label {
            textFormat: Text.PlainText
            visible: ChromePresentation.passThroughMode(statusBar.mode)
            objectName: "inputModeHint"
            Layout.leftMargin: 8
            text: ChromePresentation.inputModeHint(statusBar.mode)
            color: statusBar.browserWindow.primaryTextColor
            Accessible.ignored: true
        }

        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            text: statusBar.hintPreviewText.length > 0 ? statusBar.hintPreviewText
                  : statusBar.browserWindow.addressPresentation(
                      statusBar.displayUrl,
                      width / Math.max(1, font.pixelSize * 0.56))
            textFormat: Text.PlainText
            color: /^https:/i.test(statusBar.displayUrl)
                   ? statusBar.browserWindow.successColor
                   : statusBar.browserWindow.primaryTextColor
            elide: Text.ElideMiddle
            Accessible.ignored: true
        }

        ToolButton {
            objectName: "copyCurrentUrlButton"
            Layout.preferredHeight: statusBar.height
            text: "Copy URL"
            enabled: statusBar.displayUrl.length > 0
            Accessible.name: "Copy current page URL"
            ToolTip.visible: hovered
            ToolTip.text: "Copy current page URL (yy in Normal mode)"
            onClicked: statusBar.copyUrlRequested()
        }

        Label {
            textFormat: Text.PlainText
            Layout.maximumWidth: Math.max(120, parent.width * 0.32)
            Layout.rightMargin: 8
            text: statusBar.statusText
            color: statusBar.statusColor
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignRight
            Accessible.ignored: true
        }

        Label {
            textFormat: Text.PlainText
            Layout.rightMargin: 8
            text: statusBar.profileLabel
            color: statusBar.profileColor
            font.bold: true
            Accessible.ignored: true
        }
    }
}
