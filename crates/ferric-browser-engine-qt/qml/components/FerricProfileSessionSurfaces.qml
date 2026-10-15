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

    function restartProfileDeletePreview() { profilePreviewTimer.restart() }
    function profileMutationAccepted(kind, name, label) {
        profileManagerSurface.mutationAccepted(browserUi.profile_action_id, kind, name, label)
    }
    function profileRequestRejected(message) { profileManagerSurface.requestRejected(message, browserUi.status_text) }
    function sessionSaveAccepted(name) { sessionManagerSurface.saveAccepted(browserUi.session_save_id, name) }
    function sessionDeleteAccepted() { sessionManagerSurface.deleteAccepted() }
    function sessionRequestRejected(message) { sessionManagerSurface.requestRejected(message, browserUi.status_text) }
    function returnToProfiles() {
        profilePreviewTimer.stop()
        window.profileDeletePreviewVisible = false
        window.profileManagerVisible = true
    }

    Connections {
        target: surfaces.browserUi
        function onProfile_valuesChanged() { window.applyProfileValues() }
        function onSession_names_valuesChanged() {
            window.applySessionValues()
            sessionManagerSurface.resetDeleteConfirmation()
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
                surfaces.returnToProfiles()
                profileManagerSurface.requestRejected("The profile folders couldn't be checked. Refresh and try again.", surfaces.browserUi.profile_preview_error)
                stop()
                return
            }
            stop()
        }
    }

    FerricProfileManager {
        id: profileManagerSurface
        browserWindow: surfaces.browserWindow
        profilesModel: surfaces.profilesModel
        listPending: surfaces.browserUi.profile_values_pending
        listError: surfaces.browserUi.profile_list_error
        feedback: ({ id: surfaces.browserUi.profile_action_id, state: surfaces.browserUi.profile_action_state,
                     message: surfaces.browserUi.profile_action_message, detail: surfaces.browserUi.profile_action_detail })
        onRefreshRequested: surfaces.window.refreshProfiles()
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
        previewPending: surfaces.browserUi.profile_preview_pending
        folderText: surfaces.browserUi.profile_preview_text
        onConfirmRequested: {
            surfaces.deleteProfileRequested(surfaces.window.profileDeleteName)
        }
        onCancelRequested: {
            surfaces.returnToProfiles()
        }
    }

    FerricSessionManager {
        id: sessionManagerSurface
        browserWindow: surfaces.browserWindow
        sessionsModel: surfaces.namedSessionsModel
        listPending: surfaces.browserUi.session_names_pending
        listError: surfaces.browserUi.session_names_error
        saveFeedback: ({ id: surfaces.browserUi.session_save_id, state: surfaces.browserUi.session_save_state,
                         message: surfaces.browserUi.session_save_message, detail: surfaces.browserUi.session_save_detail })
        deleteFeedback: ({ id: surfaces.browserUi.session_delete_id, state: surfaces.browserUi.session_delete_state,
                           message: surfaces.browserUi.session_delete_message, detail: surfaces.browserUi.session_delete_detail })
        onRefreshRequested: surfaces.window.refreshSessions()
        onCloseRequested: {
            surfaces.browserWindow.sessionManagerVisible = false
            surfaces.browserWindow.closeInternalSurface()
        }
        onSaveRequested: function(name) {
            surfaces.saveSessionRequested(name)
        }
        onPreviewRequested: function(name) {
            if (!surfaces.browserWindow.showSessionPreview(name, false))
                surfaces.sessionRequestRejected("This session couldn't be opened for preview. Try again.")
        }
        onDeleteRequested: function(name, confirmed) {
            if (confirmed) {
                surfaces.deleteSessionRequested(name)
            }
        }
    }

    FerricSessionPreview {
        browserWindow: surfaces.browserWindow
        savedTabs: {
            if (!surfaces.browserWindow.sessionPreviewVisible || surfaces.browserWindow.sessionPreviewLoading
                    || surfaces.browserWindow.sessionPreviewError) return []
            var labels = surfaces.browserUi.session_preview_labels || []
            var urls = surfaces.browserUi.session_preview_urls || []
            var pinned = surfaces.browserUi.session_preview_pinned || []
            var muted = surfaces.browserUi.session_preview_muted || []
            var rows = []
            for (var i = 0; i < labels.length; ++i)
                rows.push({label: String(labels[i]), url: String(urls[i] || ""),
                    pinned: String(pinned[i]) === "true", muted: String(muted[i]) === "true"})
            return rows
        }
        onCloseRequested: {
            surfaces.window.sessionPreviewVisible = false
            surfaces.window.sessionPreviewLoading = false
            surfaces.window.sessionPreviewError = false
            surfaces.window.sessionManagerVisible = true
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
