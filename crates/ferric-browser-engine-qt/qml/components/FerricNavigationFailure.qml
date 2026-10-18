import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricCommandDialog {
    id: surface

    required property var browserUi
    property bool detailsExpanded: false

    visible: browserUi.navigation_failure_visible
    commandText: ":open"
    title: "Couldn't load this page"
    message: ChromePresentation.navigationFailureMessage(browserUi.navigation_failure_kind)
    keyHelp: "enter dismiss  ·  esc close"
    dialogWidth: 760 * scale
    dialogHeight: 480 * scale
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 65
    actions: [
        { id: "cancel", key: "esc", label: "Dismiss", safe: true }
    ]
    onActionRequested: browserUi.clear_navigation_failure()

    Connections {
        target: surface
        function onVisibleChanged() { surface.detailsExpanded = false }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * surface.scale

        Label {
            Layout.fillWidth: true
            text: surface.browserUi.navigation_failure_url || surface.browserUi.navigation_failure_requested_url || "Address unavailable"
            textFormat: Text.PlainText
            color: surface.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: "Ferric won't retry automatically or resubmit a form."
            color: surface.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        ToolButton {
            id: detailsButton
            Layout.fillWidth: true
            contentItem: Label {
                text: detailsButton.text
                font: detailsButton.font
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: surface.browserWindow.primaryTextColor
            }
            objectName: "navigationFailureDetailsButton"
            text: surface.detailsExpanded ? "Hide technical details" : "Show technical details"
            onClicked: surface.detailsExpanded = !surface.detailsExpanded
            Accessible.name: text
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: surface.detailsExpanded
            clip: true
            TextArea {
                objectName: "navigationFailureDetails"
                width: parent.width
                text: "Requested address: " + (surface.browserUi.navigation_failure_requested_url || "Unavailable")
                      + "\nFailure: " + surface.browserUi.navigation_failure_kind.toUpperCase()
                      + "\n" + (surface.browserUi.navigation_failure_detail || "No additional detail is available.")
                textFormat: TextEdit.PlainText
                readOnly: true
                selectByMouse: true
                selectByKeyboard: true
                wrapMode: TextEdit.WrapAnywhere
                color: surface.browserWindow.secondaryTextColor
                Accessible.name: "Technical details for this page failure"
            }
        }
        Item { Layout.fillHeight: true; visible: !surface.detailsExpanded }
    }
}
