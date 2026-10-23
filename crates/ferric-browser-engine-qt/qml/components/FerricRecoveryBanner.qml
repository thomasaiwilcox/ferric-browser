import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: banner
    objectName: "recoveryBanner"
    signal recoveryRequested()
    signal dismissalRequested()
    property bool recoveryPending: false
    property string recoveryError: ""

    visible: browserWindow.recoveryAvailable
    commandText: "Session recovery"
    title: recoveryPending ? "Reading saved tabs…"
         : recoveryError.length > 0 ? "Recovery needs your attention"
         : "Recover the previous browser session?"
    message: recoveryPending ? "You can start without recovery while Ferric reads your saved tabs. Forms won't be resubmitted."
           : recoveryError.length > 0 ? recoveryError
           : "Ferric didn't close normally last time. Restore your saved tabs and keep your current work. Forms won't be resubmitted."
    dialogWidth: 640 * scale
    dialogHeight: 280 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 80
    actions: [
        { id: "cancel", key: "n / alt+d", shortcuts: ["N"],
          label: "Start without recovery", safe: true },
        { id: "recover", key: "y / alt+r", shortcuts: ["Y"],
          label: recoveryPending ? "Reading saved tabs…"
               : recoveryError.length > 0 ? "Try recovery again" : "Restore previous tabs",
          enabled: !recoveryPending }
    ]
    onActionRequested: function(action) {
        if (action === "recover") {
            banner.recoveryRequested()
        } else {
            banner.dismissalRequested()
        }
    }

    Shortcut {
        sequence: "Alt+R"
        context: Qt.WindowShortcut
        enabled: banner.interactive
        onActivated: banner.requestAction("recover")
    }
    Shortcut {
        sequence: "Alt+D"
        context: Qt.WindowShortcut
        enabled: banner.interactive
        onActivated: banner.requestAction("cancel")
    }
}
