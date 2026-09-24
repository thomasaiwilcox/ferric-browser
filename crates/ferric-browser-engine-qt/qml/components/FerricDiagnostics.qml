import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: diagnostics

    required property var browserWindow

    signal closeRequested()
    signal refreshRequested()
    signal copyRequested()
    signal saveRequested()

    anchors.centerIn: parent
    width: Math.min(900, parent.width - 80)
    height: Math.min(620, parent.height - 100)
    z: 72
    visible: browserWindow.diagnosticsVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Diagnostics"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Diagnostics"
                color: diagnostics.browserWindow.primaryTextColor
                font.bold: true
                Accessible.name: "Diagnostics"
            }
            Button {
                text: "Refresh"
                Accessible.name: "Refresh diagnostics"
                onClicked: diagnostics.refreshRequested()
            }
            Button {
                text: "Copy preview"
                Accessible.name: "Copy diagnostics preview"
                onClicked: diagnostics.copyRequested()
            }
            Button {
                text: "Save preview"
                Accessible.name: "Save diagnostics preview"
                onClicked: diagnostics.saveRequested()
            }
            Button {
                text: "Close"
                Accessible.name: "Close diagnostics"
                onClicked: diagnostics.closeRequested()
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Read-only, privacy-safe runtime snapshot. Probe values are bounded and may be marked unavailable or not tested."
            color: diagnostics.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            text: "Qt logical-unit layout · current screen scale: "
                  + diagnostics.browserWindow.displayScaleLabel
            color: diagnostics.browserWindow.mutedTextColor
            Accessible.name: "Current screen scale"
        }

        Label {
            Layout.fillWidth: true
            text: "Window activation: " + diagnostics.browserWindow.activationStatus
            color: diagnostics.browserWindow.activationStatus === "unknown"
                  ? diagnostics.browserWindow.warningColor
                  : diagnostics.browserWindow.mutedTextColor
            Accessible.name: "Window activation status"
        }

        Label {
            Layout.fillWidth: true
            visible: diagnostics.browserWindow.themeContrastWarning.length > 0
            text: "Theme contrast warning: " + diagnostics.browserWindow.themeContrastWarning
            color: diagnostics.browserWindow.warningColor
            wrapMode: Text.WordWrap
            Accessible.name: "Theme contrast warning"
            Accessible.description: "The imported theme remains enabled, but one or more assessed chrome color pairs are below WCAG AA."
        }

        Label {
            Layout.fillWidth: true
            text: diagnostics.browserWindow.reducedMotionActive
                  ? "Optional interface motion is disabled (reduced-motion setting: system or on)."
                  : "Optional interface motion is enabled by the reduced-motion setting."
            color: diagnostics.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
            Accessible.name: "Reduced motion status"
        }

        ScrollView {
            id: diagnosticsScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            TextArea {
                width: diagnosticsScroll.availableWidth
                height: Math.max(diagnosticsScroll.availableHeight, contentHeight + 16)
                text: diagnostics.browserWindow.diagnosticsText
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.NoWrap
                color: diagnostics.browserWindow.primaryTextColor
                selectionColor: diagnostics.browserWindow.accentColor
                selectedTextColor: diagnostics.browserWindow.backgroundColor
                font.family: diagnostics.browserWindow.chromeFontFamily
                Accessible.name: "Read-only diagnostics snapshot"
                Accessible.description: "Privacy-safe runtime diagnostics in JSON format"
            }
        }
    }
}
