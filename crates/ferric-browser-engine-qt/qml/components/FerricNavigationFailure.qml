import QtQuick
import QtQuick.Controls
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricCommandDialog {
    id: surface
    objectName: "navigationFailureSurface"

    required property var browserUi
    property bool detailsExpanded: false
    property var sourceView: null
    property string sourceTabId: ""
    property string sourceAddress: ""
    property bool editingAddress: false
    readonly property var currentView: hostWindow && hostWindow.activeWebView
        ? hostWindow.activeWebView() : null
    readonly property bool formReloadPending: !!currentView && currentView.formResubmissionPending === true
    readonly property bool ownerCurrent: !!sourceView && sourceView === currentView
        && sourceTabId === browserUi.tab_id_for_index(browserUi.active_tab_index)
        && sourceAddress === browserUi.current_url
    signal reopenRequested(string address)
    signal editRequested(string command)
    signal backRequested()

    visible: browserUi.navigation_failure_visible
    enabled: !visible || (ownerCurrent && !formReloadPending)
    opacity: formReloadPending ? 0 : 1
    Accessible.ignored: formReloadPending
    restoreFocus: !editingAddress && (!visible || ownerCurrent)
    commandText: ":open"
    title: "Couldn't load this page"
    message: ChromePresentation.navigationFailureMessage(browserUi.navigation_failure_kind)
    dialogWidth: 760 * scale
    dialogHeight: 500 * scale
    keyHelp: "j/k or ↑/↓ select  ·  enter run  ·  esc close"
    compactKeyHelp: "esc close"
    dialogBorderColor: browserWindow.errorColor
    stackingOrder: 65
    actions: [
        { id: "reopen", key: "r", label: "Open address again", enabled: surface.ownerCurrent },
        { id: "edit", key: "e", label: "Edit address", enabled: surface.ownerCurrent },
        { id: "back", key: "b", label: "Go back", enabled: surface.ownerCurrent && !!surface.currentView.contentCanGoBack },
        { id: "cancel", key: "esc", label: "Close", safe: true }
    ]
    onOpening: {
        sourceView = currentView
        sourceTabId = browserUi.tab_id_for_index(browserUi.active_tab_index)
        sourceAddress = browserUi.current_url
        editingAddress = false
        detailsExpanded = false
    }
    onOwnerCurrentChanged: if (visible && !ownerCurrent && staleTimer) staleTimer.restart()
    Timer {
        id: staleTimer
        interval: 0
        onTriggered: if (surface.visible && !surface.ownerCurrent) surface.browserUi.clear_navigation_failure()
    }
    onActionRequested: function(action) {
        var address = sourceAddress
        var current = ownerCurrent
        editingAddress = action === "edit" && current
        browserUi.clear_navigation_failure()
        if (!current) return
        if (action === "reopen") reopenRequested(address)
        else if (action === "edit") {
            // Quote for Ferric's command grammar, including literal ;; in URLs.
            editRequested('open "' + address.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"')
        } else if (action === "back") backRequested()
    }

    Connections {
        target: surface
        function onVisibleChanged() {
            if (!surface.visible) {
                surface.sourceView = null
                surface.sourceTabId = ""
                surface.sourceAddress = ""
            }
        }
    }

    Column {
        width: parent.width
        spacing: 8 * surface.scale

        Label {
            id: failureAddress
            objectName: "navigationFailureAddress"
            width: parent.width
            text: surface.browserUi.navigation_failure_url || surface.browserUi.navigation_failure_requested_url || "Address unavailable"
            textFormat: Text.PlainText
            color: surface.browserWindow.primaryTextColor
            wrapMode: Text.WrapAnywhere
        }
        Label {
            id: recoveryNote
            textFormat: Text.PlainText
            width: parent.width
            text: "Opening the address again starts a new request. It won't resend form data."
            color: surface.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        ToolButton {
            id: detailsButton
            width: parent.width
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
        TextArea {
            id: failureDetails
            width: parent.width
            visible: surface.detailsExpanded
            objectName: "navigationFailureDetails"
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
}
