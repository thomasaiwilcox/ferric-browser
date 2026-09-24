import QtQuick

// Profile and session presentation share lifecycle polling, but do not own
// profile/session policy. The primary window remains the policy owner through
// the browserWindow boundary.
Item {
    id: surfaces

    required property var browserWindow
    required property var browserUi
    required property var profilesModel
    required property var namedSessionsModel
    readonly property var window: browserWindow

    // Presentation owns the controls and their cleanup.  The runtime surface
    // owns the command boundary and decides whether each intent is accepted.
    signal createProfileRequested(string name, string label)
    signal renameProfileRequested(string name, string label)
    signal openProfileRequested(string name, string label)
    signal deleteProfileRequested(string name)
    signal saveSessionRequested(string name)
    signal deleteSessionRequested(string name)

    function restartProfileRefresh() {
        profileRefreshTimer.restart()
    }

    function restartProfileDeletePreview() {
        profilePreviewTimer.restart()
    }

    function clearProfileCreateInputs() {
        profileManagerSurface.clearCreateInputs()
    }

    function clearProfileRenameInputs() {
        profileManagerSurface.clearRename()
    }

    Timer {
        id: profileRefreshTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (surfaces.browserUi.profile_values_pending) {
                surfaces.browserWindow.profileRefreshAttempts += 1
                if (surfaces.browserWindow.profileRefreshAttempts >= 20) {
                    stop()
                }
                return
            }
            surfaces.browserWindow.applyProfileValues()
            surfaces.browserWindow.profileRefreshAttempts += 1
            stop()
        }
    }

    Timer {
        id: profilePreviewTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (surfaces.browserUi.profile_preview_pending) {
                return
            }
            var preview = surfaces.browserUi.profile_preview_text
            if (preview.length === 0) {
                surfaces.browserWindow.profileDeletePreviewVisible = false
                surfaces.browserWindow.closeInternalSurface()
                stop()
                return
            }
            surfaces.browserWindow.profileDeletePreviewText = preview
            stop()
        }
    }

    FerricProfileManager {
        id: profileManagerSurface
        browserWindow: surfaces.browserWindow
        profilesModel: surfaces.profilesModel
        onCloseRequested: {
            surfaces.browserWindow.profileManagerVisible = false
            surfaces.browserWindow.closeInternalSurface()
        }
        onCreateRequested: function(name, label) {
            surfaces.createProfileRequested(name, label)
        }
        onRenameRequested: function(name, label) {
            surfaces.renameProfileRequested(name, label)
        }
        onOpenRequested: function(name, label) {
            surfaces.openProfileRequested(name, label)
        }
        onDeleteRequested: function(name) {
            surfaces.deleteProfileRequested(name)
        }
    }

    FerricProfileDeletePreview {
        browserWindow: surfaces.browserWindow
        onConfirmRequested: {
            surfaces.deleteProfileRequested(surfaces.window.profileDeleteName)
        }
        onCancelRequested: {
            surfaces.window.profileDeletePreviewVisible = false
            surfaces.window.closeInternalSurface()
        }
    }

    FerricSessionManager {
        browserWindow: surfaces.browserWindow
        sessionsModel: surfaces.namedSessionsModel
        onCloseRequested: {
            surfaces.browserWindow.sessionManagerVisible = false
            surfaces.browserWindow.closeInternalSurface()
        }
        onSaveRequested: function(name) {
            surfaces.saveSessionRequested(name)
        }
        onPreviewRequested: function(name) {
            surfaces.browserWindow.showSessionPreview(name, false)
        }
        onDeleteRequested: function(name, confirmed) {
            if (confirmed) {
                surfaces.deleteSessionRequested(name)
            } else {
                surfaces.browserWindow.pendingDeleteName = name
            }
        }
    }

    FerricSessionPreview {
        browserWindow: surfaces.browserWindow
        onCloseRequested: {
            surfaces.window.sessionPreviewVisible = false
            surfaces.window.sessionPreviewLoading = false
            surfaces.window.sessionPreviewError = false
            surfaces.window.closeInternalSurface()
        }
        onLoadRequested: function(append) {
            if (append) {
                surfaces.window.sessionPreviewAppend = true
            }
            surfaces.window.loadPreviewedSession()
        }
    }

    FerricReopenWindowConfirmation {
        browserWindow: surfaces.browserWindow
        onConfirmRequested: surfaces.window.confirmReopenWindow()
        onCancelRequested: surfaces.window.cancelReopenWindow()
    }
}
