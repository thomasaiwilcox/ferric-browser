import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: surface

    // Navigation classification and retry policy live in the runtime-backed UI
    // object. The view deliberately exposes dismissal only.
    required property var browserWindow
    required property var browserUi

    anchors.centerIn: parent
    width: Math.min(720, parent.width - 64)
    height: Math.min(360, parent.height - 120)
    z: 65
    visible: browserUi.navigation_failure_visible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Navigation failure"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.errorColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            browserUi.clear_navigation_failure()
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
                text: "Navigation failed · " + surface.browserUi.navigation_failure_kind.toUpperCase()
                color: surface.browserWindow.errorColor
                font.bold: true
            }
            Button {
                text: "Dismiss"
                Accessible.name: "Dismiss navigation failure"
                onClicked: surface.browserUi.clear_navigation_failure()
            }
        }
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
            text: surface.browserUi.navigation_failure_detail || "The engine did not provide additional detail."
            color: surface.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
            verticalAlignment: Text.AlignTop
        }
        Label {
            Layout.fillWidth: true
            text: "Retry is intentionally not offered here; submitted forms and authentication flows must not be replayed silently."
            color: surface.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
    }
}
