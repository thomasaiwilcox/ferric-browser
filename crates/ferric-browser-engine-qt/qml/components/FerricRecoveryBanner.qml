import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricCommandDialog {
    id: banner
    objectName: "recoveryBanner"
    signal recoveryRequested()
    signal dismissalRequested()

    visible: browserWindow.recoveryAvailable
    commandText: "Session recovery"
    title: "Recover the previous browser session?"
    message: "Ferric didn't close normally last time. Restore your previous tabs to pick up where you left off. Forms won't be resubmitted."
    dialogWidth: 640 * scale
    dialogHeight: 280 * scale
    dialogBorderColor: browserWindow.warningColor
    stackingOrder: 80
    actions: [
        { id: "cancel", key: "n / alt+d", shortcuts: ["N"],
          label: "Start without recovery", safe: true },
        { id: "recover", key: "y / alt+r", shortcuts: ["Y"],
          label: "Restore previous tabs" }
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
        enabled: banner.visible
        onActivated: banner.requestAction("recover")
    }
    Shortcut {
        sequence: "Alt+D"
        context: Qt.WindowShortcut
        enabled: banner.visible
        onActivated: banner.requestAction("cancel")
    }
}
