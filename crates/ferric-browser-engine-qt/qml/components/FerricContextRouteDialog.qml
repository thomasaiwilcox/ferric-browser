import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Explicit pre-navigation context-route decision. The composition root owns
// route validation and application.
FerricCommandDialog {
    id: dialog

    readonly property var routeUi: browserWindow.contextRouteUi || ({})

    visible: browserWindow.contextRouteVisible
    commandText: ":context-route"
    title: "Use browsing context " + (routeUi.context_route_context || "") + "?"
    message: "Context route confirmation"
    dialogWidth: 680 * scale
    dialogHeight: 430 * scale
    cancelAction: "normal"
    actions: [
        { id: "normal", key: "n", label: "Open normally", safe: true,
          shortcuts: ["N"] },
        { id: "context", key: "y", label: "Use context",
          shortcuts: ["Y"] }
    ]

    onActionRequested: function(action) {
        if (action === "context") {
            browserWindow.acceptContextRoute()
        } else {
            browserWindow.dismissContextRoute()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Route " + (dialog.routeUi.context_route_id || "")
                  + " (" + (dialog.routeUi.context_route_behavior || "prompt") + ")"
            color: dialog.browserWindow.mutedTextColor
            elide: Text.ElideRight
        }
        Label {
            Layout.fillWidth: true
            text: "Target profile: "
                  + (dialog.routeUi.context_route_profile || "unavailable")
            color: dialog.browserWindow.secondaryTextColor
            elide: Text.ElideMiddle
        }
        Label {
            Layout.fillWidth: true
            text: "Address: " + (dialog.routeUi.context_route_url || "unavailable")
            color: dialog.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 6
            elide: Text.ElideRight
            Accessible.name: "Context route address"
        }
        Label {
            Layout.fillWidth: true
            text: "This choice applies before navigation. Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
            color: dialog.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
    }
}
