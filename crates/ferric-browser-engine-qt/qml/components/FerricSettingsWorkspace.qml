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
    readonly property bool active: state.settingsVisible || state.pendingUserscriptRemoval.length > 0
    signal opening
    signal closed
    signal installManifestRequested

    anchors.fill: parent
    z: 76

    function show(search) {
        if (!state.settingsVisible)
            opening();
        showNotice("");
        if (search !== undefined)
            state.settingsSearch = String(search);
        refresh();
        var operation = browserUi.userscript_install_state;
        if (["pending", "removing", "updating"].indexOf(operation) >= 0)
            applyScriptState(operation);
        state.settingsVisible = true;
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
        browserUi.refresh_userscript_inventory();
        refreshSettings();
    }
    function refreshSettings() {
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
        if (!browserUi.set_runtime_setting(row.key, settingLiteral(row, value), state.settingsTemporary || transientProfile)) {
            showNotice("This setting couldn't be applied. Review the value and try again.", browserUi.runtime_setting_error, true);
            return false;
        }
        showNotice("Change requested for " + row.key + (state.settingsTemporary || transientProfile ? " for this session" : " for this profile"));
        refreshSettings();
        return true;
    }
    function resetSetting(row) {
        if (!browserUi.unset_runtime_setting(row.key, state.settingsTemporary || transientProfile)) {
            showNotice("This setting couldn't be reset. Try again.", browserUi.runtime_setting_error, true);
            return false;
        }
        showNotice("Reset requested for " + row.key);
        refreshSettings();
        return true;
    }
    function installManifest(selected) {
        var path = browserUi.local_file_path(selected || "");
        if (!path) {
            showNotice("Choose a local userscript manifest.", "", true);
            return false;
        }
        if (!browserUi.install_userscript_manifest(path)) {
            showNotice("The userscript couldn't be installed. Review the manifest and try again.", browserUi.status_text, true);
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
        if (["pending", "removing", "updating"].indexOf(operation) >= 0) {
            showNotice(operation === "pending" ? "Installing userscript…" : operation === "removing" ? "Removing userscript…" : "Updating userscript…", "", false, true);
        } else if (["installed", "removed", "enabled", "disabled"].indexOf(operation) >= 0) {
            surface.clearUserscriptOverrides();
            showNotice(operation === "installed" ? "Userscript installed" : operation === "removed" ? "Userscript removed" : operation === "enabled" ? "Userscript enabled. It applies on the next navigation." : "Userscript disabled. It applies on the next navigation.");
        } else if (operation.indexOf("error:") === 0) {
            surface.clearUserscriptOverrides();
            showNotice("The userscript change couldn't be completed. Review the details and try again.", operation.substring(6), true);
        }
    }

    Connections {
        target: workspace.browserUi
        function onUserscript_install_stateChanged() {
            workspace.applyScriptState(workspace.browserUi.userscript_install_state || "idle");
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
        transientProfile: workspace.transientProfile
        profileLabel: workspace.profileLabel
        spellcheckStatus: workspace.spellcheckStatus
        spellcheckDictionariesMissing: workspace.spellcheckDictionariesMissing
        userscriptRows: workspace.userscriptRows()
        settingsModel: settingsModel
        onCloseRequested: workspace.close()
        onRefreshRequested: workspace.refresh()
        onInstallUserscriptRequested: workspace.installManifestRequested()
        onTemporaryChangeRequested: function (temporary) {
            workspace.state.settingsTemporary = temporary;
            workspace.showNotice(temporary ? "Changes apply only to this window and session" : "Changes will be saved for this profile");
        }
        onSearchChanged: function (text) {
            workspace.state.settingsSearch = text;
            workspace.refreshSettings();
        }
        onUserscriptEnabledRequested: function (name, enabled) {
            var accepted = workspace.browserUi.set_userscript_enabled(name, enabled);
            surface.setUserscriptEnabled(name, accepted ? enabled : !enabled);
            if (accepted)
                workspace.showNotice("Updating userscript…", "", false, true);
            else
                workspace.showNotice("The userscript couldn't be changed. Try again.", workspace.browserUi.status_text, true);
        }
        onUserscriptRemovalRequested: function (name) {
            workspace.confirmRemoveUserscript(name);
        }
        onSettingApplyRequested: function (row, value) {
            workspace.applySetting(row, value);
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
            var name = workspace.state.pendingUserscriptRemoval;
            workspace.state.pendingUserscriptRemoval = "";
            if (workspace.browserUi.remove_userscript(name))
                workspace.showNotice("Removing userscript…", "", false, true);
            else
                workspace.showNotice("The userscript couldn't be removed. Try again.", workspace.browserUi.status_text, true);
        }
        onCancelled: workspace.state.pendingUserscriptRemoval = ""
    }
}
