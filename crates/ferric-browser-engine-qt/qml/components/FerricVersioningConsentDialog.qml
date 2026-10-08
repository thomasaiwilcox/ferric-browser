import QtQuick

// The operation ID routes a browser-owned decision; it is not an approval
// token. Only fixed disclosure copy supplied by the Rust application is shown.
FerricCommandDialog {
    id: dialog

    property string operationId: ""
    property string disclosureText: ""
    property bool acceptEnabled: false
    property bool pageCaptureMode: false

    signal acceptRequested(string operationId)
    signal declineRequested(string operationId)

    visible: operationId.length > 0 && disclosureText.length > 0
    commandText: pageCaptureMode ? "archive page" : ":stash"
    title: pageCaptureMode ? "Save a page archive?" : "Keep this page in memory?"
    message: disclosureText
    dialogWidth: 620 * scale
    dialogHeight: 360 * scale
    dialogBorderColor: browserWindow.warningColor
    cancelAction: "decline"
    actions: [
        { id: "decline", key: "n",
          label: pageCaptureMode ? "Cancel archive" : "Keep page visible", safe: true,
          shortcuts: ["N"] },
        { id: "accept", key: "y",
          label: pageCaptureMode ? "Save archive" : "Stash in memory",
          shortcuts: ["Y"], enabled: dialog.acceptEnabled }
    ]

    onActionRequested: function(action) {
        if (action === "accept") {
            dialog.acceptRequested(dialog.operationId)
        } else {
            dialog.declineRequested(dialog.operationId)
        }
    }
}
