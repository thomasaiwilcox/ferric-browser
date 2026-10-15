import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: confirmation
    signal confirmRequested()
    signal cancelRequested()

    visible: browserWindow.reopenWindowConfirmationVisible
    commandText: ":reopen-in-window"
    title: "Reopen tab in a same-profile window?"
    message: "Opens this page’s address in a new window using the same profile. Forms, media and other live page state won’t carry over."
    dialogWidth: 620 * scale
    dialogHeight: 280 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 55
    actions: [
        { id: "cancel", key: "n", shortcuts: ["N"], label: "Keep tab here", safe: true },
        { id: "confirm", key: "y", shortcuts: ["Y"], label: "Reopen in new window" }
    ]
    onActionRequested: function(action) {
        if (action === "confirm") {
            confirmation.confirmRequested()
        } else {
            confirmation.cancelRequested()
        }
    }
}
