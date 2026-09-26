import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: confirmation
    signal continueRequested()
    signal cancelRequested()

    visible: browserWindow.rapidHintConfirmationVisible
    commandText: ":hint --rapid"
    title: "Open another background-tab batch?"
    message: "Rapid hinting has opened 20 background tabs. Confirming grants one additional bounded batch of 20 tabs."
    dialogWidth: 600 * scale
    dialogHeight: 250 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 80
    actions: [
        { id: "cancel", key: "n", shortcuts: ["N"], label: "Cancel hints", safe: true },
        { id: "continue", key: "y", shortcuts: ["Y"], label: "Continue rapid hints" }
    ]
    onActionRequested: function(action) {
        if (action === "continue") {
            confirmation.continueRequested()
        } else {
            confirmation.cancelRequested()
        }
    }
}
