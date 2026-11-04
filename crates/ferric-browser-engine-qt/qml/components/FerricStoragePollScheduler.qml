import QtQuick

// One primary-window timer; ownership and completion processing stay with each UI.
Timer {
    id: scheduler
    property bool enabled: false
    property var entries: function() { return [] }
    signal polled(bool consumed)
    interval: 1000
    repeat: true
    running: enabled

    function wake() {
        if (enabled && (!running || interval > 1)) {
            interval = 1
            restart()
        }
    }

    onTriggered: {
        var owners = entries() || []
        var consumed = false
        var nextDelay = 1000
        for (var entry of owners) {
            if (entry && entry.ui && !entry.privateProfile && !entry.ephemeralProfile) {
                consumed = entry.ui.poll_storage_library() || consumed
                nextDelay = Math.min(nextDelay, entry.ui.storage_library_delay_ms())
            }
        }
        interval = nextDelay
        polled(consumed)
    }
}
