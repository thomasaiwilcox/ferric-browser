import QtQuick

// Per-window presentation and request lifecycle; BrowserUi owns validation,
// storage and profile/session policy.
Item {
    id: workspace
    required property var browserWindow
    required property var hostWindow
    required property var browserUi
    property var state: workspace
    property var profilesModel: profileRows
    property var namedSessionsModel: sessionRows
    property bool transientProfile: false
    property bool profileManagerVisible: false
    property bool profileDeletePreviewVisible: false
    property string profileDeleteName: ""
    property bool sessionManagerVisible: false
    property bool sessionPreviewVisible: false
    property bool restorePending: false
    property bool sessionPreviewLoading: false
    property bool sessionPreviewError: false
    property string sessionPreviewName: ""
    property bool sessionPreviewAppend: false
    property string sessionPreviewText: ""
    property bool reopenWindowConfirmationVisible: false
    readonly property bool active: state.profileManagerVisible || state.profileDeletePreviewVisible
        || state.sessionManagerVisible || state.sessionPreviewVisible
    signal opening
    signal closed
    anchors.fill: parent
    z: 50

    ListModel { id: profileRows }
    ListModel { id: sessionRows }
    function applyProfileValues() {
        profilesModel.clear()
        var lines = String(browserUi.profile_values || "").split("\n")
        for (var i = 0; i < lines.length; ++i) {
            var fields = lines[i].split("\t")
            if (fields.length >= 3) profilesModel.append({name:fields[0],label:fields[1],id:fields[2]})
        }
    }
    function refreshProfiles() { browserUi.request_profile_list(); applyProfileValues() }
    function applySessionValues() {
        namedSessionsModel.clear()
        var names = browserUi.session_names_values || []
        for (var i = 0; i < names.length; ++i) namedSessionsModel.append({name:String(names[i])})
    }
    function refreshSessions() { browserUi.list_named_sessions(); applySessionValues() }
    function showProfiles() { refreshProfiles(); opening(); state.profileManagerVisible = true }
    function showSessions() { refreshSessions(); opening(); state.sessionManagerVisible = true }
    function closeProfiles() {
        surfaces.stopProfileDeletePreview()
        state.profileManagerVisible = false
        state.profileDeletePreviewVisible = false
        closed()
    }
    function closeSessions() {
        state.sessionManagerVisible = false
        state.sessionPreviewVisible = false
        state.sessionPreviewLoading = false
        closed()
    }
    function showProfileDeletePreview(name) {
        if (!browserUi.request_profile_delete_preview(name)) return false
        state.profileDeleteName = name
        opening()
        state.profileManagerVisible = false
        state.profileDeletePreviewVisible = true
        surfaces.restartProfileDeletePreview()
        return true
    }
    function showSessionPreview(name, append) {
        if (!browserUi.request_session_preview(name)) return false
        state.sessionPreviewName = name
        state.sessionPreviewAppend = append
        state.sessionPreviewLoading = true
        state.sessionPreviewError = false
        state.sessionPreviewText = "Reading saved tabs…"
        opening()
        state.sessionManagerVisible = false
        state.sessionPreviewVisible = true
        return true
    }
    function presentCommandRequest() {
        var request = browserUi.take_session_preview_request()
        if (request.length !== 2) return
        state.sessionPreviewName = String(request[0])
        state.sessionPreviewAppend = String(request[1]) === "true"
        state.sessionPreviewLoading = true
        state.sessionPreviewError = false
        state.sessionPreviewText = "Reading saved tabs…"
        opening()
        state.sessionPreviewVisible = true
    }
    function loadPreviewedSession() {
        if (state.sessionPreviewError || !browserUi.request_named_session_load(
                    state.sessionPreviewName, state.sessionPreviewAppend)) return false
        restorePending = true
        state.sessionPreviewLoading = true
        state.sessionPreviewError = false
        state.sessionPreviewText = "Restoring saved tabs…"
        return true
    }
    // Called only after the tab controller has materialized the restore result.
    function finishSessionRestore(recovery) {
        if (!restorePending && recovery !== true) return
        var hadPreview = state.sessionPreviewVisible || state.sessionManagerVisible
        restorePending = false
        state.sessionPreviewLoading = false
        state.sessionPreviewVisible = false
        state.sessionManagerVisible = false
        state.sessionPreviewError = false
        if (hadPreview) closed()
    }
    function confirmReopenWindow() { hostWindow.confirmReopenWindow() }
    function cancelReopenWindow() { hostWindow.cancelReopenWindow() }
    Timer {
        id: sessionPreviewPollTimer
        interval: 50
        repeat: true
        running: !!workspace.browserUi && !!workspace.hostWindow && workspace.state.sessionPreviewLoading
            && workspace.hostWindow.runtimeReleased !== true && workspace.hostWindow.runtimeDisposing !== true
        onTriggered: {
            var preview = workspace.browserUi.take_session_preview()
            if (preview.length === 0) return
            workspace.state.sessionPreviewError = preview.indexOf("ERROR\t") === 0
            workspace.state.sessionPreviewText = workspace.state.sessionPreviewError ? preview.slice(6) : preview
            workspace.state.sessionPreviewLoading = false
        }
    }
    FerricProfileSessionSurfaces {
        id: surfaces
        anchors.fill: parent
        browserWindow: workspace.browserWindow
        hostWindow: workspace.hostWindow
        controller: workspace
        state: workspace.state
        browserUi: workspace.browserUi
        profilesModel: workspace.profilesModel
        namedSessionsModel: workspace.namedSessionsModel
        transientProfile: workspace.transientProfile
        onCreateProfileRequested: function(name, label) {
            if (workspace.browserUi.create_profile(name, label))
                surfaces.profileMutationAccepted("create", name, label)
            else surfaces.profileRequestRejected("This profile couldn't be created. Review the name and try again.")
        }
        onRenameProfileRequested: function(name, label) {
            if (workspace.browserUi.rename_profile(name, label))
                surfaces.profileMutationAccepted("rename", name, label)
            else surfaces.profileRequestRejected("This profile couldn't be renamed. Try again.")
        }
        onOpenProfileRequested: function(name, label) {
            if (!workspace.browserUi.request_profile_window(name))
                surfaces.profileRequestRejected("This profile couldn't be opened. Try again.")
        }
        onDeleteProfileRequested: function(name) {
            if (workspace.state.profileDeletePreviewVisible) {
                if (workspace.browserUi.delete_profile(workspace.state.profileDeleteName, true))
                    surfaces.profileMutationAccepted("delete", name, "")
                else surfaces.profileRequestRejected("This profile couldn't be deleted. Review the details and try again.")
                surfaces.returnToProfiles()
            } else if (!workspace.showProfileDeletePreview(name))
                surfaces.profileRequestRejected("The profile folders couldn't be checked. Try again.")
        }
        onSaveSessionRequested: function(name) {
            if (workspace.browserUi.save_named_session(name)) surfaces.sessionSaveAccepted(name)
            else surfaces.sessionRequestRejected("This session couldn't be saved. Review the name and try again.")
        }
        onDeleteSessionRequested: function(name) {
            if (workspace.browserUi.delete_named_session(name, true)) surfaces.sessionDeleteAccepted()
            else surfaces.sessionRequestRejected("This session couldn't be deleted. Try again.")
        }
    }
}
