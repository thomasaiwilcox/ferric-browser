import QtQuick
import io.github.ferricbrowser 1.0

// A settings surface belongs to one live window/profile. Presentation and
// draft state stay here; BrowserUi retains validation and persistence policy.
Item {
    id: workspace
    required property var browserWindow
    required property var browserUi
    property var hostWindow: browserWindow
    property var state: workspace
    property bool transientProfile: false
    property string profileLabel: ""
    property string spellcheckStatus: ""
    property bool spellcheckDictionariesMissing: false
    property bool settingsVisible: false
    property bool settingsTemporary: false
    property string settingsSearch: ""
    property string settingsNotice: ""
    property string pendingUserscriptRemoval: ""
    property string noticeDetail: ""
    property bool noticeFailed: false
    property bool noticePending: false
    property var settingRequest: null
    property string noticeSource: ""
    readonly property bool settingPending: !!browserUi && browserUi.setting_action_state === "pending"
    onBrowserUiChanged: {
        settingRequest = null
        noticeSource = ""
        if (surface) surface.clearSettingDrafts()
        if (!browserUi) close()
    }
    readonly property bool active: state.settingsVisible || state.pendingUserscriptRemoval.length > 0
    signal opening
    signal closed
    signal installManifestRequested

    anchors.fill: parent
    z: 76

    function show(search) {
        if (!browserUi) return;
        if (!state.settingsVisible)
            opening();
        showNotice("");
        if (search !== undefined)
            state.settingsSearch = String(search);
        refresh();
        var operation = browserUi.userscript_install_state || "idle";
        if (settingRequest) {
            noticeSource = "setting";
            applySettingFeedback();
        } else if (["pending", "removing", "updating"].indexOf(operation) >= 0 || operation.indexOf("error:") === 0)
            applyScriptState(operation);
        state.settingsVisible = true;
        surface.focusInitialControl();
    }
    function close() {
        if (!state.settingsVisible)
            return;
        state.settingsVisible = false;
        closed();
    }
    function showNotice(message, detail, failed, pending) {
        state.settingsNotice = message;
        noticeDetail = detail || "";
        noticeFailed = failed === true;
        noticePending = pending === true;
    }
    function refresh() {
        if (!browserUi) return;
        browserUi.refresh_userscript_inventory();
        refreshSettings();
    }
    function refreshSettings() {
        if (!browserUi) return;
        var query = (state.settingsSearch || "").trim().toLowerCase();
        var visibleRows = [];
        var rows = browserUi.settings_rows;
        for (var index = 0; index < rows.length; ++index) {
            var row = rows[index];
            var searchable = [row.key, row.label, row.scope, row.apply].join(" ").toLowerCase();
            if (!query || searchable.indexOf(query) >= 0)
                visibleRows.push(row);
        }
        if (!settingsModel.replaceRows(visibleRows))
            showNotice("Settings couldn't be displayed. Refresh to try again.", "The settings row data was inconsistent.", true);
    }
    function settingLiteral(row, value) {
        if (row.type === "bool")
            return value ? "true" : "false";
        if (row.type === "number")
            return String(value);
        if (row.type === "languages") {
            var languages = String(value || "").split(",");
            var normalized = [];
            for (var index = 0; index < languages.length && index < 16; ++index) {
                var language = languages[index].trim();
                if (language.length > 0 && normalized.indexOf(language) < 0)
                    normalized.push(language);
            }
            return JSON.stringify(normalized);
        }
        return JSON.stringify(String(value));
    }
    function applySetting(row, value) {
        if (settingPending) return false;
        var previousId = browserUi.setting_action_id || "";
        var accepted = browserUi.set_runtime_setting(row.key, settingLiteral(row, value), state.settingsTemporary || transientProfile);
        if (trackSettingRequest(previousId, row.key, value, false, undefined)) return accepted;
        if (!accepted) {
            showNotice("Couldn't apply this value.", browserUi.runtime_setting_error, true);
            return false;
        }
        showNotice("Change requested for " + row.key + (state.settingsTemporary || transientProfile ? " for this window and session" : " for all regular profiles in this setup"));
        refreshSettings();
        return true;
    }
    function resetSetting(row) {
        if (settingPending) return false;
        var snapshot = surface.settingDraft(row.key);
        var previousId = browserUi.setting_action_id || "";
        var accepted = browserUi.unset_runtime_setting(row.key, state.settingsTemporary || transientProfile);
        if (trackSettingRequest(previousId, row.key, undefined, true, snapshot)) return accepted;
        if (!accepted) {
            showNotice("Couldn't reset this setting.", browserUi.runtime_setting_error, true);
            return false;
        }
        showNotice("Reset requested for " + row.key);
        surface.discardSettingDraft(row.key);
        refreshSettings();
        return true;
    }
    function trackSettingRequest(previousId, key, value, reset, snapshot) {
        var id = browserUi.setting_action_id || "";
        if (!id || id === previousId) return false;
        settingRequest = { id: id, key: key, value: value, reset: reset, snapshot: snapshot, settled: false };
        noticeSource = "setting";
        applySettingFeedback();
        return true;
    }
    function applySettingFeedback() {
        var request = settingRequest;
        if (!request || !browserUi || request.id !== browserUi.setting_action_id) return;
        var operation = browserUi.setting_action_state;
        if (["pending", "succeeded", "failed"].indexOf(operation) < 0) return;
        if (operation !== "pending" && !request.settled) {
            settingRequest = Object.assign({}, request, { settled: true });
            refreshSettings();
            surface.finishSettingOperation(request.key, request.value, request.reset, request.snapshot, operation === "succeeded");
        }
        if (noticeSource === "setting")
            showNotice(browserUi.setting_action_message, browserUi.setting_action_detail, operation === "failed", operation === "pending");
    }
    function scriptFailureDetail() {
        var operation = browserUi.userscript_install_state || "";
        return operation.indexOf("error:") === 0 ? operation.substring(6) : browserUi.status_text;
    }
    function installManifest(selected) {
        noticeSource = "script";
        var path = browserUi.local_file_path(selected || "");
        if (!path) {
            showNotice("Choose a local userscript manifest.", "", true);
            return false;
        }
        if (!browserUi.install_userscript_manifest(path)) {
            showNotice("The userscript couldn't be installed. Review the manifest and try again.", scriptFailureDetail(), true);
            return false;
        }
        showNotice("Installing userscript…", "", false, true);
        return true;
    }
    function confirmRemoveUserscript(name) {
        if (!name || transientProfile)
            return;
        state.pendingUserscriptRemoval = String(name);
        removal.open();
    }
    function userscriptRows() {
        if (!browserUi) return [];
        var names = browserUi.userscript_names;
        var enabled = browserUi.userscript_enabled_values;
        var pageWorld = browserUi.userscript_page_world_values;
        var actions = browserUi.userscript_action_counts;
        if (names.length !== enabled.length || names.length !== pageWorld.length || names.length !== actions.length)
            return [];
        var rows = [];
        for (var index = 0; index < names.length; ++index) {
            var count = Number(actions[index]);
            rows.push({
                name: String(names[index]),
                enabled: String(enabled[index]) === "true",
                page_world: String(pageWorld[index]) === "true",
                actions: isFinite(count) && count >= 0 ? Math.floor(count) : 0
            });
        }
        return rows;
    }
    function applyScriptState(operation) {
        if (noticeSource === "setting") return;
        if (["pending", "removing", "updating"].indexOf(operation) >= 0) {
            showNotice(operation === "pending" ? "Installing userscript…" : operation === "removing" ? "Removing userscript…" : "Updating userscript…", "", false, true);
        } else if (["installed", "removed", "enabled", "disabled"].indexOf(operation) >= 0) {
            surface.clearUserscriptOverrides();
            showNotice(operation === "installed" ? "Userscript installed" : operation === "removed" ? "Userscript removed" : operation === "enabled" ? "Userscript enabled. It applies on the next navigation." : "Userscript disabled. It applies on the next navigation.");
        } else if (operation.indexOf("error:") === 0) {
            surface.clearUserscriptOverrides();
            showNotice("Couldn't update the script. Try again.", operation.substring(6), true);
        } else if (operation === "idle") {
            surface.clearUserscriptOverrides();
            showNotice("");
        }
    }

    Connections {
        target: workspace.browserUi
        function onSettings_rowsChanged() {
            if (workspace.state.settingsVisible) workspace.refreshSettings();
        }
        function onSetting_action_stateChanged() { workspace.applySettingFeedback(); }
        function onUserscript_install_stateChanged() {
            workspace.applyScriptState(workspace.browserUi.userscript_install_state || "idle");
        }
        function onProfile_bootstrap_pendingChanged() {
            if (workspace.browserUi.profile_bootstrap_pending) {
                workspace.settingRequest = null;
                workspace.noticeSource = "";
                surface.clearSettingDrafts();
                workspace.close();
            }
        }
    }
    FerricSettingsModel {
        id: settingsModel
    }
    FerricSettings {
        id: surface
        enabled: !removal.active
        browserWindow: workspace.browserWindow
        hostWindow: workspace.hostWindow
        settingsVisible: workspace.state.settingsVisible
        temporary: workspace.state.settingsTemporary
        searchText: workspace.state.settingsSearch
        notice: workspace.state.settingsNotice
        noticeDetail: workspace.noticeDetail
        noticeFailed: workspace.noticeFailed
        noticePending: workspace.noticePending
        settingPending: workspace.settingPending
        pendingSettingKey: workspace.settingPending && workspace.settingRequest ? workspace.settingRequest.key : ""
        transientProfile: workspace.transientProfile
        profileLabel: workspace.profileLabel
        spellcheckStatus: workspace.spellcheckStatus
        spellcheckDictionariesMissing: workspace.spellcheckDictionariesMissing
        userscriptInventoryReady: !!workspace.browserUi && workspace.browserUi.userscript_inventory_ready
        userscriptInventoryPending: !!workspace.browserUi && workspace.browserUi.userscript_inventory_pending
        userscriptInventoryError: workspace.browserUi ? workspace.browserUi.userscript_inventory_error : ""
        userscriptRows: workspace.userscriptRows()
        settingsModel: settingsModel
        onCloseRequested: workspace.close()
        onRefreshRequested: workspace.refresh()
        onInstallUserscriptRequested: workspace.installManifestRequested()
        onTemporaryChangeRequested: function (temporary) {
            workspace.state.settingsTemporary = temporary;
            if (!workspace.settingPending)
                workspace.showNotice(temporary ? "Changes apply only to this window and session" : "Changes will be saved for all regular profiles in this setup");
        }
        onSearchChanged: function (text) {
            workspace.state.settingsSearch = text;
            workspace.refreshSettings();
        }
        onUserscriptEnabledRequested: function (name, enabled) {
            workspace.noticeSource = "script";
            var accepted = workspace.browserUi.set_userscript_enabled(name, enabled);
            surface.setUserscriptEnabled(name, accepted ? enabled : !enabled);
            if (accepted)
                workspace.showNotice("Updating userscript…", "", false, true);
            else
                workspace.showNotice("The userscript couldn't be changed. Try again.", workspace.scriptFailureDetail(), true);
        }
        onUserscriptRemovalRequested: function (name) {
            workspace.confirmRemoveUserscript(name);
        }
        onSettingApplyRequested: function (row, value) {
            var accepted = workspace.applySetting(row, value);
            surface.finishSettingRequest(row.key, value, accepted);
        }
        onSettingResetRequested: function (row) {
            workspace.resetSetting(row);
        }
    }
    FerricUserscriptRemovalDialog {
        id: removal
        browserWindow: workspace.browserWindow
        hostWindow: workspace.hostWindow
        scriptName: workspace.state.pendingUserscriptRemoval
        onConfirmed: {
            workspace.noticeSource = "script";
            var name = workspace.state.pendingUserscriptRemoval;
            workspace.state.pendingUserscriptRemoval = "";
            if (workspace.browserUi.remove_userscript(name))
                workspace.showNotice("Removing userscript…", "", false, true);
            else
                workspace.showNotice("The userscript couldn't be removed. Try again.", workspace.scriptFailureDetail(), true);
        }
        onCancelled: workspace.state.pendingUserscriptRemoval = ""
    }
}
