import QtQuick

// Keep the restore dialog tree out of the initial window compilation. An inactive
// Loader with sourceComponent still compiles that tree during window startup.
Loader {
    id: restoreHost
    required property var hostWindow
    required property var browserUi
    active: false
    asynchronous: false

    function handleSidecarReady(action, allowEditable) {
        if (!active) {
            setSource(Qt.resolvedUrl("FerricVersioningRestoreCoordinator.qml"), {
                hostWindow: restoreHost.hostWindow,
                browserUi: restoreHost.browserUi
            })
            active = true
        }
        if (!item) {
            browserUi.status_text = "Restore review surface is unavailable"
            return false
        }
        item.editableRestoreEnabled = allowEditable === true
        return item.handleSidecarReady(action)
    }
}
