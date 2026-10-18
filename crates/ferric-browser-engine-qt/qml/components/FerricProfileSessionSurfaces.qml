import QtQuick

// Profile and session presentation share lifecycle polling, but do not own
// profile/session policy. The workspace supplies the requesting window and UI.
Item {
    id: surfaces
    enabled: !!browserUi && !!hostWindow && hostWindow.runtimeReleased !== true && hostWindow.runtimeDisposing !== true

    required property var browserWindow
    required property var browserUi
    property var hostWindow: browserWindow
    property var state: browserWindow
    property var controller: browserWindow
    property bool transientProfile: browserWindow.temporaryProfile === true || browserWindow.ephemeralProfile === true
    required property var profilesModel
    required property var namedSessionsModel
    readonly property var window: state

    // Presentation owns the controls and their cleanup.  The runtime surface
    // owns the command boundary and decides whether each intent is accepted.
    signal createProfileRequested(string name, string label)
    signal renameProfileRequested(string name, string label)
    signal openProfileRequested(string name, string label)
    signal deleteProfileRequested(string name)
    signal saveSessionRequested(string name)
    signal deleteSessionRequested(string name)

    function restartProfileDeletePreview() { profilePreviewTimer.restart() }
    function stopProfileDeletePreview() { profilePreviewTimer.stop() }
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
        function onProfile_valuesChanged() { surfaces.controller.applyProfileValues() }
        function onSession_names_valuesChanged() {
            surfaces.controller.applySessionValues()
            sessionManagerSurface.resetDeleteConfirmation()
        }
    }

    Timer {
        id: profilePreviewTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (!surfaces.state.profileDeletePreviewVisible) { stop(); return }
            if (!surfaces.browserUi) { stop(); return }
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
        hostWindow: surfaces.hostWindow
        state: surfaces.state
        profilesModel: surfaces.profilesModel
        listPending: !!surfaces.browserUi && surfaces.browserUi.profile_values_pending
        listError: surfaces.browserUi ? surfaces.browserUi.profile_list_error : ""
        feedback: surfaces.browserUi ? ({ id: surfaces.browserUi.profile_action_id, state: surfaces.browserUi.profile_action_state,
                     message: surfaces.browserUi.profile_action_message, detail: surfaces.browserUi.profile_action_detail }) : ({})
        onRefreshRequested: surfaces.controller.refreshProfiles()
        onCloseRequested: surfaces.controller.closeProfiles()
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
        hostWindow: surfaces.hostWindow
        state: surfaces.state
        previewPending: !!surfaces.browserUi && surfaces.browserUi.profile_preview_pending
        folderText: surfaces.browserUi ? surfaces.browserUi.profile_preview_text : ""
        onConfirmRequested: {
            surfaces.deleteProfileRequested(surfaces.window.profileDeleteName)
        }
        onCancelRequested: {
            surfaces.returnToProfiles()
        }
    }

    FerricSessionManager {
        id: sessionManagerSurface
        privateSession: surfaces.transientProfile
        browserWindow: surfaces.browserWindow
        hostWindow: surfaces.hostWindow
        state: surfaces.state
        sessionsModel: surfaces.namedSessionsModel
        listPending: !!surfaces.browserUi && surfaces.browserUi.session_names_pending
        listError: surfaces.browserUi ? surfaces.browserUi.session_names_error : ""
        saveFeedback: surfaces.browserUi ? ({ id: surfaces.browserUi.session_save_id, state: surfaces.browserUi.session_save_state,
                         message: surfaces.browserUi.session_save_message, detail: surfaces.browserUi.session_save_detail }) : ({})
        deleteFeedback: surfaces.browserUi ? ({ id: surfaces.browserUi.session_delete_id, state: surfaces.browserUi.session_delete_state,
                           message: surfaces.browserUi.session_delete_message, detail: surfaces.browserUi.session_delete_detail }) : ({})
        onRefreshRequested: surfaces.controller.refreshSessions()
        onCloseRequested: surfaces.controller.closeSessions()
        onSaveRequested: function(name) {
            surfaces.saveSessionRequested(name)
        }
        onPreviewRequested: function(name) {
            if (!surfaces.controller.showSessionPreview(name, false))
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
        hostWindow: surfaces.hostWindow
        state: surfaces.state
        savedTabs: {
            if (!surfaces.browserUi || !surfaces.state.sessionPreviewVisible || surfaces.state.sessionPreviewLoading
                    || surfaces.state.sessionPreviewError) return []
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
            surfaces.controller.loadPreviewedSession()
        }
    }

    FerricReopenWindowConfirmation {
        browserWindow: surfaces.browserWindow
        hostWindow: surfaces.hostWindow
        state: surfaces.state
        onConfirmRequested: surfaces.controller.confirmReopenWindow()
        onCancelRequested: surfaces.controller.cancelReopenWindow()
    }
}
