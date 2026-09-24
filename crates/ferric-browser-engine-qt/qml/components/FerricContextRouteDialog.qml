import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Popup {
    id: popup

    // The composition root validates and applies routes. This component only
    // renders the explicit pre-navigation decision.
    required property var browserWindow

    parent: Overlay.overlay
    modal: true
    focus: true
    closePolicy: Popup.NoAutoClose
    visible: browserWindow.contextRouteVisible
    width: Math.min(680 * browserWindow.chromeScale, browserWindow.width - 48)
    padding: 14
    x: Math.round((browserWindow.width - width) / 2)
    y: Math.round((browserWindow.height - height) / 2)

    background: Rectangle {
        color: popup.browserWindow.panelColor
        border.color: popup.browserWindow.accentColor
        radius: 4
    }

    contentItem: ColumnLayout {
        focus: true
        Accessible.role: Accessible.Dialog
        Accessible.name: "Context route confirmation"
        spacing: 10

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                popup.browserWindow.dismissContextRoute()
                event.accepted = true
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                popup.browserWindow.acceptContextRoute()
                event.accepted = true
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Use browsing context "
                  + (popup.browserWindow.contextRouteUi.context_route_context || "") + "?"
            color: popup.browserWindow.primaryTextColor
            font.bold: true
            Accessible.name: "Context route title"
        }
        Label {
            Layout.fillWidth: true
            text: "Route " + (popup.browserWindow.contextRouteUi.context_route_id || "")
                  + " (" + (popup.browserWindow.contextRouteUi.context_route_behavior || "prompt") + ")"
            color: popup.browserWindow.mutedTextColor
            elide: Text.ElideRight
        }
        Label {
            Layout.fillWidth: true
            text: "Target profile: "
                  + (popup.browserWindow.contextRouteUi.context_route_profile || "unavailable")
            color: popup.browserWindow.secondaryTextColor
            elide: Text.ElideMiddle
        }
        Label {
            Layout.fillWidth: true
            text: "Address: " + (popup.browserWindow.contextRouteUi.context_route_url || "unavailable")
            color: popup.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 6
            elide: Text.ElideRight
            Accessible.name: "Context route address"
        }
        Label {
            Layout.fillWidth: true
            text: "This choice applies before navigation. Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
            color: popup.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: "Open normally"
                Accessible.name: "Open without context route"
                onClicked: popup.browserWindow.dismissContextRoute()
            }
            Button {
                text: "Use context"
                Accessible.name: "Accept context route"
                onClicked: popup.browserWindow.acceptContextRoute()
            }
        }
    }
}
