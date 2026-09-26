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
    message: "This opens a safe URL descriptor in a new window. Live page state, forms, media, and in-progress engine work will not be preserved."
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
