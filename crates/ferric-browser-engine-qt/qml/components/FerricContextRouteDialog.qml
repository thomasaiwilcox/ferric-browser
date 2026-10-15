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
    message: "Use this context’s profile and site settings for this navigation?"
    dialogWidth: 680 * scale
    dialogHeight: 430 * scale
    cancelAction: "normal"
    actions: [
        {
            id: "normal",
            key: "n",
            label: "Open normally",
            safe: true,
            shortcuts: ["N"]
        },
        {
            id: "context",
            key: "y",
            label: "Use context",
            shortcuts: ["Y"]
        }
    ]

    onActionRequested: function (action) {
        if (action === "context") {
            browserWindow.acceptContextRoute();
        } else {
            browserWindow.dismissContextRoute();
        }
    }

    Connections {
        target: dialog
        function onVisibleChanged() {
            routeDetails.checked = false;
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Target profile: " + (dialog.routeUi.context_route_profile || "unavailable")
            color: dialog.browserWindow.secondaryTextColor
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }
        TextArea {
            objectName: "contextRouteAddress"
            Layout.fillWidth: true
            text: dialog.routeUi.context_route_url || "Address unavailable"
            textFormat: TextEdit.PlainText
            readOnly: true
            selectByMouse: true
            selectByKeyboard: true
            padding: 0
            background: null
            color: dialog.browserWindow.primaryTextColor
            wrapMode: TextEdit.Wrap
            Accessible.name: "Context route address"
        }
        Label {
            Layout.fillWidth: true
            text: "This choice applies before navigation. Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
            color: dialog.browserWindow.warningColor
            wrapMode: Text.WordWrap
        }
        ToolButton {
            id: routeDetails
            objectName: "contextRouteDetailsButton"
            Layout.fillWidth: true
            contentItem: Label {
                text: routeDetails.text
                font: routeDetails.font
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.browserWindow.primaryTextColor
            }
            text: checked ? "Hide route details" : "Show route details"
            checkable: true
        }
        TextArea {
            objectName: "contextRouteDetails"
            Layout.fillWidth: true
            visible: routeDetails.checked
            text: "Rule: " + (dialog.routeUi.context_route_id || "Unavailable") + "\nBehavior: " + (
                      dialog.routeUi.context_route_behavior || "Unavailable")
            textFormat: TextEdit.PlainText
            readOnly: true
            selectByMouse: true
            selectByKeyboard: true
            wrapMode: TextEdit.Wrap
            padding: 0
            background: null
            color: dialog.browserWindow.secondaryTextColor
            Accessible.name: "Technical route details"
        }
    }
}
