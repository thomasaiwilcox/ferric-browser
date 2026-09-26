import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: surface

    required property var browserUi

    visible: browserUi.navigation_failure_visible
    commandText: ":open"
    title: "Navigation failed · " + browserUi.navigation_failure_kind.toUpperCase()
    message: "Retry is intentionally not offered here; submitted forms and authentication flows must not be replayed silently."
    dialogWidth: 760 * scale
    dialogHeight: 480 * scale
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 65
    actions: [
        { id: "cancel", key: "esc", label: "Dismiss", safe: true }
    ]
    onActionRequested: browserUi.clear_navigation_failure()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * surface.scale

        Label {
            Layout.fillWidth: true
            text: "Requested: " + (surface.browserUi.navigation_failure_requested_url || "unavailable")
            color: surface.browserWindow.mutedTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            Layout.fillWidth: true
            text: "Failed URL: " + (surface.browserUi.navigation_failure_url || "unavailable")
            color: surface.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: surface.browserUi.navigation_failure_detail
                  || "The engine did not provide additional detail."
            color: surface.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
            verticalAlignment: Text.AlignTop
        }
    }
}
