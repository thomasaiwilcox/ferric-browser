import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: preview
    required property var previewUi
    signal closeRequested()
    signal navigationConfirmed()

    visible: browserWindow.linkPreviewVisible
    commandText: previewUi.link_preview_command === "url-explain"
                 ? ":url-explain" : ":url-clean"
    title: previewUi.link_preview_command === "url-explain"
           ? "URL explanation" : "Clean-link preview"
    message: "Preview only; no navigation was performed. Sensitive URL components are masked."
    keyHelp: "y confirm navigation when offered  ·  esc close"
    dialogWidth: 900 * scale
    dialogHeight: 520 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 70
    onDismissRequested: closeRequested()

    Shortcut {
        sequence: "Y"
        context: Qt.WindowShortcut
        enabled: preview.visible
                 && preview.previewUi.link_preview_requires_confirmation
        onActivated: preview.navigationConfirmed()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
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
