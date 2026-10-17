import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Explicit pre-navigation context-route decision. The composition root owns
// route validation and application.
FerricCommandDialog {
    id: dialog

    readonly property var routeUi: browserWindow.contextRouteUi || ({})
    hostWindow: browserWindow.contextRouteHost || browserWindow
    parent: hostWindow && hostWindow.contentItem ? hostWindow.contentItem : null

    visible: browserWindow.contextRouteVisible
    commandText: ":context-route"
    title: "Use context " + (routeUi.context_route_context || "") + "?"
    message: "Open this address with the context's profile and site settings?"
    dialogWidth: 680 * scale
    dialogHeight: Math.min(430 * scale, fittedCommandDialogHeight())
    cancelAction: "normal"
    actions: [
        {
            id: "normal",
            key: "n",
            label: "Keep current profile",
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
        width: parent.width
        height: implicitHeight
        spacing: 8 * dialog.scale

        Label {
            Layout.fillWidth: true
            text: "Profile: " + (dialog.routeUi.context_route_profile || "unavailable")
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
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: "Other open tabs and sign-in flows keep their current profile."
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
                  + "\nRouting is chosen before navigation; existing page state is not transferred between profiles."
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
