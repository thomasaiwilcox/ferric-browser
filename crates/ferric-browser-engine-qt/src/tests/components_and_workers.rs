#[test]
fn configured_editor_argv_replaces_only_the_complete_file_argument() {
    let config = serde_json::json!({
        "tools": {"editor": ["foot", "-e", "nvim", "{file}"]}
    });
    let (executable, arguments) =
        configured_editor_argv(&config, Path::new("/tmp/ferric-browser-config.toml"))
            .expect("configured editor argv");
    assert_eq!(executable, "foot");
    assert_eq!(
        arguments,
        vec!["-e", "nvim", "/tmp/ferric-browser-config.toml"]
    );

    let invalid = serde_json::json!({"tools": {"editor": ["nvim", "--cmd={file}"]}});
    assert!(configured_editor_argv(&invalid, Path::new("/tmp/config")).is_err());

    let oversized = "x".repeat(MAX_UNTRUSTED_ARGUMENT_BYTES + 1);
    let invalid = serde_json::json!({"tools": {"editor": [oversized, "{file}"]}});
    assert!(configured_editor_argv(&invalid, Path::new("/tmp/config")).is_err());
}
#[test]
fn config_editor_captures_bounded_stderr() {
    let io_source = include_str!("../userscript_io.rs");
    let process_source = include_str!("../editor_process.rs");
    assert!(io_source.contains("userscript pipe read failed"));
    assert!(io_source.contains("userscript output exceeds"));
    assert!(process_source.contains("exit_status: Option<ExitStatus>"));
}

#[test]
fn external_editor_rejects_rich_contenteditable_controls() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/scripts/BrowserScripts.js"),
    ]
    .concat();
    assert!(qml.contains("e.contentEditable==='plaintext-only'"));
    assert!(qml.contains("return {error:'focused control is not a supported plain-text editor'}"));
    assert!(qml.contains("else e.textContent=next"));
    assert!(!qml.contains("if(e.isContentEditable)return {ok:true"));
}

#[test]
fn primary_web_engine_profile_uses_selected_profile_namespace() {
    let qml = QML_SOURCE;
    assert!(qml.contains(
        "storageName: window.temporaryProfile ? \"\" : \"ferric-browser-\" + window.profileName"
    ));
    assert!(
        !qml.contains("storageName: window.temporaryProfile ? \"\" : \"ferric-browser-default\"")
    );
    assert!(qml.contains(
            "storageName: secondaryWindow.windowTransientProfile ? \"\" : \"ferric-browser-\" + secondaryWindow.windowProfileName"
        ));
    assert!(qml.contains("window.storageBasePath + \"/webengine/\" + window.profileName"));
    assert!(qml.contains("secondaryWindow.windowStorageBasePath + \"/webengine/\""));
    assert!(qml.contains("property string windowStorageBasePath: rootWindow.storageBasePath"));
}

#[test]
fn startup_profile_overrides_reach_the_browser_ui() {
    let qml = QML_SOURCE;
    assert!(qml.contains("property string startupProfileOverridesJson: \"{}\""));
    assert!(qml.contains("browserUi.set_startup_configuration("));
    assert!(qml.contains("window.startupProfileOverridesJson"));
    assert!(!qml.contains("browserUi.profile_overrides_json ="));
}

#[test]
fn command_completion_popup_has_accessible_popup_semantics() {
    let qml = QML_SOURCE;
    let popup = include_str!("../../qml/components/FerricCommandLine.qml");
    assert!(qml.contains("FerricCommandLine {"));
    assert!(popup.contains("Accessible.role: Accessible.PopupMenu"));
    assert!(popup.contains("Accessible.name: \"Command completion popup\""));
    assert!(popup.contains("Accessible.description:"));
    assert!(popup.contains("? commandSurface.browserWindow.selectionColor"));
    assert!(popup.contains("? commandSurface.browserWindow.selectionTextColor"));
    assert!(popup.contains("commandSurface.browserWindow.readableTextColor("));
    assert!(!popup.contains("browserUi."));
}

#[test]
fn ipc_permission_commands_use_typed_origin_and_permission_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "permissions",
        "arguments": {"origin": "https://example.test"}
    }))
    .expect("typed permissions command");
    assert_eq!(command.arguments, vec!["https://example.test"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "permission-reset",
        "arguments": {
            "origin": "https://example.test",
            "permission": "notifications"
        }
    }))
    .expect("typed permission reset command");
    assert_eq!(
        command.arguments,
        vec!["https://example.test", "notifications"]
    );
}

#[test]
fn ipc_site_status_accepts_an_optional_stable_tab_target() {
    let (active, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-status",
        "arguments": {}
    }))
    .expect("active site status command");
    assert!(active.arguments.is_empty());

    let (targeted, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-status",
        "arguments": {"tab": "tab-42"}
    }))
    .expect("targeted site status command");
    assert_eq!(targeted.arguments, vec!["--tab", "tab-42"]);

    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "site-status",
            "arguments": {"tab": "bad\nid"}
        }))
        .is_err()
    );
}

#[test]
fn permission_session_key_is_scoped_to_profile_uuid_and_lifetime() {
    let first_profile = Uuid::from_u128(1);
    let second_profile = Uuid::from_u128(2);
    let first = permission_session_key(first_profile, "https://example.test", "camera");
    let same = permission_session_key(first_profile, "https://example.test", "camera");
    let other_profile = permission_session_key(second_profile, "https://example.test", "camera");

    assert_eq!(first, same);
    assert_ne!(first, other_profile);
    assert_eq!(first.lifetime, PermissionLifetimeKey::ProfileSession);
}

#[test]
fn ipc_site_doctor_commands_use_typed_experiment_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "blocking-bypass"}
    }))
    .expect("typed Site Doctor command");
    assert_eq!(command.arguments, vec!["blocking-bypass"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "userscripts-off"}
    }))
    .expect("typed userscript experiment command");
    assert_eq!(command.arguments, vec!["userscripts-off"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "fresh-view"}
    }))
    .expect("typed fresh-view experiment command");
    assert_eq!(command.arguments, vec!["fresh-view"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "compiled-defaults"}
    }))
    .expect("typed compiled-default experiment command");
    assert_eq!(command.arguments, vec!["compiled-defaults"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor-undo",
        "arguments": {"id": "site-experiment-123"}
    }))
    .expect("typed Site Doctor undo command");
    assert_eq!(command.arguments, vec!["site-experiment-123"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-data-clear",
        "arguments": {"origin": "https://example.test", "confirmed": true}
    }))
    .expect("typed site-data clear command");
    assert_eq!(command.arguments, vec!["https://example.test", "--confirm"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "site-doctor",
            "arguments": {"kind": "blocking-bypass"}
        }))
        .is_err()
    );
}

#[test]
fn page_focus_requires_a_recent_user_gesture_before_insert_mode() {
    let qml = QML_SOURCE;
    let script = include_str!("../../qml/scripts/BrowserScripts.js");
    assert!(script.contains("userGestureUntil=performance.now()+1500"));
    assert!(script.contains("__ferric_browserAuthorizeExplicitFocus"));
    assert!(script.contains("user_activated:!!userActivated"));
    assert!(qml.contains("!!state.user_activated"));
    assert!(script.contains("publish(false);})();"));
}

#[test]
fn hint_collection_accepts_qt_variant_candidate_lists() {
    let qml = QML_SOURCE;
    assert!(qml.contains("value && value.candidates !== undefined"));
    assert!(!qml.contains("Array.isArray(value.candidates)"));
}

#[test]
fn site_ledger_component_renders_data_and_emits_bridge_intents() {
    let qml = include_str!("../../qml/components/FerricSiteLedger.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal activeOriginDataClearRequested()"));
    assert!(qml.contains("signal siteDoctorExperimentRequested(string kind)"));
    assert!(qml.contains("signal sanitizedReportCopyRequested(bool includeHost)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("browserUi:"));
    assert!(composition_root.contains("FerricSiteLedger {"));
    assert!(!composition_root.contains("id: legacySiteLedger"));
}

#[test]
fn diagnostics_component_is_intent_only_and_replaces_the_inline_surface() {
    let qml = include_str!("../../qml/components/FerricDiagnostics.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal copyRequested()"));
    assert!(qml.contains("signal saveRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricDiagnostics {"));
    assert!(!composition_root.contains("id: diagnosticsSurface"));
}

#[test]
fn binding_help_component_renders_rows_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricBindingHelp.qml");
    let composition_root = QML_SOURCE;
    let projection = include_str!("../binding_presentation.rs");
    assert!(qml.contains("signal searchChanged(string text)"));
    assert!(qml.contains("bindingHelp.browserWindow.bindingHelpRows"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricBindingHelp {"));
    assert!(!composition_root.contains("id: bindingHelpSurface"));
    assert!(composition_root.contains("browserUi.refresh_binding_help(window.bindingHelpSearch)"));
    assert!(composition_root.contains("browserUi.binding_help_row_kinds"));
    assert!(!composition_root.contains("bindings_json"));
    assert!(!composition_root.contains("bindingHelpData"));
    assert!(!projection.contains("ipc_bindings_query"));
    assert!(ADAPTER_SOURCE.contains("#[qproperty(QStringList, binding_help_row_kinds)]"));
}

#[test]
fn profile_delete_preview_component_only_emits_confirmation_intents() {
    let qml = include_str!("../../qml/components/FerricProfileDeletePreview.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal confirmRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricProfileDeletePreview {"));
    assert!(!composition_root.contains("id: profileDeletePreview"));
    assert!(composition_root.contains("browserUi.delete_profile(window.profileDeleteName, true)"));
}

#[test]
fn reopen_window_confirmation_component_only_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricReopenWindowConfirmation.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal confirmRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricReopenWindowConfirmation {"));
    assert!(!composition_root.contains("id: reopenWindowConfirmation"));
    assert!(composition_root.contains("window.confirmReopenWindow()"));
    assert!(composition_root.contains("window.cancelReopenWindow()"));
}

#[test]
fn session_preview_component_renders_state_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricSessionPreview.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal closeRequested()"));
    assert!(qml.contains("signal loadRequested(bool append)"));
    assert!(qml.contains("sessionPreviewLoading"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricSessionPreview {"));
    assert!(!composition_root.contains("id: sessionPreview\n"));
    assert!(composition_root.contains("window.loadPreviewedSession()"));
}

#[test]
fn session_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricSessionManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var sessionsModel"));
    assert!(qml.contains("signal saveRequested(string name)"));
    assert!(qml.contains("signal deleteRequested(string name, bool confirmed)"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricSessionManager {"));
    assert!(!composition_root.contains("id: sessionManager\n"));
    assert!(composition_root.contains("browserUi.save_named_session(name)"));
    assert!(composition_root.contains("browserUi.delete_named_session(name, true)"));
}

#[test]
fn link_preview_component_renders_preview_data_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricLinkPreview.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var previewUi"));
    assert!(qml.contains("link_preview_original"));
    assert!(!qml.contains("previewData"));
    assert!(qml.contains("signal navigationConfirmed()"));
    assert!(qml.contains("preview.previewUi.link_preview_removed_parameters"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricLinkPreview {"));
    assert!(!composition_root.contains("id: linkPreview\n"));
    assert!(composition_root.contains("browserUi.confirm_link_navigation()"));
}

#[test]
fn library_preview_components_render_models_and_emit_root_owned_intents() {
    let journey = include_str!("../../qml/components/FerricJourneyExportPreview.qml");
    let transfer = include_str!("../../qml/components/FerricPrivateHistoryTransfer.qml");
    let manager = include_str!("../../qml/components/FerricLibraryManager.qml");
    let composition_root = QML_SOURCE;
    assert!(journey.contains("signal chooseFileRequested()"));
    assert!(!journey.contains("fileDialogSurfaces."));
    assert!(!journey.contains("browserUi."));
    assert!(transfer.contains("required property var profilesModel"));
    assert!(transfer.contains("signal reopenRequested(string profileName)"));
    assert!(!transfer.contains("browserUi."));
    assert!(manager.contains("FerricJourneyExportPreview {"));
    assert!(manager.contains("FerricPrivateHistoryTransfer {"));
    assert!(!composition_root.contains("id: journeyExportPreviewSurface"));
    assert!(!composition_root.contains("id: privateHistoryTransferSurface"));
    assert!(composition_root.contains("fileDialogSurfaces.openJourneyExport()"));
}

#[test]
fn download_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricDownloadManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var downloadsModel"));
    assert!(qml.contains("signal openRequested(string downloadId, bool reveal)"));
    assert!(qml.contains("signal actionRequested(string downloadId, string action)"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricDownloadManager {"));
    assert!(!composition_root.contains("id: downloadManager\n"));
    assert!(composition_root.contains("window.requestDownloadAction(downloadId, action)"));
}

#[test]
fn profile_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricProfileManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var profilesModel"));
    assert!(qml.contains("signal createRequested(string name, string label)"));
    assert!(qml.contains("function clearCreateInputs()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricProfileManager {"));
    assert!(!composition_root.contains("id: profileManager\n"));
    assert!(composition_root.contains("browserUi.create_profile(name, label)"));
    assert!(composition_root.contains("browserUi.rename_profile(name, label)"));
}

#[test]
fn rapid_hint_confirmation_component_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricRapidHintConfirmation.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal continueRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricRapidHintConfirmation {"));
    assert!(!composition_root.contains("id: rapidHintConfirmation"));
    assert!(composition_root.contains("browserUi.confirm_rapid_hint_tabs()"));
}

#[test]
fn hint_overlay_component_renders_candidates_and_emits_root_owned_intents() {
    let qml = include_str!("../../qml/components/FerricHintOverlay.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var hintResults"));
    assert!(qml.contains("signal activationRequested(string label)"));
    assert!(qml.contains("signal actionsRequested(string label)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("activateHint("));
    assert!(composition_root.contains("FerricHintOverlay {"));
    assert!(composition_root.contains("window.activateHint(label)"));
    assert!(composition_root.contains("window.showHintActions(label)"));
}

#[test]
fn userscript_inventory_component_keeps_enable_rollback_at_the_root_boundary() {
    let qml = include_str!("../../qml/components/FerricUserscriptInventory.qml");
    let settings = include_str!("../../qml/components/FerricSettings.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal enabledRequested(string name, bool enabled)"));
    assert!(qml.contains("function setEnabled(name, enabled)"));
    assert!(!qml.contains("browserUi."));
    assert!(settings.contains("FerricUserscriptInventory {"));
    assert!(!composition_root.contains("id: userscriptList"));
    assert!(composition_root.contains("browserUi.set_userscript_enabled(name, enabled)"));
    assert!(settings.contains("userscriptInventory.setEnabled(name, enabled)"));
    assert!(composition_root.contains("settingsSurface.setUserscriptEnabled(name, !enabled)"));
}

#[test]
fn setting_rows_component_emits_intents_without_reaching_the_runtime_bridge() {
    let qml = include_str!("../../qml/components/FerricSettingRows.qml");
    let settings = include_str!("../../qml/components/FerricSettings.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal applyRequested(var row, var value)"));
    assert!(qml.contains("signal resetRequested(var row)"));
    assert!(qml.contains("settingRows.applyRequested(settingRow.rowData, checked)"));
    assert!(qml.contains("settingRows.resetRequested(settingRow.rowData)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("window.applySetting"));
    assert!(settings.contains("FerricSettingRows {"));
    assert!(!composition_root.contains("id: settingsList"));
    assert!(composition_root.contains("window.applySetting(row, value)"));
    assert!(composition_root.contains("window.resetSetting(row)"));
}

#[test]
fn settings_component_composes_schema_rows_and_keeps_mutations_at_the_root() {
    let qml = include_str!("../../qml/components/FerricSettings.qml");
    let rows = include_str!("../../qml/components/FerricSettingRows.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("FerricUserscriptInventory {"));
    assert!(qml.contains("FerricSettingRows {"));
    assert!(qml.contains("signal userscriptEnabledRequested(string name, bool enabled)"));
    assert!(qml.contains("signal settingApplyRequested(var row, var value)"));
    // Characterization: QML generates temporaryChanged for the temporary
    // property, so the extracted component must use a distinct intent signal.
    assert!(qml.contains("signal temporaryChangeRequested(bool temporary)"));
    assert!(!qml.contains("signal temporaryChanged(bool temporary)"));
    assert!(composition_root.contains("onTemporaryChangeRequested:"));
    assert!(qml.contains("function setUserscriptEnabled(name, enabled)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("set_runtime_setting"));
    assert!(qml.contains("required property var settingsModel"));
    assert!(rows.contains("required property var settingsModel"));
    assert!(rows.contains("model: settingRows.settingsModel"));
    assert!(rows.contains("options: options"));
    assert!(rows.contains("rowData.options"));
    assert!(composition_root.contains("FerricSettings {"));
    assert!(composition_root.contains("FerricSettingsModel { id: settingsModel }"));
    assert!(composition_root.contains("settingsModel.replaceRows("));
    assert!(!composition_root.contains("JSON.parse(browserUi.config_json)"));
    assert!(!composition_root.contains("function settingConfigValue("));
    assert!(composition_root.contains("browserUi.set_userscript_enabled(name, enabled)"));
    assert!(composition_root.contains("settingsSurface.setUserscriptEnabled(name, !enabled)"));
    assert!(composition_root.contains("window.applySetting(row, value)"));
}

#[test]
fn journey_graph_component_only_renders_root_owned_graph_models() {
    let qml = include_str!("../../qml/components/FerricJourneyGraph.qml");
    let manager = include_str!("../../qml/components/FerricLibraryManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("required property var graphNodes"));
    assert!(qml.contains("required property var graphEntries"));
    assert!(qml.contains("required property var lineData"));
    assert!(qml.contains("function requestPaint()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricJourneyGraph {"));
    assert!(manager.contains("graphNodes: libraryManager.graphNodes"));
    assert!(manager.contains("graphEntries: libraryManager.graphEntries"));
    assert!(composition_root.contains("libraryManager.requestGraphPaint()"));
}

#[test]
fn switcher_results_component_keeps_navigation_and_dispatch_at_the_root_boundary() {
    let qml = include_str!("../../qml/components/FerricSwitcherResults.qml");
    let switcher = include_str!("../../qml/components/FerricSwitcher.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal activationRequested(int index)"));
    assert!(qml.contains("signal actionRequested(int index, string action)"));
    assert!(qml.contains("function moveBy(delta)"));
    assert!(qml.contains("function moveToBeginning()"));
    assert!(qml.contains("function moveToEnd()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("activateSwitcher("));
    assert!(switcher.contains("FerricSwitcherResults {"));
    assert!(switcher.contains("switcherResultsList.moveBy(8)"));
    assert!(composition_root.contains("window.activateSwitcher(index)"));
    assert!(composition_root.contains("window.activateSwitcherAction(index, action)"));
}

#[test]
fn switcher_component_exposes_focus_and_query_without_runtime_policy() {
    let qml = include_str!("../../qml/components/FerricSwitcher.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("property alias query: switcherInput.text"));
    assert!(qml.contains("function focusInput()"));
    // Characterization: the query alias already owns queryChanged; the search
    // edit is a separate user intent emitted toward the composition boundary.
    assert!(qml.contains("signal queryChangeRequested(string query)"));
    assert!(!qml.contains("signal queryChanged(string query)"));
    assert!(qml.contains("signal activationRequested(int index)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricSwitcher {"));
    assert!(composition_root.contains("switcherSurface.query = query || \"\""));
    assert!(composition_root.contains("switcherSurface.focusInput()"));
    assert!(composition_root.contains("onQueryChangeRequested:"));
}

#[test]
fn command_line_component_emits_editing_intents_without_executing_commands() {
    let qml = include_str!("../../qml/components/FerricCommandLine.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal completionUpdateRequested(string text, int cursorPosition)"));
    assert!(qml.contains("signal submitted(string text)"));
    assert!(qml.contains("signal completionMoveRequested(int delta)"));
    assert!(qml.contains("function focusInput()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricCommandLine {"));
    assert!(composition_root.contains("browserUi.update_completion(text, cursorPosition)"));
    assert!(composition_root.contains("browserUi.execute_command(text)"));
    assert!(composition_root.contains("browserUi.completion_move(delta)"));
}

#[test]
fn search_bar_component_emits_intents_without_page_or_engine_policy() {
    let qml = include_str!("../../qml/components/FerricSearchBar.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal searchChanged(string text)"));
    assert!(qml.contains("signal nextRequested(bool backward)"));
    assert!(qml.contains("function focusInput()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_ui_action"));
    assert!(composition_root.contains("FerricSearchBar {"));
    assert!(composition_root.contains("browserUi.search_changed(text)"));
    assert!(composition_root.contains("browserUi.search_next(browserUi.search_backward)"));
    assert!(composition_root.contains("browserUi.execute_ui_action("));
}

#[test]
fn journey_search_component_emits_queries_without_constructing_commands() {
    let qml = include_str!("../../qml/components/FerricJourneySearch.qml");
    let manager = include_str!("../../qml/components/FerricLibraryManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal searchRequested(string text)"));
    assert!(qml.contains("signal clearRequested()"));
    assert!(qml.contains("signal currentRequested()"));
    assert!(qml.contains("signal allRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricJourneySearch {"));
    assert!(
        composition_root.contains("window.runJourneyQuery(window.journeySearchArgument(text))")
    );
    assert!(!composition_root.contains("journeySearchField.text ="));
}

#[test]
fn library_entries_component_preserves_row_state_and_emits_root_owned_operations() {
    let qml = include_str!("../../qml/components/FerricLibraryEntries.qml");
    let manager = include_str!("../../qml/components/FerricLibraryManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("signal openRequested(string entryKind, string entryId)"));
    assert!(qml.contains("signal editRequested(string entryKind, string entryId, string value)"));
    assert!(qml.contains("signal deleteRequested(string entryKind, string entryId)"));
    assert!(qml.contains("signal journeyReopenRequested(string nodeId, string target)"));
    assert!(qml.contains("signal journeyExpandRequested(string nodeId)"));
    assert!(qml.contains("function markEditSaved(entryKind, entryId)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricLibraryEntries {"));
    assert!(composition_root.contains("window.editLibraryEntry(entryKind, entryId, value)"));
    assert!(composition_root.contains("libraryManager.markEditSaved(entryKind, entryId)"));
    assert!(composition_root.contains("window.deleteLibraryEntry(entryKind, entryId)"));
    assert!(composition_root.contains("browserUi.execute_command(command)"));
}

#[test]
fn library_manager_composes_presentation_and_forwards_every_operation_to_the_root() {
    let qml = include_str!("../../qml/components/FerricLibraryManager.qml");
    let composition_root = QML_SOURCE;
    assert!(qml.contains("FerricJourneySearch {"));
    assert!(qml.contains("FerricLibraryEntries {"));
    assert!(qml.contains("FerricJourneyGraph {"));
    assert!(
        qml.contains("signal entryEditRequested(string entryKind, string entryId, string value)")
    );
    assert!(qml.contains("signal journeyReopenRequested(string nodeId, string target)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricLibraryManager {"));
    assert!(composition_root.contains("onJourneyReopenRequested: function(nodeId, target)"));
    assert!(
        composition_root
            .contains("onPrivateHistoryBookmarkRequested: function(profileName, profileLabel)")
    );
}

#[test]
fn all_hints_cover_qutebrowser_control_families_and_activate_controls() {
    let qml = QML_SOURCE;
    let script = include_str!("../../qml/scripts/BrowserScripts.js");
    for selector in [
        "input:not([type='hidden'])",
        "summary",
        "[onclick]",
        "[role='checkbox']",
        "[role='menuitem']",
        "[aria-haspopup]",
        "[tabindex]:not([tabindex='-1'])",
    ] {
        assert!(
            script.contains(selector),
            "missing hint selector {selector}"
        );
    }
    assert!(qml.contains("function hintClickScript(elementId)"));
    assert!(qml.contains("result.action === \"click\""));
    assert!(script.contains("window.__ferric_browserHintElements=elements"));
    assert!(script.contains("record&&record.element"));
    assert!(script.contains("el.click();return true"));
}

#[test]
fn config_watcher_notifies_after_atomic_file_replacement_with_polling_fallback() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-watch-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("watch directory");
    let path = directory.join("config.toml");
    fs::write(&path, "[ui]\nfont_size_pt = 10.0\n").expect("initial config");

    let mut watch = ConfigWatch::default();
    watch.set_sources(&path, std::slice::from_ref(&path));
    // Force the portable polling path even on Linux, where inotify is
    // normally available. This also covers environments where native
    // notification setup fails or a directory cannot be watched.
    #[cfg(target_os = "linux")]
    watch.stop_inotify();
    let temporary = directory.join(".config.toml.tmp");
    fs::write(&temporary, "[ui]\nfont_size_pt = 11.0\n").expect("replacement");
    fs::rename(&temporary, &path).expect("atomic replacement");

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut notified = false;
    while !notified && Instant::now() < deadline {
        notified = watch.changed();
        if notified {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(notified, "inotify did not report the replacement");
    drop(watch);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn config_reload_worker_loads_and_validates_off_thread() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-worker-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("worker directory");
    let path = directory.join("config.toml");
    fs::write(&path, "schema_version = 3\n[ui]\nfont_size_pt = 11.0\n").expect("worker config");

    let mut worker = ConfigReloadWorker::spawn().expect("config worker");
    worker
        .request(path.clone(), "default".into())
        .expect("reload request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let result = result
        .expect("reload response")
        .expect("valid configuration");
    assert_eq!(result.operation, ConfigReadOperation::Reload);
    assert!((result.loaded.config.ui.font_size_pt - 11.0).abs() < f64::EPSILON);
    assert!(result.profile_overrides.settings.is_empty());

    let contexts_path = directory.join("contexts.toml");
    fs::write(&contexts_path, "unknown = true\n").expect("invalid contexts document");
    worker
        .request(path.clone(), "default".into())
        .expect("reload with invalid contexts request");
    let invalid_contexts = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let invalid_contexts = match invalid_contexts.expect("invalid contexts response") {
        Err(error) => error,
        Ok(_) => panic!("invalid contexts must reject reload"),
    };
    assert!(invalid_contexts.contains("contexts configuration is invalid"));
    let _ = fs::remove_file(&contexts_path);

    worker
        .request_check(path.clone())
        .expect("configuration check request");
    let check = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let check = check
        .expect("check response")
        .expect("valid checked configuration");
    assert_eq!(check.operation, ConfigReadOperation::Check);
    assert_eq!(check.loaded.sources.len(), 1);

    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn config_write_worker_creates_private_non_overwriting_file() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-writer-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("writer directory");
    let path = directory.join("export.toml");
    let contents = b"[ui]\nfont_size_pt = 11.0\n";

    let mut worker = ConfigWriteWorker::spawn().expect("config writer");
    worker
        .request(path.clone(), contents.to_vec())
        .expect("write request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let result = result
        .expect("write response")
        .expect("private configuration write");
    assert_eq!(result.path, path);
    assert_eq!(result.bytes, contents.len());
    assert_eq!(fs::read(&path).expect("written configuration"), contents);
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &fs::metadata(&path).expect("written metadata").permissions(),
        ) & 0o777,
        0o600
    );

    worker
        .request(path.clone(), b"replacement".to_vec())
        .expect("second write request");
    let second = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let collision = second.expect("collision response");
    assert!(collision.is_err(), "create-new must refuse overwrite");
    assert_eq!(fs::read(&path).expect("original configuration"), contents);

    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn userscript_manager_worker_reads_and_updates_off_thread() {
    let root = std::env::temp_dir().join(format!(
        "ferric-browser-userscript-worker-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let directory = root.join("userscripts");
    fs::create_dir_all(&directory).expect("userscript worker directory");
    fs::write(
        directory.join("worker.toml"),
        r#"
schema_version = 1
name = "worker"
executable = "/bin/true"
enabled = true
matches = ["https://example.test/*"]
"#,
    )
    .expect("userscript worker manifest");

    let mut worker = UserscriptManagerWorker::spawn().expect("userscript manager");
    worker
        .request(UserscriptManagerRequest::Refresh { root: root.clone() })
        .expect("inventory request");
    let inventory = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match inventory.expect("inventory response") {
        UserscriptManagerResult::Inventory(Ok(scripts)) => {
            assert_eq!(scripts.len(), 1);
            assert!(scripts[0].enabled);
        }
        other => panic!("unexpected inventory result: {other:?}"),
    }

    worker
        .request(UserscriptManagerRequest::SetEnabled {
            root: root.clone(),
            name: "worker".into(),
            enabled: false,
        })
        .expect("toggle request");
    let toggled = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match toggled.expect("toggle response") {
        UserscriptManagerResult::SetEnabled {
            enabled: false,
            result: Ok(scripts),
        } => assert!(!scripts[0].enabled),
        other => panic!("unexpected toggle result: {other:?}"),
    }

    let source = root.join("installed.toml");
    fs::write(
        &source,
        r#"
schema_version = 1
name = "installed"
executable = "/bin/true"
"#,
    )
    .expect("source userscript manifest");
    worker
        .request(UserscriptManagerRequest::Install {
            root: root.clone(),
            source,
        })
        .expect("install request");
    let installed = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match installed.expect("install response") {
        UserscriptManagerResult::Installed(Ok(scripts)) => {
            assert_eq!(scripts.len(), 2);
        }
        other => panic!("unexpected install result: {other:?}"),
    }

    worker
        .request(UserscriptManagerRequest::Remove {
            root: root.clone(),
            name: "installed".into(),
        })
        .expect("remove request");
    let removed = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match removed.expect("remove response") {
        UserscriptManagerResult::Removed(Ok(scripts)) => assert_eq!(scripts.len(), 1),
        other => panic!("unexpected remove result: {other:?}"),
    }
    assert!(!root.join("userscripts/installed.toml").exists());

    drop(worker);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn editor_write_worker_creates_private_scratch_file_off_thread() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-editor-writer-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let path = directory.join("editor").join("scratch.txt");
    let contents = b"editor text";
    let mut worker = EditorWriteWorker::spawn().expect("editor writer");
    worker
        .request(path.clone(), contents.to_vec())
        .expect("scratch request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert_eq!(
        result.expect("scratch response").expect("scratch write"),
        path
    );
    assert_eq!(fs::read(&path).expect("scratch contents"), contents);
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &fs::metadata(&path).expect("scratch metadata").permissions(),
        ) & 0o777,
        0o600
    );
    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(path.parent().expect("scratch parent"));
    let _ = fs::remove_dir(&directory);
}

#[test]
fn profile_delete_worker_removes_data_and_registry_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    let profile = registry
        .create("worker-delete", "Worker delete", ProfilePrivacy::Normal)
        .expect("profile record")
        .clone();
    let profile_data = roots.data.join("profiles").join(profile.id.to_string());
    fs::create_dir_all(&profile_data).expect("profile data");
    fs::write(profile_data.join("marker"), b"delete me").expect("profile marker");

    let mut worker = ProfileDeleteWorker::spawn().expect("profile delete worker");
    worker
        .request_create(
            roots.clone(),
            "worker-create".to_owned(),
            "Worker create".to_owned(),
        )
        .expect("create request");
    let create_result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert!(matches!(
        create_result
            .expect("create response")
            .expect("profile creation"),
        ProfileMutationResult::Created
    ));
    worker
        .request_rename(
            roots.clone(),
            "worker-create".to_owned(),
            "Worker renamed".to_owned(),
        )
        .expect("rename request");
    let rename_result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert!(matches!(
        rename_result
            .expect("rename response")
            .expect("profile rename"),
        ProfileMutationResult::Renamed
    ));
    assert_eq!(
        ProfileRegistry::open(&roots)
            .expect("reopen after rename")
            .profiles()
            .iter()
            .find(|record| record.name == "worker-create")
            .expect("created profile")
            .label,
        "Worker renamed"
    );
    worker
        .request_delete(roots.clone(), profile.name.clone(), None)
        .expect("delete request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert_eq!(
        match result.expect("delete response").expect("profile deletion") {
            ProfileMutationResult::Deleted(outcome) => outcome.profile_name,
            _ => panic!("unexpected profile mutation response"),
        },
        profile.name
    );
    assert!(!profile_data.exists());
    assert!(
        !ProfileRegistry::open(&roots)
            .expect("reopen profile registry")
            .profiles()
            .iter()
            .any(|record| record.id == profile.id)
    );
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn profile_list_worker_reads_registry_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    registry
        .create("worker-list", "Worker list", ProfilePrivacy::Normal)
        .expect("profile record");

    let mut worker = ProfileListWorker::spawn().expect("profile list worker");
    worker.request(roots.clone()).expect("list request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let values = result.expect("list response").expect("profile list");
    assert!(values.contains("worker-list\tWorker list\t"));
    assert_eq!(
        profile_from_list_values(&values, "worker-list"),
        Some(("worker-list".into(), "Worker list".into()))
    );
    assert!(profile_from_list_values(&values, "missing").is_none());
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn private_history_is_bounded_deduplicated_and_origin_clearable() {
    let mut records = Vec::new();
    let mut next_id = -1;
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://example.test/one",
        "One",
        10,
    );
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://other.test/two",
        "Two",
        20,
    );
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://example.test/one",
        "Updated",
        30,
    );
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].url, "https://example.test/one");
    assert_eq!(records[0].visit_count, 2);
    assert_eq!(records[0].title, "Updated");
    assert!(records.iter().all(|record| record.id < 0));

    let deleted =
        BrowserUi::clear_private_history_records(&mut records, None, Some("https://example.test"));
    assert_eq!(deleted, 1);
    assert_eq!(records[0].url, "https://other.test/two");
    assert_eq!(
        BrowserUi::clear_private_history_records(&mut records, None, None),
        1
    );
    assert!(records.is_empty());
}

#[test]
fn network_policy_worker_loads_cached_policy_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    let mut worker = NetworkPolicyWorker::spawn().expect("network policy worker");
    worker
        .request(
            Some(roots.clone()),
            serde_json::json!({
                "blocking": {
                    "enabled": true,
                    "network_filtering": true,
                    "lists": [],
                }
            }),
        )
        .expect("policy request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let policy = result.expect("policy response").expect("policy load");
    assert!(policy.blocked_hosts.is_empty());
    assert!(policy.compile_failures.is_empty());
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn profile_preview_worker_reads_delete_metadata_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    registry
        .create("worker-preview", "Worker preview", ProfilePrivacy::Normal)
        .expect("profile record");

    let mut worker = ProfilePreviewWorker::spawn().expect("profile preview worker");
    worker
        .request(roots.clone(), "worker-preview".into())
        .expect("preview request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let preview = result.expect("preview response").expect("profile preview");
    assert!(preview.contains("Profile: worker-preview"));
    assert!(preview.contains("QtWebEngine storage is not deleted"));
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}
