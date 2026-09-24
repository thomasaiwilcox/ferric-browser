import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: preview
    required property var browserWindow
    required property var previewUi
    signal closeRequested()
    signal navigationConfirmed()

    anchors.centerIn: parent
    width: Math.min(900, parent.width - 80)
    height: Math.min(470, parent.height - 120)
    z: 70
    visible: browserWindow.linkPreviewVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Clean-link preview"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.warningColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            preview.closeRequested()
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
                text: preview.previewUi.link_preview_command === "url-explain" ? "URL explanation" : "Clean-link preview"
                color: preview.browserWindow.primaryTextColor
                font.bold: true
            }
            Button {
                text: "Confirm navigation"
                visible: preview.previewUi.link_preview_requires_confirmation
                Accessible.name: "Confirm cleaned URL navigation"
                onClicked: preview.navigationConfirmed()
            }
            Button {
                text: "Close"
                Accessible.name: "Close URL preview"
                onClicked: preview.closeRequested()
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Preview only; no navigation was performed. Sensitive URL components are masked."
            color: preview.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            text: "Original: " + (preview.previewUi.link_preview_original || "")
            color: preview.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            Layout.fillWidth: true
            text: "Result: " + (preview.previewUi.link_preview_cleaned || "")
            color: preview.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            Layout.fillWidth: true
            text: "Rules: " + (preview.previewUi.link_preview_applied_rules.join(", ") || "none")
            color: preview.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
        }
        Label {
            Layout.fillWidth: true
            text: "Removed: " + (preview.previewUi.link_preview_removed_parameters.join(", ") || "none")
            color: preview.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
        }
        Label {
            Layout.fillWidth: true
            text: "Retained: " + (preview.previewUi.link_preview_retained_parameters.join(", ") || "none")
            color: preview.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
        }
        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: preview.previewUi.link_preview_explanation || ""
            color: preview.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
            verticalAlignment: Text.AlignTop
        }
    }
}
