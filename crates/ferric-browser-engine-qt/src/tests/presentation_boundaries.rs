#[test]
fn ipc_url_output_removes_credentials_auth_secrets_and_fragments() {
    assert_eq!(
        safe_ipc_url("https://user:password@example.test/path?keep=1&token=secret#private"),
        "https://example.test/path?keep=1"
    );
    for key in [
        "refresh_token",
        "client_secret",
        "credential",
        "jwt",
        "%61ccess_token",
        "access%5Ftoken",
    ] {
        let url = format!("https://example.test/path?keep=1&{key}=secret");
        assert_eq!(safe_ipc_url(&url), "https://example.test/path?keep=1");
    }
    assert_eq!(
        safe_ipc_url("https://example.test/path?access%ZZtoken=secret&keep=1"),
        "https://example.test/path?keep=1"
    );
    assert_eq!(
        redact_process_stderr_token("refresh_token=secret"),
        "refresh_token=[redacted]"
    );
}

#[test]
fn engine_url_rejection_diagnostics_are_closed_categories() {
    let source = include_str!("../url_presentation.rs");
    assert!(source.contains("pub(super) enum EngineUrlError"));
    assert!(source.contains("Err(EngineUrlError::OriginMismatch)"));
    assert!(source.contains("Self::OriginMismatch => \"URL normalization mismatch\""));
    let conversion = source
        .split_once("pub(super) fn canonical_engine_url")
        .expect("engine URL conversion exists")
        .1
        .split_once("\nfn ensure_canonical_origins_match")
        .expect("canonical-origin check follows conversion")
        .0;
    assert!(conversion.contains("ensure_canonical_origins_match(&rust_raw_url, &rust_qt_url)?"));
    assert!(!conversion.contains("format!("));
    assert!(!conversion.contains("rust_raw_url}"));
    assert!(!conversion.contains("rust_qt_url}"));
    assert!(!source.contains("Rust origin={}"));
    assert!(!source.contains("Qt origin={}"));
}

#[test]
fn spawn_and_editor_failures_do_not_render_paths_or_os_diagnostics() {
    let runtime = include_str!("../lib.rs");
    assert!(
        runtime.contains("map_err(|_| \"could not create process working directory\".to_owned())?")
    );
    assert!(
        runtime.contains("map_err(|_| \"could not secure process working directory\".to_owned())?")
    );
    assert!(!runtime.contains("could not create process working directory: {error}"));
    assert!(!runtime.contains("could not secure process working directory: {error}"));

    let editor = include_str!("../editor_process.rs");
    assert!(editor.contains("map_err(|_| \"editor process status is unavailable\".to_owned())?"));
    assert!(!editor.contains("editor wait failed: {error}"));
}

#[test]
fn unsupported_ipc_method_errors_do_not_echo_client_input() {
    let source = include_str!("../browser_ui_ipc_dispatch.rs");
    let fallback = source
        .split_once("_ => ipc_failure(")
        .expect("unsupported IPC method fallback exists")
        .1;
    assert!(fallback.contains("\"unsupported IPC method\""));
    assert!(!fallback.contains("request.method"));
    assert!(!source.contains("unsupported IPC method: {}"));
}

#[test]
fn ipc_route_source_errors_do_not_echo_client_input() {
    let source = include_str!("../ipc_route.rs");
    assert!(source.contains("\"command context source is unsupported\""));
    assert!(!source.contains("command context source is unsupported: {value}"));
}

#[test]
fn ipc_command_failures_do_not_forward_dynamic_status_or_parser_text() {
    let source = include_str!("../browser_ui_ipc.rs");
    for safe_message in [
        "input workflow command was not accepted",
        "Site Doctor experiment could not be started",
        "Site Doctor result is invalid",
        "Site Doctor experiment could not be undone",
        "clipboard navigation was not accepted",
    ] {
        assert!(source.contains(safe_message));
    }
    assert!(!source.contains("status_text.to_string()"));
    assert!(!source.contains("serde_json::from_str(&payload).map_err(|error| error.to_string())"));
}

#[test]
fn blocking_status_does_not_project_the_global_status_bar_text() {
    let source = include_str!("../browser_ui_query_site.rs");
    assert!(source.contains("\"status\": if self.rust().blocking_enabled"));
    assert!(!source.contains("\"status\": self.rust().status_text.to_string()"));
}

#[test]
fn ipc_operation_errors_do_not_forward_global_status_or_parser_details() {
    let permissions = include_str!("../browser_ui_downloads_permissions.rs");
    let site = include_str!("../browser_ui_site.rs");
    let tabs = include_str!("../browser_ui_tabs.rs");
    let configuration = include_str!("../browser_ui_configuration.rs");

    for source in [permissions, site, tabs] {
        assert!(!source.contains("status_text.to_string()"));
    }
    for (source, safe_message) in [
        (permissions, "Permission listing is unavailable; details suppressed"),
        (permissions, "Permission reset failed; details suppressed"),
        (site, "Site-data clearing could not be started; details suppressed"),
        (tabs, "Command action could not be executed; details suppressed"),
        (tabs, "captured command arguments are invalid JSON"),
        (configuration, "Configuration reload rejected; details suppressed"),
    ] {
        assert!(source.contains(safe_message));
    }
    assert!(!tabs.contains("captured command arguments are invalid JSON: {error}"));
    assert!(!configuration.contains("return Err(status)"));
}

#[test]
fn runtime_setting_failures_use_fixed_status_and_property_copy() {
    let source = include_str!("../browser_ui_runtime_config.rs");
    let setter = source
        .split_once("pub(super) fn set_runtime_setting")
        .expect("runtime setting setter exists")
        .1
        .split_once("pub(super) fn unset_runtime_setting")
        .expect("runtime setting unsetter follows setter")
        .0;
    let unsetter = source
        .split_once("pub(super) fn unset_runtime_setting")
        .expect("runtime setting unsetter exists")
        .1
        .split_once("pub(super) fn clear_site_experiment_presentation")
        .expect("site experiment projection follows unsetter")
        .0;

    for body in [setter, unsetter] {
        assert!(body.contains("Runtime setting update failed; details suppressed"));
        assert!(body.contains("set_status_text(message.clone())"));
        assert!(body.contains("set_runtime_setting_error(message)"));
        assert!(!body.contains("QString::from(error"));
    }
}

#[test]
fn network_policy_status_suppresses_diagnostics_and_reports_only_success() {
    let source = include_str!("../browser_ui_runtime_config.rs");
    let network_policy = source
        .split_once("if let Some(result) = network_policy_result")
        .expect("network policy result handler exists")
        .1
        .split_once("let userscript_manager_result")
        .expect("network policy result handler ends before userscripts")
        .0;

    assert!(network_policy.contains("Network policy unavailable; details suppressed"));
    assert!(network_policy.contains("Err(_) => {"));
    assert!(network_policy.contains("self.as_mut().set_status_text(QString::from("));
    assert!(!network_policy.contains("Err(error)"));
    assert!(!network_policy.contains("{error}"));
    assert!(network_policy.contains("let reload_succeeded = match result"));
    assert!(network_policy.contains("let succeeded = policy.compile_failures.is_empty()"));
    assert!(network_policy.contains("self.as_mut().apply_network_policy_snapshot(policy)"));
    assert!(network_policy.contains("if reload_requested && reload_succeeded"));
    assert!(!network_policy.contains("if reload_requested {"));
}

#[test]
fn blocklist_update_status_does_not_echo_configured_cache_identity() {
    let updater = include_str!("../blocklist_updater.cpp");
    let runtime = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let completion = updater
        .split_once("void FerricBrowserBlocklistUpdater::finishOne(QNetworkReply *reply)")
        .expect("blocklist reply completion handler")
        .1
        .split_once("void FerricBrowserBlocklistUpdater::setStatus(")
        .expect("blocklist reply completion boundary")
        .0;

    assert!(completion.contains("setStatus(QStringLiteral(\"Blocklist unchanged\"))"));
    assert!(!completion.contains("Blocklist unchanged: %1"));
    assert!(!completion.contains(".arg(cacheId)"));
    assert!(completion.contains("failed_.append(cacheId)"));
    assert!(runtime.contains("browserUi.status_text = blocklistUpdater.status"));
}

#[test]
fn sensitive_qt_transient_records_do_not_regain_derived_debug() {
    let switcher = include_str!("../switcher_index.rs");
    assert!(!switcher.contains("Debug"));

    let state = include_str!("../browser_ui_state.rs");
    for declaration in [
        "#[derive(Clone, Debug)]\npub(super) struct PendingContextRoute",
        "#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]\npub(super) struct PermissionDecisionKey",
        "#[derive(Clone, Debug, Default)]\npub(super) struct ContextSnapshot",
        "#[derive(Clone, Debug)]\npub(super) struct SwitcherQueryCache",
        "#[derive(Debug, Default)]\npub(super) struct SessionCheckpoint",
    ] {
        assert!(
            !state.contains(declaration),
            "derived Debug returned: {declaration}"
        );
    }

    let editor = include_str!("../editor_process.rs");
    assert!(!editor.contains("#[derive(Clone, Debug)]"));
    let workers = include_str!("../background_workers.rs");
    assert!(!workers.contains("#[derive(Debug)]\npub(super) enum UserscriptManagerResult"));

    let browser = include_str!("../lib.rs");
    for declaration in [
        "#[derive(Clone, Debug)]\nstruct SiteExperiment",
        "#[derive(Clone, Debug)]\nstruct ClosedTabDescriptor",
        "#[derive(Clone, Debug)]\nstruct PendingExternalNavigation",
        "#[derive(Clone, Debug)]\nstruct PendingUserscript",
        "#[derive(Clone, Debug)]\nstruct UserscriptCompletion",
    ] {
        assert!(
            !browser.contains(declaration),
            "derived Debug returned: {declaration}"
        );
    }

    let evidence = include_str!("../blocking_evidence.rs");
    assert!(!evidence.contains("#[derive(Clone, Debug, Default)]"));
    let windows = include_str!("../window_registry.rs");
    assert!(!windows.contains("#[derive(Clone, Debug)]"));
}

#[test]
fn qml_runtime_warning_projection_does_not_render_warning_text() {
    let diagnostics = include_str!("../qml_diagnostics.cpp");
    let raw_warning_suppression = diagnostics
        .split("void ferric_browser_suppress_qml_warning_stderr")
        .nth(1)
        .expect("raw QML warning suppression function exists")
        .split("void ferric_browser_enable_qml_diagnostics")
        .next()
        .expect("raw warning suppression is separate from opt-in reporting");
    assert!(raw_warning_suppression.contains("setOutputWarningsToStandardError(false)"));

    let warning_hook = diagnostics
        .split("void ferric_browser_dump_qml_component_errors")
        .next()
        .expect("warning hook precedes opt-in component diagnostics");
    assert!(warning_hook.contains("warning details suppressed"));
    assert!(warning_hook.contains("warnings.size()"));
    assert!(!warning_hook.contains("warning.toString()"));

    let component_dump = diagnostics
        .split("void ferric_browser_dump_qml_component_errors")
        .nth(1)
        .expect("explicit startup diagnostics function exists");
    assert!(component_dump.contains("component diagnostic details suppressed"));
    assert!(component_dump.contains("errors.size()"));
    assert!(!component_dump.contains("error.toString()"));
    assert!(!component_dump.contains("rendered.constData()"));

    let executable = include_str!("../../../ferric-browser/src/main.rs");
    let failure_callback = executable
        .split("on_object_creation_failed")
        .nth(1)
        .expect("QML object-creation failure callback exists")
        .split("let initial_url")
        .next()
        .expect("failure callback ends before initial URL setup");
    let stderr_message = failure_callback
        .split("eprintln!(")
        .nth(1)
        .expect("failure callback emits a user-safe fixed diagnostic")
        .split(");")
        .next()
        .expect("stderr statement is terminated");
    assert_eq!(
        stderr_message,
        "\"Ferric QML component failed to load; source details suppressed\""
    );
    assert!(failure_callback.contains("if !qml_failed_signal.swap(true, Ordering::Relaxed)"));

    let warning_setup = executable
        .split("let qml_diagnostics_enabled")
        .nth(1)
        .expect("QML diagnostic policy is configured before loading the root")
        .split("let qml_failed")
        .next()
        .expect("warning policy is configured before failure handling");
    assert!(warning_setup.contains(
        "suppress_qml_warning_stderr(engine.as_mut());\n        if qml_diagnostics_enabled"
    ));
    assert!(warning_setup.contains("enable_qml_diagnostics(engine.as_mut());"));
}

#[test]
fn page_supplied_hint_diagnostics_use_fixed_categories() {
    let navigation = include_str!("../browser_ui_navigation.rs");
    for forbidden in [
        "format!(\"Hints rejected: {error}\")",
        "json!({\"error\": error})",
        "error.to_string()}).to_string()",
        "format!(\"Hint selection rejected: {error}\")",
        "format!(\"Hint link rejected: {error}\")",
        "format!(\"Clean hint link rejected: {error}\")",
        "format!(\"Hint navigation rejected: {error}\")",
        "format!(\"Hint external target rejected: {error}\")",
        "available\":labeled.len()",
        "only {} candidate(s)",
    ] {
        assert!(
            !navigation.contains(forbidden),
            "page-derived hint diagnostics must not escape: {forbidden}"
        );
    }
    assert!(!navigation.contains("{error}"));
    assert!(navigation.contains("page candidates were invalid"));
    assert!(navigation.contains("invalid-candidates"));
    assert!(navigation.contains("Hint index is unavailable for this page"));

    let payload = include_str!("../hint_payload.rs");
    assert!(payload.contains("map_err(|_| \"hint candidate JSON is invalid\".to_owned())?"));
    assert!(payload.contains("hint candidate kind is unsupported"));
    assert!(!payload.contains("unknown hint kind: {value}"));
    assert!(!payload.contains("format!(\"hint candidate JSON is invalid: {error}\")"));
    assert!(!payload.contains("format!(\"hint candidate count {} exceeds"));

    let selection = include_str!("../browser_ui_edit_commands.rs");
    assert!(selection.contains("return fail(self, \"Selection search is unavailable\".into());"));
    assert!(!selection.contains("Selection search unavailable: {error}"));
}

#[test]
fn profile_management_failures_do_not_surface_worker_diagnostics() {
    let management = include_str!("../profile_management.rs");
    let workers = include_str!("../profile_workers.rs");
    let runtime = include_str!("../browser_ui_runtime_config.rs");

    for safe_message in [
        "Profile list is unavailable",
        "Profile creation is unavailable",
        "Profile rename is unavailable",
        "Profile preview is unavailable",
        "Profile deletion is unavailable",
    ] {
        assert!(management.contains(safe_message));
    }
    assert!(runtime.contains("Profile list is unavailable"));
    assert!(runtime.contains("Profile preview is unavailable"));
    assert!(runtime.contains("Profile operation failed"));

    for dynamic_format in [
        "Profile list failed: {error}",
        "Profile creation failed: {error}",
        "Profile rename failed: {error}",
        "Profile preview failed: {error}",
        "Profile deletion failed: {error}",
        "Profile list unavailable: {error}",
        "Profile preview unavailable: {error}",
        "Profile operation failed: {error}",
        "profile registry unavailable: {error}",
    ] {
        assert!(!management.contains(dynamic_format));
        assert!(!runtime.contains(dynamic_format));
        assert!(!workers.contains(dynamic_format));
    }
    assert!(!workers.contains("delete_profile_transaction(&roots, &profile.name)\n                            .map_err(|error| error.to_string())"));
}

#[test]
fn asynchronous_profile_and_context_statuses_do_not_echo_user_labels() {
    let runtime = include_str!("../browser_ui_runtime_config.rs");
    let completion = runtime
        .split_once("let profile_list_result = {")
        .expect("profile-list completion handler exists")
        .1
        .split_once("let profile_delete_result = {")
        .expect("profile-list completion handler ends before deletion completion")
        .0;

    for fixed_message in [
        "Opening profile window",
        "Profile not found",
        "Context created",
        "Context creation failed; details suppressed",
        "Profiles available",
        "No durable profiles",
    ] {
        assert!(completion.contains(fixed_message));
    }
    assert!(completion.contains("set_profile_values(QString::from(values))"));
    for dynamic_message in [
        "Opening profile window: {profile_label}",
        "Profile not found: {name}",
        "Profile not found: {profile}",
        "Context created: {name}",
        "Context creation failed: {error}",
        "format!(\"Profiles: {status_values}\")",
    ] {
        assert!(!completion.contains(dynamic_message));
    }
}

#[test]
fn compositor_worker_statuses_do_not_echo_os_or_socket_diagnostics() {
    let runtime = include_str!("../browser_ui_runtime.rs");
    let config_runtime = include_str!("../browser_ui_runtime_config.rs");
    for fixed_message in [
        "Hyprland workspace routing failed; details suppressed",
        "Hyprland workspace move failed; details suppressed",
        "Hyprland browser-client query failed; details suppressed",
    ] {
        assert!(runtime.contains(fixed_message));
        assert!(config_runtime.contains(fixed_message));
    }
    for dynamic_message in [
        "Hyprland workspace routing failed: {error}",
        "Hyprland workspace move failed: {error}",
        "Hyprland browser-client query failed: {error}",
        "Hyprland routing unavailable: {status}",
        "Hyprland routing request unavailable: {error:?}",
        "Window movement unavailable: {status}",
        "Window movement request unavailable: {error:?}",
        "Hyprland client query unavailable: {status}",
        "Hyprland client query unavailable: {error:?}",
    ] {
        assert!(!runtime.contains(dynamic_message));
        assert!(!config_runtime.contains(dynamic_message));
    }
}

#[test]
fn profile_bootstrap_status_does_not_surface_paths_or_parser_diagnostics() {
    let source = include_str!("../browser_ui_profile_setup.rs");
    assert!(source.contains("Profile bootstrap failed; diagnostic details suppressed"));
    assert!(source.contains("durable storage unavailable; details suppressed"));
    assert!(source.contains("metadata worker unavailable; details suppressed"));
    assert!(source.contains("configuration issue; details suppressed"));
    assert!(source.contains("context configuration issue; details suppressed"));
    assert!(source.contains("Blocklist could not be installed"));
    assert!(source.contains("Blocklist installed; cache metadata is unavailable"));
    assert!(source.contains("Clean-link update rejected; last accepted rules retained"));
    assert!(!source.contains("Profile bootstrap failed: {error}"));
    assert!(!source.contains("durable storage unavailable: {error}"));
    assert!(!source.contains("metadata worker unavailable: {error}"));
    assert!(!source.contains("configuration issue: {error}"));
    assert!(!source.contains("context configuration issue: {error}"));
    assert!(!source.contains("Blocklist ID rejected: {id}"));
    assert!(!source.contains("Blocklist {id} could not be installed: {error}"));
    assert!(!source.contains("Clean-link update rejected; last accepted rules retained: {error}"));
    assert!(!source.contains("Clean-link rules accepted: {revision}"));
    assert!(!source.contains("map_err(|error| error.to_string())"));
}

#[test]
fn userscript_install_failures_do_not_surface_paths_or_parser_diagnostics() {
    let source = include_str!("../browser_ui_ui_actions.rs");
    let completion = include_str!("../browser_ui_runtime_config.rs");
    let qml = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    assert!(source.contains("error:userscript installation could not be queued"));
    assert!(source.contains("Userscript installation could not be queued; details suppressed"));
    assert!(completion.contains("Userscript inventory unavailable; details suppressed"));
    assert!(completion.contains("Userscript installation failed; details suppressed"));
    assert!(completion.contains("Userscript removal failed; details suppressed"));
    assert!(completion.contains("Userscript change rejected; details suppressed"));
    assert!(qml.contains("window.settingsNotice = \"Userscript operation failed: \""));
    assert!(!source.contains("QString::from(format!(\"error:{error}\"))"));
    assert!(!source.contains("Userscript installation failed: {error}"));
    for dynamic_message in [
        "Userscript inventory unavailable: {error}",
        "Userscript installation failed: {error}",
        "Userscript removal failed: {error}",
        "Userscript change rejected: {error}",
        "QString::from(format!(\"error:{error}\"))",
    ] {
        assert!(!completion.contains(dynamic_message));
    }
}

#[test]
fn action_catalog_statuses_do_not_surface_manifest_or_configuration_diagnostics() {
    let userscripts = include_str!("../userscript_presentation.rs");
    let external_actions = include_str!("../browser_ui_ui_actions.rs");
    let ipc_query = include_str!("../browser_ui_queries.rs");
    let action_list = include_str!("../browser_ui_repeat_macros.rs");

    assert!(userscripts.contains("Userscript actions unavailable; details suppressed"));
    assert!(!userscripts.contains("Userscript actions unavailable: {error}"));

    assert!(external_actions.contains("External actions unavailable; details suppressed"));
    assert!(!external_actions.contains("External actions unavailable: {error}"));

    assert!(ipc_query.contains(
        "let targets = configured_action_targets(&self.rust().config)\n            .map_err(|_| \"configured action catalog is unavailable; details suppressed\")?;"
    ));
    assert!(ipc_query.contains(
        ".map_err(|_| \"userscript action catalog is unavailable; details suppressed\")?;"
    ));

    assert!(action_list.contains("action catalog request is invalid; details suppressed"));
    assert!(action_list.contains("configured action catalog is unavailable; details suppressed"));
    assert!(action_list.contains("userscript action catalog is unavailable; details suppressed"));
    assert!(!action_list.contains("action_list_value(subject)?"));
    assert!(!action_list.contains("configured_action_targets(&self.as_ref().rust().config)?"));
}

#[test]
fn runtime_status_projection_uses_safe_user_message_not_diagnostic_context() {
    let source = include_str!("../runtime_bridge.rs");

    assert!(source.contains("error.user_message().to_owned()"));
    assert!(!source.contains("error.diagnostic_context()"));
}

#[test]
fn hint_userscript_failures_do_not_surface_manifest_or_worker_diagnostics() {
    let source = include_str!("../browser_ui_navigation.rs");

    assert!(source.contains("Hint userscript action rejected; details suppressed"));
    assert!(source.contains("Hint userscript rejected; details suppressed"));
    assert!(!source.contains("Hint userscript action rejected: {error}"));
    assert!(!source.contains("Hint userscript rejected: {error}"));
}

#[test]
fn userscript_execution_admission_suppresses_manifest_and_executable_diagnostics() {
    let source = include_str!("../browser_ui_web_actions.rs");
    let actions = include_str!("../browser_ui_ui_actions.rs");
    let ipc_actions = include_str!("../browser_ui_ipc_userscripts.rs");
    for fixed_message in [
        "userscript is unavailable or invalid; details suppressed",
        "userscript executable is unavailable or invalid; details suppressed",
        "userscript context encoding failed; details suppressed",
        "userscript process could not start; details suppressed",
        "userscript context delivery failed; details suppressed",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "userscript::load(&roots.config, name)?",
        "userscript::validate_executable(&loaded.executable)?",
        "could not encode userscript context: {error}",
        "could not start userscript: {error}",
    ] {
        assert!(!source.contains(dynamic_message));
    }
    for fixed_message in [
        "userscript action catalog is unavailable; details suppressed",
        "userscript action manifest is unavailable or invalid; details suppressed",
    ] {
        assert!(actions.contains(fixed_message));
    }
    for dynamic_message in [
        "userscript::registered_actions(&root)?",
        "userscript::registered_actions(&roots.config)?",
        "userscript::load(&roots.config, &action.script)?",
    ] {
        assert!(!actions.contains(dynamic_message));
    }
    assert!(
        ipc_actions.contains("userscript action catalog is unavailable; details suppressed")
    );
    let catalog_failure = ipc_actions
        .split_once("Some(Err(_))")
        .expect("userscript catalog failure arm exists")
        .1
        .split_once("Some(Ok(actions))")
        .expect("catalog success arm follows failure arm")
        .0;
    assert!(catalog_failure.contains(
        "userscript action catalog is unavailable; details suppressed"
    ));
    assert!(!catalog_failure.contains("error"));
}

#[test]
fn action_command_and_page_userscript_statuses_do_not_echo_dynamic_names() {
    let commands = include_str!("../browser_ui_command_workflows.rs");
    let target_routing = include_str!("../browser_ui_web_actions.rs");
    let browser = include_str!("../lib.rs");
    let external_actions = include_str!("../browser_ui_external_actions.rs");
    let switcher = include_str!("../browser_ui_runtime.rs");
    let services = include_str!("../../qml/components/FerricBrowserRuntimeServices.qml");
    let mapping = include_str!("../action_mapping.rs");

    for fixed_message in [
        "Action started",
        "Userscript action started",
        "Userscript action failed; details suppressed",
        "Selection sent to configured action target",
        "Switcher action executed",
    ] {
        assert!(commands.contains(fixed_message)
            || external_actions.contains(fixed_message)
            || switcher.contains(fixed_message));
    }
    for dynamic_message in [
        "format!(\"Executing {action_id}\")",
        "Executing userscript action {action_id}",
        "QString::from(userscript_error)",
    ] {
        assert!(!commands.contains(dynamic_message));
    }
    assert!(services.contains("Page userscript failed; details suppressed"));
    assert!(!services.contains("Page userscript failed: \" + scriptName"));
    assert!(!services.contains("})(ui, script.name, script.source)"));
    assert!(mapping.contains("action arguments are invalid; details suppressed"));
    assert!(!mapping.contains("format!(\"unknown action subject/verb: {subject} {verb}\")"));
    for fixed_message in [
        "configured action target catalog is unavailable; details suppressed",
        "configured action target is unavailable",
        "action target does not accept this subject",
        "Sent to configured action target",
    ] {
        assert!(target_routing.contains(fixed_message));
    }
    for dynamic_message in [
        "configured_action_targets(&self.as_ref().rust().config)?",
        "format!(\"unknown action target: {target_name}\")",
        "format!(\"action target does not accept {required_subject}\")",
        "Sent to {target_name}: {operation_id}",
    ] {
        assert!(!target_routing.contains(dynamic_message));
    }
    let pending_action = browser
        .split_once("struct PendingActionTarget {")
        .expect("pending action target declaration exists")
        .1
        .split_once("\n}")
        .expect("pending action target declaration ends")
        .0;
    assert!(!pending_action.contains("target_name"));
    assert!(!target_routing.contains("target_name: target_name.clone()"));
    assert!(target_routing.contains("\"target\": target_name"));
    assert!(target_routing.contains("\"operation_id\": operation_id"));
    assert!(!switcher.contains("Switcher action executed: {}"));
    assert!(!switcher.contains("definition.label"));
    assert!(!external_actions.contains("Sent selection to {}: {operation_id}"));
}

#[test]
fn spawn_status_does_not_expose_operation_ids() {
    let command = include_str!("../browser_ui_edit_commands.rs");
    let callback = include_str!("../browser_ui_web_actions.rs");

    for source in [command, callback] {
        assert!(source.contains("QString::from(\"Spawn started\")"));
        assert!(!source.contains("Spawn started: {operation_id}"));
    }
    assert!(command.contains("\"operation_id\": operation_id"));
}

#[test]
fn userscript_completion_suppresses_process_and_protocol_diagnostics() {
    let source = include_str!("../browser_ui_web_actions.rs");
    let protocol = include_str!("../userscript_protocol.rs");
    for fixed_message in [
        "failed (userscript result was invalid)",
        "failed (userscript output was unavailable)",
        "failed (userscript exited unsuccessfully)",
        "failed (process status unavailable)",
        "failed (userscript context delivery failed)",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "format!(\"failed ({error})\")",
        "format!(\"failed ({status})\")",
        "format!(\"failed ({})\", error)",
    ] {
        assert!(!source.contains(dynamic_message));
    }
    assert!(source.contains("sanitize_process_stderr(&output)"));
    assert!(source.contains("format!(\"completed: {}\", output.message)"));
    for fixed_message in [
        "invalid userscript JSON result",
        "userscript result type is not allowed",
        "userscript command is not allowed",
        "userscript result type is unsupported",
    ] {
        assert!(protocol.contains(fixed_message));
    }
    for dynamic_message in [
        "invalid userscript JSON result: {error}",
        "userscript result type is not allowed: {result_type}",
        "userscript command is not allowed: {name}",
        "userscript result type is not implemented: {result_type}",
    ] {
        assert!(!protocol.contains(dynamic_message));
    }
}

#[test]
fn search_configuration_diagnostics_do_not_echo_caller_or_parser_values() {
    let source = include_str!("../navigation_policy.rs");
    assert!(source.contains("search configuration is invalid; details suppressed"));
    assert!(source.contains("configured search engine was not found"));
    assert!(source.contains("search URL could not be resolved; details suppressed"));
    assert!(!source.contains("search configuration is invalid: {error}"));
    assert!(!source.contains("configured search engine not found: {engine}"));
}

#[test]
fn blocklist_compilation_errors_do_not_surface_downloaded_or_parser_details() {
    let source = include_str!("../blocking_presentation.rs");
    let policy = include_str!("../network_policy.rs");
    assert!(
        source.contains("Blocklist compilation failed; current rules retained; details suppressed")
    );
    assert!(!source.contains("policy.compile_failures.join"));
    assert!(policy.contains("adblock-engine compilation failed; details suppressed"));
    assert!(!policy.contains("compile_failures.push(format!(\"adblock-engine: {error}\"))"));
}

#[test]
fn site_blocker_toggle_status_does_not_echo_the_current_host() {
    let source = include_str!("../blocking_presentation.rs");
    assert!(source.contains("Site blocker bypass disabled\""));
    assert!(source.contains("Site blocker bypass enabled\""));
    assert!(!source.contains("Site blocker bypass disabled for {host}"));
    assert!(!source.contains("Site blocker bypass enabled for {host}"));
    assert!(source.contains("sites.push(host.clone())"));
}

#[test]
fn interactive_site_data_preview_does_not_put_the_origin_in_status() {
    let dispatch = include_str!("../browser_ui_command_dispatch.rs");
    let site = include_str!("../browser_ui_site.rs");
    let typed_command = site
        .split_once("pub(super) fn execute_site_data_clear_command")
        .expect("typed site-data command exists")
        .1;
    assert!(dispatch.contains(
        "Site-data scope preview ready; repeat with --confirm to clear supported categories"
    ));
    assert!(!dispatch.contains("Site-data scope preview: {plan}"));
    assert!(!dispatch.contains("site_data_clear_plan(&QString::from(origin))"));
    assert!(typed_command.contains("return Ok(value);"));
}

#[test]
fn external_uri_status_does_not_echo_the_caller_scheme() {
    let sessions = include_str!("../browser_ui_sessions.rs");
    assert!(sessions.contains("External URI confirmed; opening with the system handler"));
    assert!(!sessions.contains("{} URI confirmed; opening with the system handler"));
    assert!(sessions.contains("Some(format!(\"external-open\\t{}\", pending.url))"));
}

#[test]
fn userscript_print_and_journey_export_errors_suppress_local_diagnostics() {
    let content_tools = include_str!("../browser_ui_content_tools.rs");
    let popups = include_str!("../browser_ui_popups.rs");
    for fixed_message in [
        "Page userscripts unavailable; details suppressed",
        "Userscript inventory request unavailable; details suppressed",
        "Userscript change rejected; details suppressed",
        "Userscript removal rejected; details suppressed",
        "Print PDF path rejected; details suppressed",
        "Save Page path rejected; details suppressed",
        "Print staging directory is unavailable; details suppressed",
        "Print staging directory permissions could not be secured; details suppressed",
        "PDF output unavailable or invalid; details suppressed",
        "Print failed; details suppressed",
    ] {
        assert!(content_tools.contains(fixed_message));
    }
    for fixed_message in [
        "Journey export serialization failed; details suppressed",
        "Journey export is already loading or unavailable; details suppressed",
        "Journey export unavailable; details suppressed",
        "Journey export failed; details suppressed",
    ] {
        assert!(popups.contains(fixed_message));
    }
    for dynamic_message in [
        "Page userscripts unavailable: {error}",
        "Userscript inventory unavailable: {error}",
        "Userscript inventory request unavailable: {error}",
        "Userscript change rejected: {error}",
        "Userscript removal rejected: {error}",
        "Print PDF path rejected: {error}",
        "Save Page path rejected: {error}",
        "generated PDF is unavailable: {error}",
        "Print staging directory is unavailable: {error}",
        "Print staging directory permissions could not be secured: {error}",
        "Print failed: {error}",
        "Journey export is already loading or unavailable: {error}",
        "Journey export unavailable: {error}",
        "Journey export failed: {error}",
        "map_err(|error| error.to_string())",
        "QString::from(error)",
    ] {
        assert!(!content_tools.contains(dynamic_message));
        assert!(!popups.contains(dynamic_message));
    }
}

#[test]
fn configuration_query_failures_do_not_echo_keys_or_parser_diagnostics() {
    let source = include_str!("../browser_ui_query_configuration.rs");
    for category in ["IpcQueryError::Invalid", "IpcQueryError::NotFound"] {
        assert!(source.contains(category));
    }
    for dynamic_message in [
        "configuration key not found: {key}",
        "configuration key {key} does not support site scope",
        "configuration key is not in the setting registry: {key}",
        "validated configuration could not be decoded: {error}",
    ] {
        assert!(!source.contains(dynamic_message));
    }
}

#[test]
fn permission_query_origin_validation_uses_fixed_error_copy() {
    let source = include_str!("../browser_ui_query_permissions.rs");
    assert!(source.contains(".map_err(|_| IpcQueryError::Invalid)?"));
    assert!(!source.contains("map_err(IpcQueryError::Invalid)"));
}

#[test]
fn ipc_event_subscription_failures_suppress_worker_diagnostics() {
    let source = include_str!("../browser_ui_maintenance.rs");
    assert!(source.contains("IPC event subscription is unavailable; details suppressed"));
    assert!(source.contains("subscribe_event_stream(stream, event_types).is_err()"));
    assert!(!source.contains("ipc_failure(&pending.request().id, \"E_IO\", error)"));
}

#[test]
fn shutdown_storage_failures_do_not_surface_worker_diagnostics() {
    let source = include_str!("../browser_ui_runtime.rs");
    assert!(source.contains("Durable metadata flush failed; details suppressed"));
    assert!(source.contains("Durable metadata flush unavailable; details suppressed"));
    assert!(source.contains(
        "Stale session checkpoint cleanup unavailable; recovery data was retained; details suppressed"
    ));
    assert!(!source.contains("Durable metadata flush failed: {error}"));
    assert!(!source.contains("Durable metadata flush unavailable: {error}"));
    assert!(!source.contains("recovery data was retained: {error}"));
}

#[test]
fn browser_status_error_projections_suppress_diagnostic_values() {
    let input = include_str!("../browser_ui_input.rs");
    let permissions = include_str!("../browser_ui_downloads_permissions.rs");
    let permission_reset = include_str!("../browser_ui_permission_reset.rs");
    let sessions = include_str!("../browser_ui_sessions.rs");
    let navigation = include_str!("../browser_ui_context_navigation.rs");
    let navigation_lifecycle = include_str!("../browser_ui_navigation_lifecycle.rs");
    let tab_state = include_str!("../browser_ui_tab_state.rs");
    let tabs = include_str!("../browser_ui_tabs.rs");

    for (source, fixed_message) in [
        (input, "Binding rejected; details suppressed"),
        (permissions, "Permission session decision rejected; details suppressed"),
        (permissions, "Permission rule rejected; details suppressed"),
        (permissions, "Permissions unavailable; details suppressed"),
        (permission_reset, "Permission reset rejected; details suppressed"),
        (permission_reset, "Permission reset unavailable; details suppressed"),
        (sessions, "Recovery unavailable; details suppressed"),
        (sessions, "Session restore unavailable; details suppressed"),
        (sessions, "Session preview unavailable; details suppressed"),
        (sessions, "Session deletion unavailable; details suppressed"),
        (sessions, "Clean-link navigation rejected; details suppressed"),
        (navigation, "Navigation rejected; details suppressed"),
        (navigation, "Clipboard navigation rejected; details suppressed"),
        (navigation_lifecycle, "Navigation URL rejected; details suppressed"),
        (navigation_lifecycle, "Engine URL rejected; details suppressed"),
        (
            navigation_lifecycle,
            "Tab close acknowledgement rejected; details suppressed",
        ),
        (tab_state, "Tab transfer URL rejected; details suppressed"),
        (tabs, "Tab pinning rejected; details suppressed"),
        (tabs, "Tab mute rejected; details suppressed"),
        (tabs, "Tab close rejected; details suppressed"),
    ] {
        assert!(source.contains(fixed_message), "missing {fixed_message}");
    }

    for dynamic_message in [
        "Binding rejected: {error}",
        "Permission session decision rejected: {error}",
        "Permission rule rejected: {error}",
        "Permissions unavailable: {error}",
        "Permission reset rejected: {error}",
        "Permission reset unavailable; retry the command: {error}",
        "Recovery unavailable: {error}",
        "Session restore unavailable: {error}",
        "Session preview unavailable: {error}",
        "Session deletion unavailable: {error}",
        "Clean-link navigation rejected: {error}",
        "Navigation rejected: {error}",
        "Navigation URL rejected: {error}",
        "Engine URL rejected: {error}",
        "Tab transfer URL rejected: {error}",
        "Tab pinning rejected: {error}",
        "Tab mute rejected: {error}",
        "Tab close rejected: {error}",
        "Tab close acknowledgement rejected: {error}",
    ] {
        assert!(
            ![
                input,
                permissions,
                permission_reset,
                sessions,
                navigation,
                navigation_lifecycle,
                tab_state,
                tabs,
            ]
            .iter()
            .any(|source| source.contains(dynamic_message)),
            "dynamic status projection remains: {dynamic_message}"
        );
    }
}

#[test]
fn reduced_chrome_command_status_does_not_echo_command_text() {
    let source = include_str!("../browser_ui_command_history.rs");
    let dispatch = source
        .split_once("fn execute_interactive_command_for_surface")
        .expect("surface-specific interactive command dispatcher exists")
        .1
        .split_once("let eligible =")
        .expect("reduced-chrome rejection precedes command eligibility")
        .0;

    assert!(dispatch.contains(
        "This command is unavailable in this window; use a full browser window"
    ));
    assert!(!dispatch.contains("command.name"));
    assert!(!dispatch.contains("format!("));
}

#[test]
fn durable_library_statuses_do_not_surface_storage_or_page_diagnostics() {
    let source = include_str!("../browser_ui_library.rs");
    for fixed_message in [
        "Session catalog query failed; details suppressed",
        "Session snapshot failed; changes retained; details suppressed",
        "Recovery rejected; details suppressed",
        "Profile metadata query failed; details suppressed",
        "Command history write failed; details suppressed",
        "Journey query failed; details suppressed",
        "Journey export unavailable; details suppressed",
        "Download destination update failed; details suppressed",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "Session catalog query failed: {error}",
        "Session snapshot failed; changes retained: {error}",
        "Recovery rejected: {error}",
        "Profile metadata query failed: {error}",
        "Command history write failed: {error}",
        "Journey query failed: {error}",
        "Journey export unavailable: {error}",
        "Download destination update failed: {error}",
        "set_status_text(QString::from(error))",
    ] {
        assert!(!source.contains(dynamic_message));
    }
    assert!(source.contains("Some(\"command history write failed; details suppressed\".into())"));
    assert!(!source.contains("command_history_write_error = Some(error.clone())"));
}

#[test]
fn storage_batch_statuses_do_not_surface_worker_diagnostics() {
    let source = include_str!("../browser_ui_storage_writes.rs");
    let scheduling = include_str!("../browser_ui_persistence.rs");
    for fixed_message in [
        "History worker write failed; details suppressed",
        "Permission worker write failed; details suppressed",
        "Download worker write failed; details suppressed",
        "Download update could not be scheduled; details suppressed",
        "Permission write could not be scheduled; details suppressed",
        "History write could not be scheduled; details suppressed",
        "Journey write could not be scheduled; history retained; details suppressed",
    ] {
        assert!(source.contains(fixed_message) || scheduling.contains(fixed_message));
    }
    for dynamic_message in [
        "History worker write failed: {error}",
        "Permission worker write failed: {error}",
        "Download worker write failed: {error}",
        "Download update could not be scheduled: {error}",
        "Permission write could not be scheduled: {error}",
        "History write could not be scheduled: {error}",
        "Journey write could not be scheduled; history retained: {error}",
    ] {
        assert!(!source.contains(dynamic_message));
        assert!(!scheduling.contains(dynamic_message));
    }
}

#[test]
fn configuration_worker_statuses_do_not_surface_paths_or_parser_diagnostics() {
    let source = include_str!("../browser_ui_runtime_config.rs");
    let configuration = include_str!("../browser_ui_configuration.rs");
    assert!(source.contains("Configuration write failed; details suppressed"));
    assert!(source.contains("Configuration reload rejected; details suppressed"));
    assert!(source.contains("Configuration reload unavailable; details suppressed"));
    assert!(source.contains("Print failed; details suppressed"));
    assert!(configuration.contains("Site rules unavailable; details suppressed"));
    assert!(configuration.contains("Configuration editor status unavailable; details suppressed"));
    for dynamic_message in [
        "Configuration write failed: {error}",
        "Configuration reload rejected: {error}",
        "Configuration reload unavailable: {error}",
        "Print failed: {error}",
        "Site rules unavailable: {error}",
        "Configuration editor status unavailable: {error}",
    ] {
        assert!(!source.contains(dynamic_message));
        assert!(!configuration.contains(dynamic_message));
    }
}

#[test]
fn external_process_failures_do_not_surface_os_diagnostics() {
    let source = include_str!("../browser_ui_process_actions.rs");
    assert!(source.contains("could not start executable"));
    assert!(source.contains("failed (process status unavailable)"));
    assert!(source.contains("could not start action target"));
    for dynamic_message in [
        "could not start executable: {error}",
        "failed ({error})",
        "could not start action target: {error}",
    ] {
        assert!(!source.contains(dynamic_message));
    }
}

#[test]
fn context_configuration_failures_do_not_echo_parser_paths_or_user_ids() {
    let source = include_str!("../browser_ui_context_commands.rs");
    for fixed_message in [
        "Profile window navigation rejected; details suppressed",
        "contexts configuration is unavailable; details suppressed",
        "contexts configuration could not be published; details suppressed",
        "contexts configuration is invalid; details suppressed",
        "could not serialize contexts configuration; details suppressed",
        "context route write failed; details suppressed",
        "context route not found",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "Profile window navigation rejected: {error}",
        "contexts configuration is invalid: {error}",
        "could not serialize contexts configuration: {error}",
        "save_contexts_atomic(&path, &config).map_err(|error| error.to_string())",
        "context route not found: {route_id}",
    ] {
        assert!(!source.contains(dynamic_message));
    }
}

#[test]
fn switcher_action_failures_do_not_surface_dynamic_details() {
    let source = include_str!("../browser_ui_runtime.rs");
    assert!(source.contains("Switcher action unavailable; details suppressed"));
    assert!(source.contains("Switcher action failed; details suppressed"));
    assert!(!source.contains("Switcher action unavailable: {error}"));
    assert!(!source.contains("Switcher action failed: {error}"));
}

#[test]
fn download_open_failures_do_not_surface_paths_or_os_diagnostics() {
    let source = include_str!("../browser_ui_downloads_permissions.rs");
    for fixed_message in [
        "download file is unavailable; details suppressed",
        "download directory is unavailable; details suppressed",
        "download location could not be resolved; details suppressed",
        "Download index unavailable; details suppressed",
        "Download directory unavailable; details suppressed",
        "Download destination rejected; details suppressed",
        "Download staging unavailable; details suppressed",
        "Download index update failed; details suppressed",
        "Download update could not be scheduled; details suppressed",
        "Download finalization failed; details suppressed",
        "Download action could not be queued; details suppressed",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "download file is unavailable: {error}",
        "download directory is unavailable: {error}",
        "Download index unavailable: {error}",
        "Download directory unavailable: {error}",
        "Download destination rejected: {error}",
        "Download staging unavailable: {error}",
        "Download index update failed: {error}",
        "Download finalization failed: {error}",
        "Download action could not be queued: {error}",
        "if let Err(error) = fs::create_dir_all(&directory)",
    ] {
        assert!(!source.contains(dynamic_message));
    }
    assert!(!source.contains(
        "path_to_file_url(&target)\n                .map_err(|error| error.to_string())"
    ));
}

#[test]
fn site_doctor_and_diagnostics_failures_do_not_surface_dynamic_details() {
    let source = include_str!("../browser_ui_site.rs");
    for fixed_message in [
        "Site Doctor proposal could not read effective config; details suppressed",
        "Site Doctor durable fix was not applied; details suppressed",
        "Site status unavailable; details suppressed",
        "Diagnostics unavailable; details suppressed",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "Site Doctor proposal could not read effective config: {error}",
        "Site Doctor durable fix was not applied: {error}",
        "json!({\"error\": error})",
        "QString::from(error)",
    ] {
        assert!(!source.contains(dynamic_message));
    }
}

#[test]
fn renderer_callback_errors_do_not_echo_page_supplied_text() {
    let source = include_str!("../browser_ui_external_actions.rs");
    let maintenance = include_str!("../browser_ui_maintenance.rs");
    for fixed_message in [
        "Action target selection unavailable",
        "Action target failed",
        "Selection could not be copied",
        "Userscript selection unavailable",
        "Userscript selection failed",
        "Caret operation was rejected",
        "External editor could not start",
        "External editor could not complete the request",
        "External changes could not be applied",
        "editor file could not be written",
        "configured editor command is unavailable",
        "could not read edited text",
        "editor exited unsuccessfully",
        "editor process could not be monitored",
    ] {
        assert!(source.contains(fixed_message));
    }
    for dynamic_message in [
        "Action target selection unavailable: {error}",
        "action target failed: {error}",
        "Selection not copied: {error}",
        "Userscript selection unavailable: {error}",
        "Userscript selection failed: {error}",
        "Caret operation rejected: {error}",
        "could not queue editor file: {error}",
        "could not write editor file: {error}",
        "could not read edited text: {error}",
        "Editor could not start: {error}",
        "Editor not applied: {error}",
        "editor exited with status {status}",
    ] {
        assert!(!source.contains(dynamic_message));
    }
    assert!(maintenance.contains("failed (userscript action failed)"));
    assert!(!maintenance.contains("userscript action: {error}"));
}

#[test]
fn selection_spawn_failures_do_not_surface_command_or_os_details() {
    let source = include_str!("../browser_ui_web_actions.rs");
    assert!(source.contains("Spawn selection could not be resolved"));
    assert!(source.contains("Spawn could not start"));
    assert!(!source.contains("QString::from(error)"));
    assert!(!source.contains("Spawn failed: {error}"));
}

#[test]
fn subprocess_output_errors_are_fixed_or_discarded() {
    let userscript_io = include_str!("../userscript_io.rs");
    let editor_launch = include_str!("../browser_ui_edit_commands.rs");
    assert!(userscript_io.contains("external process output read failed"));
    assert!(editor_launch.contains(".stderr(Stdio::null())"));
    assert!(!userscript_io.contains("userscript pipe read failed: {error}"));
    assert!(!editor_launch.contains(".stderr(Stdio::piped())"));
}

#[test]
fn display_url_redacts_credentials_secrets_and_directional_controls() {
    assert_eq!(
        display_url("https://user:password@example.test/path?keep=1&token=secret#private"),
        "https://example.test\u{2068}/path?keep=1\u{2069}"
    );
    assert_eq!(
        display_url("https://example.test/a\u{202e}b\u{0007}"),
        "https://example.test\u{2068}/a[bidi]b�\u{2069}"
    );
    assert_eq!(
        display_url("https://раypal.example/path"),
        "https://xn--ypal-43d9g.example\u{2068}/path\u{2069}"
    );
    let qml = QML_SOURCE;
    let runtime_chrome = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let status_bar = include_str!("../../qml/components/FerricStatusBar.qml");
    let secondary_window = include_str!("../../qml/components/FerricBrowserWindow.qml");
    assert!(qml.contains("function addressPresentation(value, maxCharacters)"));
    assert!(qml.contains("var origin = safe.slice(0, authorityEnd)"));
    assert!(qml.contains("var separator = tail.charAt(0) === \"/\" ? \"/…\" : \"…\""));
    assert!(runtime_chrome.contains("displayUrl: browserUi.display_url"));
    assert!(status_bar.contains("Accessible.name: statusVisible"));
    assert!(secondary_window.contains("Accessible.description: secondaryUi.display_url"));
    assert!(secondary_window.contains("addressEditing = activeFocus"));
    assert!(display_url("https://example.test/path").contains('\u{2068}'));
    assert!(display_url("https://example.test/path").contains('\u{2069}'));
}

#[test]
fn site_origin_is_normalized_and_never_keeps_path_or_credentials() {
    assert_eq!(
        safe_site_origin("HTTPS://user:password@Example.test:443/private?token=secret"),
        Some("https://example.test".into())
    );
    assert_eq!(safe_site_origin("file:///tmp/private"), None);
    assert_eq!(
        safe_site_origin("https://[::1]:443/path"),
        Some("https://[::1]".into())
    );
}

#[test]
fn qt_url_conversion_is_strict_before_rust_policy_canonicalization() {
    assert_eq!(
        canonical_engine_url(QString::from(
            "HTTPS://Example.TEST.:443/a%2Fb?q=%2F#frag%23",
        ))
        .expect("canonical Qt URL"),
        "https://example.test/a%2Fb?q=%2F#frag%23"
    );
    assert_eq!(
        canonical_engine_url(QString::from(
            "https://login.example.test/return?next=https%3A%2F%2Fapp.example.test%2F#state",
        ))
        .expect("redirect URL"),
        "https://login.example.test/return?next=https%3A%2F%2Fapp.example.test%2F#state"
    );
    assert!(canonical_engine_url(QString::from("https://example.test/a b")).is_err());
    assert!(canonical_engine_url(QString::from("javascript:alert(1)")).is_err());
    assert!(canonical_engine_url(QString::from("rb://settings")).is_err());
}

#[test]
fn page_titles_are_sanitized_and_bounded_before_storage() {
    let title = sanitize_untrusted_title(&format!(
        "line\n{}\u{202e}tail",
        "x".repeat(MAX_PAGE_TITLE_BYTES)
    ));
    assert!(title.len() <= MAX_PAGE_TITLE_BYTES);
    assert!(!title.chars().any(char::is_control));
    assert!(!title.contains('\u{202e}'));
}

#[test]
fn page_authority_boundary_has_no_web_channel_or_privileged_page_object() {
    let qml = QML_SOURCE;
    assert!(!qml.contains("WebChannel"));
    assert!(!qml.contains("contextProperty"));
    assert!(qml.contains("WebEngineScript.ApplicationWorld"));
    assert!(qml.contains("runJavaScript"));
    assert!(qml.contains("page_focus_observed_for"));
}

#[test]
fn external_navigation_has_a_native_confirmation_boundary() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricExternalNavigationDialog.qml"),
    ]
    .concat();
    assert!(qml.contains("external_navigation_visible"));
    assert!(qml.contains("confirm_external_navigation"));
    assert!(qml.contains("cancel_external_navigation"));
    assert!(qml.contains("external-open\\t"));
    assert!(qml.contains("function externalUriAllowed(uri)"));
    assert!(qml.contains("External URI rejected by scheme policy"));
    assert!(qml.contains("return /^file:\\/\\/\\/[^/]/i.test(value)"));
    assert!(qml.contains("Qt.openUrlExternally"));
}

#[test]
fn navigation_failure_surface_keeps_safe_context_without_retry_action() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricNavigationFailure.qml"),
    ]
    .concat();
    let lifecycle = include_str!("../navigation_lifecycle.rs");
    assert!(qml.contains("navigation_failure_requested_url"));
    assert!(qml.contains("navigation_failure_url"));
    assert!(qml.contains("navigation_failure_kind"));
    assert!(qml.contains("navigation_failed_with_details"));
    assert!(qml.contains("Retry is intentionally not offered"));
    assert!(lifecycle.contains("Additional navigation details are suppressed."));
    assert!(!lifecycle.contains("bounded_navigation_failure_detail"));
    assert!(!lifecycle.contains("format!(\"{} failure: {detail}\""));
    assert!(!lifecycle.contains("detail.to_string()"));
}

#[test]
fn switcher_surface_exposes_selection_state_and_scaled_keyboard_navigation() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricSwitcher.qml"),
        include_str!("../../qml/components/FerricSwitcherResults.qml"),
    ]
    .concat();
    assert!(qml.contains("Accessible.role: Accessible.List"));
    assert!(qml.contains("Accessible.role: Accessible.ListItem"));
    assert!(qml.contains("Accessible.selected: index === resultList.currentIndex"));
    assert!(qml.contains("Qt.Key_PageDown"));
    assert!(qml.contains("Qt.Key_PageUp"));
    assert!(qml.contains("browserWindow.chromeRowHeight"));
}

#[test]
fn accessibility_surface_reports_theme_contrast_and_reduced_motion() {
    let source = ADAPTER_SOURCE;
    let preferences = include_str!("../chrome_preferences.rs");
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricDiagnostics.qml"),
    ]
    .concat();
    assert!(qml.contains("theme_background_color"));
    assert!(qml.contains("theme_selection_text_color"));
    assert!(qml.contains("theme_contrast_status"));
    assert!(qml.contains("theme_contrast_reason"));
    assert!(!qml.contains("theme_palette_json"));
    assert!(!qml.contains("theme_contrast_json"));
    assert!(qml.contains("themeContrastWarning"));
    assert!(qml.contains("renderedContrastReport"));
    assert!(qml.contains("renderedThemeContrastStatus"));
    assert!(qml.contains("system_reduced_motion_status"));
    assert!(qml.contains("system_reduced_motion_enabled"));
    assert!(qml.contains("system_font_scale_status"));
    assert!(qml.contains("system_font_scale"));
    assert!(qml.contains("ui.system_reduced_motion_status === \"available\""));
    assert!(source.contains("chrome_reduced_motion"));
    assert!(preferences.contains("struct ChromePreferences"));
    assert!(qml.contains("ui.chrome_reduced_motion === \"on\""));
    assert!(qml.contains("window.chromeFontFamily = ui.chrome_font_family"));
    assert!(qml.contains("Optional interface motion is disabled"));
}

#[test]
fn scalar_feature_preferences_cross_the_qml_boundary_without_config_parsing() {
    let source = ADAPTER_SOURCE;
    let preferences = include_str!("../feature_preferences.rs");
    let qml = QML_SOURCE;
    assert!(source.contains("feature_switcher_max_results"));
    assert!(source.contains("update_feature_preferences"));
    assert!(preferences.contains("struct FeaturePreferences"));
    assert!(qml.contains("return browserUi.feature_switcher_max_results"));
    assert!(qml.contains("return browserUi.feature_downloads_ask_destination"));
    assert!(qml.contains("feature_desktop_notifications_enabled"));
    assert!(qml.contains("feature_desktop_media_keys_enabled"));
    assert!(qml.contains("feature_push_service_enabled"));
    assert!(qml.contains("feature_blocking_update_interval_hours"));
    assert!(qml.contains("feature_link_cleaning_update_source"));
    assert!(qml.contains("feature_blocking_list_ids"));
}

#[test]
fn normal_input_uses_logical_unmodified_text_and_preserves_unicode_fields() {
    let qml = QML_SOURCE;
    let keyboard_presentation = include_str!("../../qml/scripts/KeyboardPresentation.js");
    assert!(qml.contains("function logicalNormalKeyText(event)"));
    assert!(qml.contains("event.key === Qt.Key_D"));
    assert!(qml.contains("event.key === Qt.Key_U"));
    assert!(qml.contains("return \"Ctrl+d\""));
    assert!(qml.contains("return \"Ctrl+u\""));
    assert!(qml.contains("return \"Ctrl+p\""));
    assert!(qml.contains("return \"Ctrl+Space\""));
    assert!(qml.contains("return \"Ctrl+v\""));
    assert!(qml.contains("return \"Ctrl+Shift+t\""));
    assert!(qml.contains("return \"Alt+m\""));
    assert!(qml.contains("return \"Ctrl+Alt+p\""));
    assert!(qml.contains("return \"Ctrl+PgDown\""));
    assert!(qml.contains("return \"Ctrl+F5\""));
    assert!(qml.contains("return \"F11\""));
    assert!(qml.contains("browserUi.binding_overlay.length > 0"));
    assert!(qml.contains("event.modifiers & Qt.AltModifier"));
    assert!(qml.contains("event.modifiers & Qt.MetaModifier"));
    assert!(keyboard_presentation.contains("text = text || \"\""));
    assert!(qml.contains("KeyboardPresentation.printableKey("));
    assert!(qml.contains("event.text, event.key, shift, Qt.Key_A, Qt.Key_Z"));
    assert!(qml.contains("ui.handle_key(logicalText)"));
    assert!(qml.contains("function handleBrowserKey(ui, host, event)"));
    assert!(qml.matches("BrowserKeyRouter {").count() >= 2);
    assert!(qml.contains("browserKeyRouter.acceptCurrentEvent()"));
    assert!(qml.contains("secondaryKeyRouter.acceptCurrentEvent()"));
    assert!(qml.contains("readonly property bool browserChromeInputActive:"));
    assert!(qml.contains("browserUi.external_navigation_visible"));
    assert!(qml.contains("window.pendingContextMenuRequest !== null"));
    assert!(qml.contains("browserUi.mode === \"insert\""));
    assert!(qml.contains("browserUi.mode === \"pass-through\""));
    assert!(qml.contains("? controller.modeFocusReturnTarget"));
    assert!(!qml.contains("globalKeyHandler"));
    assert!(!qml.contains("window.handleBrowserKey(viewUi, viewHost, event)"));
    assert!(qml.contains("Accessible.role: Accessible.EditableText"));
}

#[test]
fn browser_key_router_filters_before_webengine_and_requires_explicit_acceptance() {
    let header = include_str!("../browser_key_router.h");
    let source = include_str!("../browser_key_router.cpp");
    assert!(header.contains("QML_NAMED_ELEMENT(BrowserKeyRouter)"));
    assert!(header.contains("Q_PROPERTY(QWindow *targetWindow"));
    assert!(header.contains("Q_INVOKABLE void acceptCurrentEvent()"));
    assert!(source.contains("application->installEventFilter(this)"));
    assert!(source.contains("event->type() == QEvent::KeyPress"));
    assert!(source.contains("event->type() == QEvent::ShortcutOverride"));
    assert!(source.contains("QGuiApplication::focusWindow() != targetWindow_"));
    assert!(source.contains("emit keyPressed("));
    assert!(source.contains("return handled;"));
    assert!(source.contains("nativeScanCode()"));
    assert!(!source.contains("| quint64(quint32(keyEvent->modifiers()))"));
}

#[test]
fn spatial_grid_owns_complete_input_gestures_and_stale_surface_callbacks() {
    let adapter_header = include_str!("../browser_page_pointer_adapter.h");
    let adapter_source = include_str!("../browser_page_pointer_adapter.cpp");
    let overlay = include_str!("../../qml/components/FerricSpatialGridOverlay.qml");
    let qml = QML_SOURCE;

    assert!(adapter_header.contains("QPointer<QQuickItem> targetItem_"));
    assert!(adapter_header.contains("QSet<int> consumedButtons_"));
    assert!(adapter_header.contains("bool consumedTabletSequence_"));
    assert!(adapter_header.contains("bool consumedTouchSequence_"));
    assert!(adapter_header.contains("bool surfaceExhausted_"));
    assert!(adapter_source.contains("QObject::disconnect(targetItem_.data()"));
    assert!(adapter_source.contains("const QPointer<QQuickItem> dispatchTarget = targetItem_"));
    assert!(adapter_source.contains("const QPointer<QWindow> dispatchWindow = targetWindow_"));
    assert!(adapter_source.contains("QCoreApplication::sendEvent(dispatchWindow.data(), &press)"));
    assert!(!adapter_source.contains("QCoreApplication::sendEvent(dispatchTarget.data(), &press)"));
    assert!(adapter_source.contains("event->type() == QEvent::MouseButtonRelease"));
    assert!(adapter_source.contains("consumedButtons_.remove"));
    assert!(adapter_source.contains("QEvent::TabletRelease"));
    assert!(adapter_source.contains("QEvent::TouchEnd"));
    assert!(!adapter_source.contains("consumedButtons_.clear()"));
    assert!(adapter_source.contains("surfaceExhausted_ = true"));
    assert!(!adapter_source.contains("QCursor::setPos"));

    assert!(qml.contains("inputBlocked: window.browserChromeInputActive"));
    assert!(qml.contains("enabled: browserUi.spatial_visible"));
    assert!(!qml.contains("spatialPointerAdapter.enabled = true"));
    assert!(qml.contains("var sessionId = browserUi.spatial_session_id"));
    assert!(qml.contains("browserUi.spatial_session_id !== sessionId"));
    assert!(ADAPTER_SOURCE.contains("#[qproperty(QStringList, spatial_labels)]"));
    assert!(overlay.contains("browserUi.spatial_selecting"));
    assert!(overlay.contains("Accessible.role: Accessible.Indicator"));
    assert!(overlay.contains("readonly property bool labelsFit:"));
    assert!(overlay.contains("anchors.centerIn: parent"));
    assert!(overlay.contains("readonly property real reticleOuterRadius:"));
    assert!(overlay.contains("readonly property real gridLineThickness:"));
    assert!(overlay.contains("readonly property color gridLineHaloColor:"));
    assert!(overlay.contains("Shift+Enter right-clicks"));
    assert!(overlay.contains("Ctrl+Enter middle-clicks"));
    assert!(!overlay.contains("JSON.parse("));
}

#[test]
fn spatial_grid_reports_ready_pending_and_terminal_outcomes() {
    let source = ADAPTER_SOURCE;
    assert!(source.contains("set_spatial_visible(true)"));
    assert!(source.contains("Grid ready"));
    assert!(source.contains("Grid action pending"));
    assert!(source.contains("Grid click delivered"));
    assert!(source.contains("Grid hover delivered; selection reset"));
    assert!(source.contains("Grid denied during macro recording; recording aborted"));
    assert!(source.contains("Grid denied during macro playback; playback aborted"));
    assert!(source.contains("SpatialAck::Inactive(reason)"));
}

#[test]
fn scaling_uses_qt_logical_units_and_bounds_large_font_overlays() {
    let qml = QML_SOURCE;
    let downloads = include_str!("../../qml/components/FerricDownloadManager.qml");
    let modal = include_str!("../../qml/components/FerricModalSurface.qml");
    assert!(qml.contains("Screen.devicePixelRatio"));
    assert!(qml.contains("Screen.logicalPixelDensity"));
    assert!(qml.contains("readonly property real chromeScale"));
    assert!(downloads.contains("dialogWidth: 800 * scale"));
    assert!(modal.contains("surface.width - 32 * surface.scale"));
    assert!(modal.contains("surface.height - 32 * surface.scale"));
    assert!(qml.contains("anchors.bottom: parent.bottom"));
    assert!(qml.contains("double-scale high-DPI"));
}

#[test]
fn activation_uses_qt_request_and_reports_unknown_compositor_outcomes() {
    let qml = QML_SOURCE;
    assert!(qml.contains("hostWindow.requestActivate()"));
    assert!(qml.contains("activationOutcomeTimer"));
    assert!(qml.contains("Window activation outcome unknown"));
    assert!(qml.contains("hostWindow.active"));
    assert!(!qml.contains("xdotool"));
    assert!(!qml.contains("wmctrl"));
}

#[test]
fn authentication_and_client_certificates_stay_in_native_engine_prompts() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricPageDialog.qml"),
        include_str!("../../qml/components/FerricCertificatePrompts.qml"),
    ]
    .concat();
    assert!(qml.contains("request.proxyHost"));
    assert!(qml.contains("HTTP authentication"));
    assert!(qml.contains("Proxy authentication"));
    assert!(qml.contains("authentication ? \"authentication\" : \"page-dialog\""));
    assert!(qml.contains("\"dialogAccept\", [username, password]"));
    assert!(qml.contains("pageDialogPopup.passwordText = \"\""));
    assert!(qml.contains("onSelectClientCertificate"));
    assert!(qml.contains("selection.certificates"));
    assert!(qml.contains("\"client-certificate\", \"select\", [index]"));
    assert!(qml.contains("\"client-certificate\", \"selectNone\", []"));
    assert!(qml.contains("Private keys are never exposed here."));
    assert!(!qml.contains("console.log(username"));
    assert!(!qml.contains("console.log(password"));
}

#[test]
fn qml_qt_request_resolution_is_centralized_and_bounded() {
    let qml = QML_SOURCE;
    assert!(qml.contains("function resolveQtRequest(ui, request, kind, method, args)"));
    assert!(qml.contains("resolved.length > 256"));
    assert!(qml.contains("record_request_resolution"));
    assert!(!qml.contains("request.dialogReject()"));
    assert!(!qml.contains("request.dialogAccept("));
    assert!(!qml.contains("permissionRequest.deny()"));
}

#[test]
fn ordinary_profile_downloads_do_not_probe_the_unsafe_view_getter() {
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let routing = include_str!("../../qml/scripts/ProfileDownloadRouting.js");
    let owner = include_str!("../../qml/components/FerricVersioningWindowOwner.qml");

    assert!(requests.contains("if (download && download.isSavePageDownload !== true)"));
    assert!(requests.contains("ProfileDownloadRouting.boundProfileOwner("));
    assert!(requests.contains("profileHasPoisonedPageCaptureTab("));
    assert!(requests.contains("download, window, browserProfile, true"));
    assert!(requests.contains("if (inspectView === false)\n                return null"));
    assert!(requests.contains("var observedView = null"));
    assert!(requests.contains("window.retireNativePageCaptureDownload(unownedId)"));
    assert!(requests.contains("profile ownership could not be confirmed"));
    assert!(secondary.contains("ProfileDownloadRouting.boundProfileOwner("));
    assert!(secondary.contains("ProfileDownloadRouting.requestOwnerForExactDownload("));
    assert!(!secondary.contains("download.view"));
    assert!(secondary.contains("download.isSavePageDownload === true"));
    assert!(routing.contains("host.browserProfile === profile"));
    assert!(routing.contains("host.windowWebEngineProfile === profile"));
    assert!(owner.contains("function profileHasPoisonedPageCaptureTab(profile)"));
    assert!(owner.contains("poisoned.length > 0"));
    let poison_scan = owner
        .split("function profileHasPoisonedPageCaptureTab(profile)")
        .nth(1)
        .expect("profile-wide save-page poison scan");
    assert!(poison_scan
        .contains("popups === undefined && host !== root.hostWindow"));
    assert_eq!(poison_scan.matches("host.popupWindowRegistry").count(), 1);
    assert!(poison_scan.contains(
        "PopupWindowOwnership.snapshotPopupEntriesFromRegistry(popups)"
    ));
    assert!(poison_scan.contains("popupCount + popupEntries.length > 64"));
    assert!(poison_scan.contains("seenPopupHosts.indexOf(popupHost) >= 0"));
    assert!(!poison_scan.contains("popupHost = entry ? entry.host : null"));
    let popup_ownership = include_str!("../../qml/scripts/PopupWindowOwnership.js");
    assert!(popup_ownership.contains("function snapshotPopupEntriesFromRegistry(popups)"));
    assert!(popup_ownership.contains("popupCount > MAX_POPUP_WINDOWS"));
    let capture_callbacks = include_str!("../../qml/scripts/NativePageCaptureCallbacks.js");
    assert!(capture_callbacks.contains("allowUnpermittedSavePage === true"));
    assert!(!capture_callbacks.contains("nativePageCaptureTabIdForView(download.view)"));
    assert!(!owner.contains("download.view"));
    assert!(!secondary.contains("download.view"));

    let exact_save_start = requests
        .find("// For Ferric's active archive save")
        .expect("exact archive-save route exists");
    let unpermitted_save_start = requests[exact_save_start..]
        .find("\n        if (inspectView !== false && download\n                && download.isSavePageDownload === true)")
        .expect("unpermitted Save Page route follows exact archive-save route")
        + exact_save_start;
    let exact_save_route = &requests[exact_save_start..unpermitted_save_start];
    assert!(exact_save_route.contains("hostForDownload(\n                        download, null, window)"));
    assert!(exact_save_route.contains("activePageCaptureHost("));
    assert!(!exact_save_route.contains("download.view"));
}

#[test]
fn page_capture_revalidates_view_and_document_before_accepting_collected_sidecar() {
    let surface = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let callbacks = include_str!("../../qml/scripts/NativePageCaptureCallbacks.js");
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let collection = surface
        .split("function finishNativePageCaptureCollection(capture, sidecarJson)")
        .nth(1)
        .expect("native page-capture collection completion")
        .split("\n    Timer {")
        .next()
        .expect("native page-capture completion boundary");

    assert!(callbacks.contains("function preSaveCaptureStatus(capture, activeView)"));
    assert!(callbacks.contains("return \"native-view-lost\""));
    assert!(callbacks.contains(
        "return liveUrl === capture.capturedUrl ? \"current\" : \"document-changed\""
    ));
    let revalidation = collection
        .find("preSaveCaptureStatus(")
        .expect("view and document revalidation");
    let sidecar_acceptance = collection
        .find("complete_native_page_capture_sidecar(")
        .expect("sidecar acceptance follows revalidation");
    let save_path_claim = collection
        .find("claim_native_page_capture_save(")
        .expect("native save request follows revalidation");
    assert!(revalidation < sidecar_acceptance);
    assert!(revalidation < save_path_claim);
    assert!(collection.contains("fail_native_window_capture_page("));
    assert!(collection.contains("captureStatus, \"\", \"\""));
    assert!(collection.contains("capture.capturedUrl) || \"\")"));
    assert!(collection.contains("var saveFailureReason = saveFailureStatus === \"current\""));
    assert!(collection.contains("? \"invalid-archive\" : saveFailureStatus"));
    assert!(collection.contains("saveFailureReason, path, \"\""));

    let completion = requests
        .split("function completeNativePageCaptureMutationProbe(capture, facts, evidence)")
        .nth(1)
        .expect("native archive completion validation")
        .split("function cancelNativePageCaptureForShutdown()")
        .next()
        .expect("native archive completion validation boundary");
    assert!(callbacks.contains(
        "function completedArchiveFailure(facts, maximumBytes, requestMatches)"
    ));
    assert!(callbacks.contains("if (requestMatches !== true)\n        return \"invalid-archive\""));
    assert!(callbacks.contains("if (facts.view_matches !== true)\n        return \"native-view-lost\""));
    assert!(completion.contains("completedArchiveFailure("));
    assert!(completion.contains(
        "capture.completionRequestMatches === true"
    ));
    assert!(!requests.contains("request_matches: viewFacts.request_matches"));
    assert!(completion.contains("pageFailure === \"native-view-lost\""));
    assert!(completion.contains("pageFailure === \"document-changed\""));
}

#[test]
fn approved_restore_window_retains_download_and_popup_owners_until_release() {
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let chrome = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let popup = include_str!("../../qml/components/FerricPopupWindow.qml");
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let runtime_base = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let owner = include_str!("../../qml/components/FerricVersioningWindowOwner.qml");
    let routing = include_str!("../../qml/scripts/ProfileDownloadRouting.js");
    let popup_ownership = include_str!("../../qml/scripts/PopupWindowOwnership.js");
    let restore_presentation = include_str!("../../qml/scripts/VersionedRestorePresentation.js");

    assert!(secondary.contains("function requestApprovedWindowRelease()"));
    assert!(secondary.contains("if (secondaryWindow.hasActiveDownloads()) {"));
    assert!(secondary.contains(
        "hasOutstandingDownloadRequests(\n                    secondaryWindow.pendingDownloadRequests)"
    ));
    assert!(popup.contains(
        "hasOutstandingDownloadRequests(\n                    popupWindow.pendingDownloadRequests)"
    ));
    assert!(presentation.contains(
        "hasOutstandingDownloadRequests(\n                    window.pendingDownloadRequests)"
    ));
    assert!(secondary.contains("function hasOwnedPopupWindows()"));
    assert!(secondary.contains("secondaryWindow.hasOwnedPopupWindows()"));
    assert!(secondary.contains("PopupWindowOwnership.hasOwnedPopupWindows("));
    let popup_download_request = popup
        .split("function handleDownloadRequested(download)")
        .nth(1)
        .expect("popup profile download request handler")
        .split("\n    function handleDownloadFinished(download)")
        .next()
        .expect("popup profile download request handler boundary");
    let popup_guard = popup_download_request
        .find("ProfileDownloadRouting.retainAndCancelIfShutdownApproved(")
        .expect("popup request is retained before UI admission");
    assert!(popup_download_request[popup_guard..].contains("return rootWindow.resolveQtRequest("));
    let popup_chooser = popup_download_request[popup_guard..]
        .find("ui.offer_download(")
        .map(|offset| popup_guard + offset)
        .expect("popup chooser follows owner retention");
    assert!(popup_guard < popup_chooser);
    let secondary_download_owner = secondary
        .split("function downloadOwnerFor(download, inspectView)")
        .nth(1)
        .expect("restore-window download owner")
        .split("\n    // A shared-profile secondary receives its profile-wide signals")
        .next()
        .expect("restore-window download owner boundary");
    assert!(secondary_download_owner.contains(
        "PopupWindowOwnership.snapshotPopupHosts(rootWindow)"
    ));
    assert!(secondary_download_owner.contains("popupHosts === null"));
    assert!(secondary_download_owner.contains(
        "ProfileDownloadRouting.requestOwnerForExactDownload("
    ));
    assert!(secondary_download_owner.contains("if (exactOwner === undefined)"));
    assert!(!secondary_download_owner.contains("activeDownloads || ({})"));
    assert!(!secondary_download_owner.contains("pendingDownloadRequests || ({})"));
    assert!(!secondary_download_owner.contains("downloadCompletionRequests || ({})"));
    assert!(secondary_download_owner.contains("catch (error)"));
    assert!(secondary_download_owner.contains("return undefined"));
    assert!(!secondary_download_owner.contains("rootWindow.popupWindowRegistry || []"));
    assert!(routing.contains(
        "function requestOwnerForExactDownload(download, sourceHost, popupHosts, profile)"
    ));
    let outstanding_completion = routing
        .split("function hasOutstandingDownloadRequests(downloads)")
        .nth(1)
        .expect("download shutdown ledger predicate")
        .split("\nfunction isRecord(")
        .next()
        .expect("download shutdown ledger predicate boundary");
    assert!(outstanding_completion.contains("if (!isRecord(downloads))\n            return true"));
    assert!(outstanding_completion.contains("Object.prototype.hasOwnProperty.call(downloads, id)"));
    assert!(outstanding_completion.contains("catch (error) {\n        return true"));
    assert!(routing.contains("var popupProfile = popupHost.popupProfile"));
    assert!(routing.contains("indexedDownload !== download"));
    assert!(popup_ownership.contains("var MAX_POPUP_WINDOWS = 64"));
    assert!(popup_ownership.contains("function snapshotPopupHosts(rootWindow)"));
    assert!(popup_ownership.contains("function snapshotPopupEntries(rootWindow)"));
    assert!(popup_ownership.contains("if (!host || hosts.indexOf(host) >= 0)"));
    assert!(restore_presentation.contains("owner.canCreate() === true"));
    assert!(restore_presentation.contains("owner.retain(child) === true"));
    assert!(restore_presentation.contains("if (child && typeof child.destroy === \"function\")"));
    assert!(restore_presentation.contains("catch (error) {\n        retained = false"));
    assert!(popup_ownership.contains("popups.length >= MAX_POPUP_WINDOWS"));
    assert!(popup_ownership.contains("popups.length > MAX_POPUP_WINDOWS"));
    assert!(popup_ownership.contains("function canRegisterPopupWindow(rootWindow)"));
    let popup_registration_helper = popup_ownership
        .split("function registerPopupWindow(rootWindow, hostWindow, openerUi, view)")
        .nth(1)
        .expect("popup registration revalidates ownership at publication");
    assert!(popup_registration_helper.contains("popups.length > MAX_POPUP_WINDOWS"));
    assert!(popup_registration_helper.contains("retained.length >= MAX_POPUP_WINDOWS"));
    assert!(popup_registration_helper.contains("applicationShutdownInProgress === true"));
    assert!(popup_registration_helper.contains("shutdownApproved === true"));
    assert!(popup_registration_helper.contains("catch (error)"));
    assert!(popup_ownership.contains("applicationShutdownInProgress === true"));
    assert!(popup_ownership.contains("shutdownApproved === true"));
    assert!(runtime_base.contains("function canRegisterPopupWindow()"));
    assert!(runtime_base
        .contains("return PopupWindowOwnership.canRegisterPopupWindow(window)"));
    assert!(runtime_base.contains("PopupWindowOwnership.registerPopupWindow("));
    let unregister_helper = runtime_base
        .split("function unregisterPopupWindow(hostWindow)")
        .nth(1)
        .expect("popup unregistration helper");
    assert!(unregister_helper.contains(
        "return PopupWindowOwnership.unregisterPopupWindow(window, hostWindow)"
    ));
    let popup_unregistration_helper = popup_ownership
        .split("function unregisterPopupWindow(rootWindow, hostWindow)")
        .nth(1)
        .expect("popup unregistration revalidates the bounded owner registry");
    assert!(popup_unregistration_helper.contains("popups.length > MAX_POPUP_WINDOWS"));
    assert!(popup_unregistration_helper.contains("catch (error)"));
    assert!(popup_ownership.contains("return true"));
    assert!(secondary.contains("ProfileDownloadRouting.retainAndCancelIfShutdownApproved("));
    assert!(requests.contains("ProfileDownloadRouting.retainAndCancelIfShutdownApproved("));
    assert!(secondary.contains("secondaryWindow.requestApprovedWindowRelease()"));
    assert!(owner.contains("function releaseWhenApproved(child)"));
    assert!(owner.contains("child.requestApprovedWindowRelease()"));
    assert!(owner.contains("typeof child.hasActiveDownloads !== \"function\""));
    assert!(owner.contains("|| child.hasActiveDownloads()"));
    assert!(owner.contains("child.hasOwnedPopupWindows() !== false"));
    assert!(chrome.contains("popupOpenerHost: window"));
    assert!(secondary.contains("popupOpenerHost: secondaryWindow"));
    let primary_popup_admission = chrome
        .find("if (!window.canRegisterPopupWindow())")
        .expect("primary popup admission checks the shared bound");
    let primary_popup_creation = chrome
        .find("popupWindowComponent.createObject(null, {")
        .expect("primary popup creation boundary");
    assert!(primary_popup_admission < primary_popup_creation);
    let restore_popup_admission = secondary
        .find("if (!rootWindow.canRegisterPopupWindow())")
        .expect("restore popup admission checks the shared bound");
    let restore_popup_creation = secondary
        .find("rootWindow.popupWindowFactory.createObject(null, {")
        .expect("restore popup creation boundary");
    assert!(restore_popup_admission < restore_popup_creation);
    let popup_registration = popup
        .find("if (!rootWindow.registerPopupWindow(")
        .expect("popup registration rechecks its admission");
    let popup_open = popup
        .find("popupWindow.popupRequest.openIn(popupView)")
        .expect("only a registered popup is opened");
    assert!(popup_registration < popup_open);
    assert!(popup.contains("property var popupOpenerHost: null"));
    let popup_destruction = popup
        .rfind("Component.onDestruction: {")
        .map(|start| &popup[start..])
        .expect("popup destruction releases its opener owner");
    let unregister_popup = popup_destruction
        .find("rootWindow.unregisterPopupWindow(popupWindow)")
        .expect("popup unregisters before owner release");
    let opener_release = popup_destruction
        .find("openerHost.requestApprovedWindowRelease()")
        .expect("approved opener release is retried after popup teardown");
    assert!(unregister_popup < opener_release);
    assert!(owner.contains("function retain(child)"));
    assert!(owner.contains("var published = false"));
    assert!(owner.contains("child.versioningDiscoveryChanged.disconnect(discoveryHandler)"));
    assert!(owner.contains("child.visibleChanged.disconnect(visibleHandler)"));
    let restore_owner_download = owner
        .split("function hostForDownload(download, observedView, sourceHost)")
        .nth(1)
        .expect("restore owner download routing")
        .split("\n    function requestCloseAll()")
        .next()
        .expect("restore owner download routing boundary");
    assert!(restore_owner_download.contains("PopupWindowOwnership.snapshotPopupEntries(child)"));
    assert!(restore_owner_download.contains(
        "PopupWindowOwnership.snapshotPopupEntriesFromRegistry(\n                                childPopupRegistry)"
    ));
    assert!(restore_owner_download.contains("popupEntries === null"));
    assert!(restore_owner_download.contains("seenPopupHosts.indexOf(popupHost) >= 0"));
    assert!(!restore_owner_download.contains("child.popupWindowRegistry || []"));
    assert!(restore_owner_download.contains(
        "function requestIdentityMatches(host, requestId, request)"
    ));
    assert!(restore_owner_download.contains("function indexedValue(index, key)"));
    assert!(restore_owner_download.contains("var views = child.tabViews"));
    assert!(!restore_owner_download.contains("child.activeDownloads || ({})"));
    assert!(!restore_owner_download.contains("child.pendingDownloadRequests || ({})"));
    assert!(!restore_owner_download.contains("child.downloadCompletionRequests || ({})"));
    assert!(!restore_owner_download.contains("popupHost.activeDownloads || ({})"));
    assert!(!restore_owner_download.contains(
        "popupHost.pendingDownloadRequests || ({})"
    ));
    assert!(!restore_owner_download.contains(
        "popupHost.downloadCompletionRequests || ({})"
    ));
    assert!(!restore_owner_download.contains("var views = child.tabViews || []"));
    assert!(!restore_owner_download
        .contains("host.nativePageCaptureRetiredDownloads || ({})"));
    assert!(restore_owner_download.contains(
        "function retiredSaveIdentityMatches(host, allowAbsentIndexes)"
    ));
    assert!(restore_owner_download.contains("} catch (error) {\n            return undefined\n        }"));

    let shutdown_count = runtime_base
        .split("function shutdownParticipantCount()")
        .nth(1)
        .expect("shutdown participant counter")
        .split("\n    function abortApplicationShutdown()")
        .next()
        .expect("shutdown count boundary");
    assert!(shutdown_count.contains("PopupWindowOwnership.snapshotPopupHosts(window)"));
    assert!(shutdown_count.contains("popupHosts === null ? count + 1"));
    assert!(shutdown_count.contains("count + popupHosts.length"));

    let shutdown_begin = runtime_base
        .split("function beginApplicationShutdown()")
        .nth(1)
        .expect("shutdown begin handler")
        .split("\n    function forceApplicationShutdown()")
        .next()
        .expect("shutdown begin boundary");
    let popup_preflight = shutdown_begin
        .find("PopupWindowOwnership.snapshotPopupHosts(window)")
        .expect("shutdown preflights popup owner snapshot");
    let admission_close = shutdown_begin
        .find("window.applicationShutdownInProgress = true")
        .expect("shutdown closes popup admission");
    let gate_call = shutdown_begin
        .find("browserUi.begin_shutdown_gate()")
        .expect("shutdown starts browser UI gate");
    assert!(popup_preflight < admission_close && admission_close < gate_call);
    assert!(shutdown_begin.contains("var popup = popupHosts[j]"));
    assert!(shutdown_begin.contains("window.applicationShutdownInProgress = false"));

    let shutdown_abort = runtime_base
        .split("function abortApplicationShutdown()")
        .nth(1)
        .expect("shutdown cancellation handler")
        .split("\n    function finishApplicationShutdown()")
        .next()
        .expect("shutdown cancellation boundary");
    assert!(shutdown_abort.contains("PopupWindowOwnership.snapshotPopupHosts(window)"));
    assert!(shutdown_abort.contains("if (popupHosts !== null)"));
    assert!(shutdown_abort.contains("popup ownership is unavailable"));

    let shutdown_guard = routing
        .split("function retainAndCancelIfShutdownApproved(")
        .nth(1)
        .expect("shared shutdown download guard")
        .split("\nfunction resolve(")
        .next()
        .expect("shutdown guard boundary");
    let owner_recorded = shutdown_guard
        .find("completionRequests[id] = download")
        .expect("exact request owner is retained");
    let approved_close = shutdown_guard[owner_recorded..]
        .find("shutdownApproved = host.shutdownApproved === true")
        .map(|offset| owner_recorded + offset)
        .expect("root and restore shutdown states are checked");
    assert!(shutdown_guard.contains("host.windowShutdownApproved === true"));
    let cancelled = shutdown_guard[approved_close..]
        .find("cancelSafely()")
        .map(|offset| approved_close + offset)
        .expect("request is canceled after retaining ownership");
    assert!(owner_recorded < approved_close && approved_close < cancelled);
    assert!(shutdown_guard.contains("/^\\d{1,20}$/"));
    assert!(shutdown_guard.contains("!isRecord(completionRequests)"));
    assert!(shutdown_guard.contains(
        "Object.prototype.hasOwnProperty.call(completionRequests, id)"
    ));
    assert!(shutdown_guard.contains("completionRequests[id] !== download"));
    assert!(shutdown_guard.contains("catch (error) {\n        cancelSafely()"));
    assert!(shutdown_guard.contains("cancellationResolved = cancelRequest(download) === true"));
    assert!(shutdown_guard.contains("if (!cancellationResolved)"));
    assert!(shutdown_guard.contains("download.cancel()"));

    let request = secondary
        .split("function handleForwardedDownloadRequested(download)")
        .nth(1)
        .expect("profile download request handler");
    let guard_call = request
        .find("ProfileDownloadRouting.retainAndCancelIfShutdownApproved(")
        .expect("secondary request uses shared shutdown guard");
    assert!(request[guard_call..].contains("return rootWindow.resolveQtRequest("));
    let chooser = request[guard_call..]
        .find("secondaryUi.offer_download")
        .map(|offset| guard_call + offset)
        .expect("ordinary chooser follows shutdown guard");
    assert!(guard_call < chooser);

    let primary_request = requests
        .split("function handleDownloadRequested(download)")
        .nth(1)
        .expect("primary profile download request handler");
    let primary_guard = primary_request
        .find("ProfileDownloadRouting.retainAndCancelIfShutdownApproved(")
        .expect("primary request uses shared shutdown guard");
    assert!(primary_request[primary_guard..].contains("return window.resolveQtRequest("));
    let primary_chooser = primary_request[primary_guard..]
        .find("browserUi.offer_download")
        .map(|offset| primary_guard + offset)
        .expect("primary chooser follows shutdown guard");
    assert!(primary_guard < primary_chooser);

    let finish = secondary
        .split("function handleForwardedDownloadFinished(download)")
        .nth(1)
        .expect("profile download terminal handler")
        .split("onDownloadFinished:")
        .next()
        .expect("profile download terminal handler boundary");
    let released = finish
        .find("delete secondaryWindow.downloadCompletionRequests[id]")
        .expect("terminal callback removes its exact completion owner");
    let retried = finish
        .find("secondaryWindow.requestApprovedWindowRelease()")
        .expect("terminal callback retries approved owner release");
    assert!(released < retried);
}

#[test]
fn extracted_prompt_components_render_state_and_emit_intents_without_engine_policy() {
    let main = QML_SOURCE;
    let certificate = include_str!("../../qml/components/FerricCertificatePrompts.qml");
    let webauth = include_str!("../../qml/components/FerricWebAuthPrompt.qml");
    let context_menu = include_str!("../../qml/components/FerricContextMenu.qml");
    let userscript_removal = include_str!("../../qml/components/FerricUserscriptRemovalDialog.qml");

    assert!(main.contains("FerricCertificatePrompts"));
    assert!(main.contains("FerricWebAuthPrompt"));
    assert!(main.contains("FerricContextMenu"));
    assert!(main.contains("FerricUserscriptRemovalDialog"));
    assert!(certificate.contains("signal clientCertificateAccepted(int index)"));
    assert!(certificate.contains("signal certificateAccepted()"));
    assert!(webauth.contains("signal pinSubmitted()"));
    assert!(webauth.contains("signal retryRequested()"));
    assert!(context_menu.contains("signal itemActivated(var item)"));
    assert!(context_menu.contains("signal dismissed()"));
    assert!(userscript_removal.contains("signal confirmed()"));
    for component in [certificate, webauth, context_menu, userscript_removal] {
        assert!(!component.contains("browserUi."));
        assert!(!component.contains("resolveQtRequest"));
    }
    assert!(!userscript_removal.contains("remove_userscript"));
}

#[test]
fn account_flows_keep_tabs_and_popups_on_the_opener_profile() {
    let qml = QML_SOURCE;
    assert!(qml.contains("profile: viewProfile"));
    assert!(qml.contains("property var viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("popupProfile: viewProfile"));
    assert!(qml.contains("viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("popupPrivateProfile: viewTransientProfile"));
    assert!(qml.contains("viewTransientProfile: secondaryWindow.windowTransientProfile"));
    assert!(qml.contains("storageName: profileHost.browserWindow.temporaryProfile"));
    assert!(qml.contains("? \"\" : \"ferric-browser-\" + profileHost.browserWindow.profileName"));
    assert!(qml.contains("storageName: secondaryWindow.windowTransientProfile ? \"\""));
    assert!(!qml.contains("new WebEngineProfile"));
}

#[test]
fn context_routes_validate_before_assigning_window_membership() {
    let source = ADAPTER_SOURCE;
    let command_start = source
        .find("fn execute_ipc_command_with_operation(")
        .expect("IPC command executor exists");
    let command_source = &source[command_start..];
    let assign = command_source
        .find("self.as_mut().assign_ipc_context(route)?;")
        .expect("context assignment exists");
    let validate = command_source
        .find("self.as_ref().validate_ipc_route(route)?;")
        .expect("route validation exists");
    assert!(validate < assign);
}

#[test]
fn context_routes_are_pre_navigation_and_browser_confirmed() {
    let context_navigation = include_str!("../browser_ui_context_navigation.rs");
    let context_switching = include_str!("../browser_ui_contexts.rs");
    let source = [
        ADAPTER_SOURCE,
        context_navigation,
        include_str!("../tab_presentation.rs"),
    ]
    .concat();
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricContextRouteDialog.qml"),
    ]
    .concat();
    assert!(source.contains("matching_context_routes"));
    assert!(source.contains("queue_context_route_for_input"));
    assert!(source.contains("execute_context_route_command"));
    assert!(source.contains("save_contexts_atomic"));
    assert!(source.contains("fn navigate_initial"));
    assert!(source.contains("NavigationSource::ExplicitUrl"));
    assert!(source.contains("fn accept_context_route"));
    assert!(source.contains("fn dismiss_context_route"));
    assert!(context_navigation.contains("\"Context route accepted\""));
    assert!(context_navigation.contains(
        "\"Context route accepted; opening the selected profile\""
    ));
    assert!(!context_navigation.contains("Context route accepted: {}"));
    assert!(!context_navigation.contains("Context route accepted; opening {} in profile {}"));
    assert!(context_switching.contains("\"Context requires a separate profile window\""));
    assert!(context_switching.contains("\"Context is already open in another window\""));
    assert!(!context_switching.contains("Context {} requires a separate {} window"));
    assert!(!context_switching.contains("Context {} is already open in another window"));
    assert!(source.contains("fn sync_context_metadata"));
    assert!(source.contains("context_workspace"));
    assert!(qml.contains("context_route_id"));
    assert!(!qml.contains("context_route_json"));
    assert!(qml.contains("Context route confirmation"));
    assert!(qml.contains("contextStatus"));
    assert!(qml.contains("contextStatusColor"));
    assert!(qml.contains("function routeContextWorkspace(ui)"));
    assert!(qml.contains("entry.host.active !== true"));
    assert!(qml.contains("Workspace routing pending until the browser window is active"));
    assert!(qml.contains("pendingWorkspaceRouteHost"));
    assert!(qml.contains("onContext_workspaceChanged"));
    assert!(qml.contains(
            "Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
        ));
    assert!(qml.contains("windowStartupContext"));
}

#[test]
fn context_move_choices_are_a_typed_qml_projection() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("fn set_contexts_configuration"));
    assert!(source.contains("set_context_choice_names"));
    assert!(source.contains("set_context_choice_labels"));
    assert!(source.contains("set_context_choice_profiles"));
    assert!(qml.contains("context_choice_names"));
    assert!(qml.contains("set_contexts_configuration("));
    assert!(!qml.contains("JSON.parse(browserUi.contexts_json"));
}

#[test]
fn live_window_registry_decoder_requires_complete_typed_rows() {
    let ids = vec!["window-1".into()];
    let owners = vec!["owner-1".into()];
    let profiles = vec!["default".into()];
    let private = vec!["false".into()];
    let ephemeral = vec!["true".into()];
    let tabs = vec!["2".into()];

    let decoded =
        decode_live_window_registry(&ids, &owners, &profiles, &private, &ephemeral, &tabs)
            .expect("complete typed registry row");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].id, "window-1");
    assert!(decoded[0].ephemeral);
    assert_eq!(decoded[0].tab_count, 2);

    assert!(
        decode_live_window_registry(
            &ids,
            &owners,
            &profiles,
            &["not-a-bool".into()],
            &ephemeral,
            &tabs,
        )
        .is_none()
    );
    assert!(
        decode_live_window_registry(
            &ids,
            &owners,
            &profiles,
            &private,
            &ephemeral,
            &["1000001".into()],
        )
        .is_none()
    );
    assert!(
        decode_live_window_registry(&ids, &owners, &profiles, &private, &ephemeral, &[],).is_none()
    );
}

#[test]
fn live_window_registry_never_uses_qml_json() {
    let source = include_str!("../window_registry.rs");
    let qml = QML_SOURCE;

    assert!(source.contains("fn publish_live_window_registry"));
    assert!(source.contains("decode_live_window_registry"));
    assert!(qml.contains("publish_live_window_registry("));
    assert!(!source.contains("window_registry_json"));
    assert!(!qml.contains("window_registry_json"));
}

#[test]
fn qml_json_contract_allowlist_is_explicit() {
    let qml = QML_SOURCE;
    let allowlist = include_str!("../../../../docs/architecture/qml-json-contracts.md");

    assert!(allowlist.contains("Opaque page-script request/result contracts"));
    assert!(allowlist.contains("Prohibited presentation payloads"));
    assert_eq!(qml.matches("JSON.parse(").count(), 17);
    assert_eq!(qml.matches("JSON.stringify(").count(), 30);
}

#[test]
fn qml_submits_runtime_intents_instead_of_writing_runtime_state() {
    let qml = QML_SOURCE;
    let bridge_references = [
        "browserUi.",
        "secondaryUi.",
        "viewUi.",
        "ownerWindow.browserUi.",
    ];

    for line in qml.lines().filter(|line| line.contains(" = ")) {
        if bridge_references
            .iter()
            .any(|reference| line.trim_start().starts_with(reference))
        {
            // Status narration is transient presentation output from native
            // callbacks. Every application decision must instead be sent
            // through a named bridge request or a registered command.
            assert!(
                line.contains(".status_text ="),
                "QML directly writes runtime state: {line}"
            );
        }
    }
}

#[test]
fn download_desktop_actions_cross_the_qml_boundary_as_a_typed_uri() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("#[qproperty(QString, download_desktop_uri)]"));
    assert!(source.contains("fn resolve_download_desktop_uri("));
    assert!(source.contains(
        "fn download_desktop_action(self: Pin<&mut BrowserUi>, id: &QString, reveal: bool) -> bool;"
    ));
    assert!(qml.contains("if (!browserUi.download_desktop_action(id, reveal))"));
    assert!(qml.contains("window.openExternalUri(browserUi, browserUi.download_desktop_uri)"));
}

#[test]
fn download_activation_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("#[qproperty(QString, download_request_token)]"));
    assert!(source.contains("#[qproperty(QString, download_request_url)]"));
    assert!(source.contains("fn take_download_request(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("ui.download_request_url"));
    assert!(qml.contains("browserUi.download_request_token"));
    assert!(!qml.contains("JSON.parse(downloadRequest)"));
}

#[test]
fn caret_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("#[qproperty(QString, caret_request_token)]"));
    assert!(source.contains("#[qproperty(QString, caret_request_operation)]"));
    assert!(source.contains("fn take_caret_request(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("var caretToken = browserUi.caret_request_token"));
    assert!(qml.contains("window.caretScript(caretOperation, window.caretSelecting)"));
    assert!(!qml.contains("JSON.parse(caretRequest)"));
}

#[test]
fn editor_completion_records_cross_the_qml_boundary_as_typed_fields() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("#[qproperty(QString, editor_completion_token)]"));
    assert!(!source.contains("editor_completion_stderr"));
    assert!(source.contains("fn take_editor_completion(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("var editorToken = browserUi.editor_completion_token"));
    assert!(qml.contains("window.editorApplyScript(editorOriginal, editorUpdated)"));
    assert!(!qml.contains("JSON.parse(editorCompletion)"));
    assert!(!qml.contains("editorStderr"));
    assert!(!qml.contains("editorError += \" (stderr: \""));
    assert!(qml.contains("external-editor-failed"));
}

#[test]
fn external_editor_output_and_config_paths_do_not_reach_status_projections() {
    let editor = include_str!("../browser_ui_external_actions.rs");
    assert!(editor.contains(".stderr(Stdio::null())"));
    assert!(!editor.contains("read_bounded(stderr"));
    assert!(!editor.contains("join_editor_stderr"));

    let config_edit = include_str!("../browser_ui_edit_commands.rs");
    assert!(config_edit.contains(
        "configured file-backed configuration is unavailable\".to_owned()"
    ));
    assert!(config_edit.contains("configuration editor could not start\".to_owned()"));
    assert!(config_edit.contains(".stderr(Stdio::null())"));
    assert!(!config_edit.contains("configured file-backed configuration is unavailable: {error}"));
    assert!(!config_edit.contains("configuration editor could not start: {error}"));
    assert!(!config_edit.contains("\"path\": path"));

    let config_status = include_str!("../browser_ui_configuration.rs");
    assert!(config_status.contains("Configuration editor finished; reloading configuration"));
    assert!(config_status.contains("Configuration editor exited unsuccessfully"));
    assert!(!config_status.contains("pending.path.display()"));
    assert!(!config_status.contains("stderr_detail"));

    let export = config_status
        .split_once("fn execute_config_export_command")
        .expect("config export command exists")
        .1
        .split_once("fn execute_config_check_command")
        .expect("config check follows export")
        .0;
    assert!(export.contains("Configuration export queued"));
    assert!(!export.contains("path.display()"));

    let defaults = config_status
        .split_once("fn execute_config_write_defaults_command")
        .expect("default config writer exists")
        .1
        .split_once("fn execute_theme_reload_command")
        .expect("theme reload follows default config writer")
        .0;
    assert!(defaults.contains("Default configuration write queued"));
    assert!(!defaults.contains("path.display()"));

    let config_check = config_status
        .split_once("fn execute_config_check_command")
        .expect("config check command exists")
        .1
        .split_once("fn execute_config_reload_command")
        .expect("config reload follows check")
        .0;
    assert!(config_check.contains("\"source\": \"configured-file\""));
    assert!(!config_check.contains("\"source\": path"));

    let config_reload = config_status
        .split_once("fn execute_config_reload_command")
        .expect("config reload command exists")
        .1
        .split_once("fn execute_config_write_defaults_command")
        .expect("default config writer follows reload")
        .0;
    assert!(config_reload.contains("\"source\": \"configured-file\""));
    assert!(!config_reload.contains("\"source\": path"));

    let theme_reload = config_status
        .split_once("fn execute_theme_reload_command")
        .expect("theme reload command exists")
        .1
        .split_once("fn poll_config_edit")
        .expect("configuration editor polling follows theme reload")
        .0;
    assert!(theme_reload.contains("\"path\": \"[redacted]\""));
    assert!(theme_reload.contains("\"palette\": palette_value"));
    assert!(!theme_reload.contains("\"palette\": serde_json::to_value(&palette)"));

    let config_completion = include_str!("../browser_ui_runtime_config.rs");
    assert!(config_completion.contains("Configuration written ({} bytes)"));
    assert!(!config_completion.contains("Configuration written to {}"));
    assert!(!config_completion.contains("result.path.display()"));

    let profile_delete_completion = config_completion
        .split_once("let profile_delete_result")
        .expect("profile delete completion poll exists")
        .1
        .split_once("self.as_mut().poll_config_edit()")
        .expect("configuration editor poll follows profile deletion")
        .0;
    assert!(profile_delete_completion.contains("Profile deleted. QtWebEngine storage was not deleted."));
    assert!(profile_delete_completion.contains("Profile deleted; data cleanup is pending"));
    assert!(!profile_delete_completion.contains("outcome.profile_name"));
}

#[test]
fn jseval_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("#[qproperty(QString, jseval_tab_id)]"));
    assert!(source.contains("set_jseval_script"));
    assert!(qml.contains("if (action === \"jseval\")"));
    assert!(!qml.contains("evalPayload = JSON.parse("));
    assert!(!qml.contains("secondaryEvalPayload = JSON.parse("));
}

#[test]
fn blocking_host_lists_cross_the_qml_boundary_as_string_lists() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    for property in [
        "blocking_hosts",
        "blocking_exceptions",
        "blocking_bypass_sites",
        "blocking_security_deny_hosts",
    ] {
        assert!(source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(!qml.contains(&format!("JSON.parse(browserUi.{property})")));
        assert!(!qml.contains(&format!("JSON.parse(secondaryUi.{property})")));
    }
    assert!(qml.contains("blockedHosts: browserUi.blocking_hosts"));
    assert!(qml.contains("blockedHosts: secondaryUi.blocking_hosts"));
    assert!(!qml.contains("JSON.parse(browserUi.blocking_rule_"));
    assert!(!qml.contains("JSON.parse(secondaryUi.blocking_rule_"));
    assert!(qml.contains("blockedRuleHosts: browserUi.blocking_rule_hosts"));
    assert!(qml.contains("blockedRuleHosts: secondaryUi.blocking_rule_hosts"));
    for property in [
        "blocking_cosmetic_rule_hosts",
        "blocking_cosmetic_rule_selectors",
        "blocking_cosmetic_exception_hosts",
        "blocking_cosmetic_exception_selectors",
    ] {
        assert!(source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.blocking_cosmetic_"));
}

#[test]
fn userscript_inventory_crosses_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("../userscript_presentation.rs");
    let qml = QML_SOURCE;

    for property in [
        "userscript_names",
        "userscript_enabled_values",
        "userscript_page_world_values",
        "userscript_action_counts",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(qml.contains(&format!("browserUi.{property}")));
    }
    assert!(!qml.contains("JSON.parse(browserUi.userscript_inventory"));
    assert!(qml.contains("names.length !== enabled.length"));
}

#[test]
fn userscript_actions_cross_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("../userscript_presentation.rs");
    let qml = QML_SOURCE;

    assert!(projection.contains("fn select_userscript_action_subject"));
    for property in [
        "userscript_action_ids",
        "userscript_action_labels",
        "userscript_action_availability",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(browserUi.userscript_actions"));
    assert!(!qml.contains("ui.userscript_actions(subject)"));
}

#[test]
fn page_userscript_metadata_crosses_the_qml_boundary_as_typed_columns() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("fn select_matching_page_scripts"));
    for property in [
        "page_userscript_names",
        "page_userscript_sources",
        "page_userscript_run_at",
        "page_userscript_runs_on_sub_frames",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.matching_page_scripts("));
    assert!(!qml.contains("ui.matching_page_scripts(url, privateProfile)"));
}

#[test]
fn site_rules_cross_the_qml_boundary_as_typed_properties() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("fn select_site_rule_settings"));
    for property in [
        "site_rule_javascript_set",
        "site_rule_images_set",
        "site_rule_force_dark_set",
        "site_rule_autoplay_set",
        "site_rule_zoom_set",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.site_rule_settings(url))"));
}

#[test]
fn external_actions_cross_the_qml_boundary_as_typed_columns() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(source.contains("fn select_external_action_subject"));
    for property in [
        "external_action_ids",
        "external_action_labels",
        "external_action_availability",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(raw || \"[]\")"));
    assert!(!qml.contains("ui.external_action_values(subject)"));
}

#[test]
fn journey_graph_edges_cross_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("../library_presentation.rs");
    let source = ADAPTER_SOURCE;
    let production_source = source;
    let qml = QML_SOURCE;

    for property in [
        "library_graph_edge_sources",
        "library_graph_edge_targets",
        "library_graph_edge_transitions",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(production_source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(qml.contains(&format!("browserUi.{property}")));
    }
    assert!(!production_source.contains("library_graph_values"));
    assert!(!qml.contains("JSON.parse(browserUi.library_graph_values)"));
}

#[test]
fn blocking_policy_projection_is_separate_from_the_qt_bootstrap() {
    let projection = include_str!("../blocking_presentation.rs");
    let bootstrap = ADAPTER_SOURCE;
    let production_bootstrap = bootstrap
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("production source precedes the test module");

    assert!(projection.contains("fn apply_network_policy_snapshot"));
    assert!(projection.contains("fn reload_blocking_policy"));
    assert!(projection.contains("fn toggle_blocking_site"));
    assert!(!production_bootstrap.contains("fn apply_network_policy_snapshot"));
}

#[test]
fn blocking_evidence_uses_bounded_native_rows_not_qml_json() {
    let qml = QML_SOURCE;
    let projection = include_str!("../blocking_evidence.rs");
    let native_header = include_str!("../request_interceptor.h");

    assert!(qml.contains("set_blocking_active_evidence("));
    assert!(qml.contains("publish_blocking_live_counts("));
    assert!(qml.contains("blockedRequestExplanationFields(activeHost)"));
    assert!(!qml.contains("blocking_active_explanation = JSON.stringify"));
    assert!(!qml.contains("blocking_active_decisions = JSON.stringify"));
    assert!(!qml.contains("browserUi.blocking_blocked_count ="));
    assert!(!qml.contains("browserUi.blocking_unknown_context_count ="));
    assert!(projection.contains("const EVIDENCE_FIELD_COUNT: usize = 13"));
    assert!(projection.contains("const MAX_DECISIONS: usize = 100"));
    assert!(native_header.contains("blockedRequestDecisionFields"));
}

#[test]
fn desktop_multiple_url_launches_open_separate_startup_tabs() {
    let qml = QML_SOURCE;
    let executable = include_str!("../../../ferric-browser/src/main.rs");
    assert!(qml.contains("property var startupAdditionalUrls: []"));
    assert!(qml.contains("property bool startupBackground: false"));
    assert!(qml.contains("function openAdditionalStartupUrls()"));
    assert!(qml.contains("var urls = window.startupAdditionalUrls || []"));
    assert!(executable.contains("QString::from(\"startupAdditionalUrls\")"));
    assert!(executable.contains("collect::<QStringList>()"));
    assert!(!qml.contains("startupAdditionalUrlsJson"));
    assert!(qml.contains("browserUi.new_tab()"));
    assert!(qml.contains("browserUi.navigate_initial(url, \"external-open\", true)"));
    assert!(qml.contains("Qt.callLater(window.openAdditionalStartupUrls)"));
    assert!(qml.contains("if (window.startupBackground)"));
}

#[test]
fn tab_projection_changes_use_qt_property_setters() {
    let source = include_str!("../tab_presentation.rs");
    let direct_tab_count_assignment = ["this.", "tab_count", " ="].concat();
    let direct_active_index_assignment = ["this.", "active_tab_index", " ="].concat();

    assert!(!source.contains(&direct_tab_count_assignment));
    assert!(!source.contains(&direct_active_index_assignment));
    assert!(source.contains("self.as_mut().set_tab_count(tab_count)"));
    assert!(source.contains("self.as_mut().set_active_tab_properties(active_index, tab)"));
}

#[test]
fn status_surfaces_report_bounded_navigation_and_activity_state() {
    let qml = QML_SOURCE;
    let status_bar = include_str!("../../qml/components/FerricStatusBar.qml");
    let window_status_bar = include_str!("../../qml/components/FerricWindowStatusBar.qml");
    assert!(qml.contains("function statusTransport(view)"));
    assert!(qml.contains("HTTPS transport (site trust separate)"));
    assert!(qml.contains("HTTP transport (not secure)"));
    assert!(qml.contains("function statusLoad(view)"));
    assert!(qml.contains("function statusMedia(view)"));
    assert!(qml.contains("function statusPermission(ui)"));
    assert!(qml.contains("function statusCapture(hostWindow)"));
    assert!(qml.contains("function statusDownloads(hostWindow)"));
    assert!(qml.contains("function statusTabBlocking(view)"));
    assert!(qml.contains("function statusDetails(ui, hostWindow, view"));
    assert!(qml.contains("browserUi, window, window.activeWebView()"));
    assert!(qml.contains("secondaryUi, secondaryWindow,"));
    assert!(qml.contains("secondaryWindow.activeView,"));
    assert!(qml.matches(".statusDetails(").count() >= 3);
    assert!(qml.contains("siteDoctorBadge(ui)"));
    assert!(qml.contains("FerricStatusBar {"));
    assert!(status_bar.contains("required property string accessibleDetails"));
    assert!(status_bar.contains("function activityLabel()"));
    assert!(status_bar.contains("id: statusUrl"));
    assert!(!status_bar.contains("browserUi."));
    assert!(qml.contains("FerricWindowStatusBar {"));
    assert!(window_status_bar.contains("required property string accessibleDetails"));
    assert!(window_status_bar.contains("id: modeLabel"));
    assert!(!window_status_bar.contains("browserUi."));
}

#[test]
fn site_doctor_presentation_uses_typed_properties_not_qml_json() {
    let production_source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;

    assert!(production_source.contains("publish_site_experiment_presentation"));
    assert!(production_source.contains("set_site_experiment_active"));
    assert!(production_source.contains("set_site_experiment_remaining_seconds"));
    assert!(qml.contains("site_experiment_active"));
    assert!(qml.contains("site_experiment_kind"));
    assert!(!production_source.contains("site_experiment_json"));
    assert!(!qml.contains("site_experiment_json"));
}

#[test]
fn default_chrome_is_compact_modal_and_content_first() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricTabStrip.qml"),
    ]
    .concat();
    let status_bar = include_str!("../../qml/components/FerricStatusBar.qml");
    let command_line = include_str!("../../qml/components/FerricCommandLine.qml");
    let chrome_presentation = include_str!("../../qml/scripts/ChromePresentation.js");
    assert!(qml.contains("height: window.tabPosition === \"top\" && window.tabStripVisible"));
    assert!(qml.contains("visible: window.tabStripVisible"));
    assert!(qml.contains("text: (tabIndex + 1) + \"  \""));
    assert!(qml.contains("positionViewAtIndex(activeTabIndex, ListView.Contain)"));
    assert!(status_bar.contains("text: statusBar.mode.toUpperCase()"));
    assert!(status_bar.contains("id: statusUrl"));
    assert!(command_line.contains("id: commandPrefix"));
    assert!(command_line.contains("text: \":\""));
    assert!(command_line.contains("background: Rectangle { color: \"transparent\" }"));
    assert!(qml.contains("header: ToolBar {\n        height: 0\n        visible: false"));
    assert!(qml.contains("palette.buttonText: window.primaryTextColor"));
    assert!(qml.contains("ChromePresentation.contrastReport"));
    assert!(qml.contains("function readableTextColor(candidate, background)"));
    assert!(command_line.contains("commandSurface.browserWindow.contrastText(parent.color)"));
    assert!(chrome_presentation.contains("function colorChannels(value)"));
    assert!(chrome_presentation.contains("function contrastReport(colors)"));
}

#[test]
fn presentation_helper_resources_have_no_runtime_or_page_authority() {
    let helpers = [
        include_str!("../../qml/scripts/ChromePresentation.js"),
        include_str!("../../qml/scripts/SpellcheckPresentation.js"),
    ];
    for helper in helpers {
        for forbidden in [
            "browserUi",
            "execute_command",
            "execute_ui_action",
            "runJavaScript",
            "WebEngine",
            "XMLHttpRequest",
        ] {
            assert!(
                !helper.contains(forbidden),
                "presentation helper must not gain authority: {forbidden}"
            );
        }
    }
}

#[test]
fn context_entry_restores_safe_descriptors_lazily_and_focuses_live_members() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
    let restore_source = include_str!("../browser_ui_session_restore.rs");
    assert!(restore_source.contains("fn restore_context_membership"));
    assert!(source.contains("focused_member"));
    assert!(source.contains("restored_tab_descriptors"));
    assert!(restore_source.contains("Runtime tab IDs are deliberately not reused"));
    assert!(restore_source.contains("selected_effects = Some(effects)"));
    assert!(source.contains("context_entry_force_reuse"));
    assert!(source.contains("context-window\\tfalse\\t"));
    assert!(source.contains("let live_member_window = self"));
    assert!(source.contains("window-focus\\t{member_window}"));
    assert!(qml.contains("function applyRestorePayload(payload, hasModeLine)"));
    assert!(qml.contains("tabIndex === browserUi.active_tab_index"));
    assert!(qml.contains("fields.length > 4 && fields[4].length > 0"));
    assert!(qml.contains("function focusExistingContextWindow(commandText, sourceUi)"));
    assert!(qml.contains("windowStartupContextRestore: true"));
    assert!(qml.contains("secondaryUi.set_context_entry_reuse("));
    assert!(qml.contains("browserUi.set_context_entry_reuse(true)"));
    assert!(!qml.contains("context_entry_force_reuse ="));
}

#[test]
fn popup_windows_capture_opener_context_identity() {
    let qml = QML_SOURCE;
    assert!(qml.contains("property string popupContextName"));
    assert!(qml.contains("property string popupContextLabel"));
    assert!(qml.contains("property string popupJourneyToken"));
    assert!(qml.contains("take_popup_journey_token"));
    assert!(qml.contains("popup_navigation_started"));
    assert!(qml.contains("popup_navigation_committed"));
    assert!(qml.contains("release_popup_journey_token"));
    assert!(qml.contains("popupContextName: viewUi.context_name"));
    assert!(qml.contains("property string popupProfileName"));
    assert!(qml.contains("popupWindow.popupProfileName"));
    assert!(qml.contains("popupWindow.popupEphemeralProfile"));
    assert!(qml.contains(
        "popupWindow, popupView,\n                      popupWindow.popupPrivateProfile"
    ));
    assert!(qml.contains("property var viewUi: secondaryUi"));
    assert!(qml.contains("Ferric Browser popup · context"));
    assert!(qml.contains("popupPermissionUi: viewUi"));
}

#[test]
fn journey_export_requires_preview_and_explicit_local_destination() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
    let manager = include_str!("../../qml/components/FerricLibraryManager.qml");
    let dialogs = include_str!("../../qml/components/FerricFileDialogSurfaces.qml");
    assert!(source.contains("fn journey_export_preview"));
    assert!(source.contains("memory-only"));
    assert!(source.contains("refuses to overwrite an existing file"));
    assert!(source.contains("atomic_write_private(path_ref"));
    assert!(qml.contains("journey_export_preview"));
    assert!(qml.contains("property bool journeyExportPreviewVisible"));
    assert!(qml.contains("property bool journeyExportAwaiting"));
    assert!(source.contains("journey_export_preview_text"));
    assert!(qml.contains("fileDialogSurfaces.openJourneyExport()"));
    assert!(dialogs.contains("title: \"Choose journey export destination\""));
    assert!(manager.contains("Export journey records"));
}

#[test]
fn diagnostics_export_requires_explicit_local_save_and_never_overwrites() {
    let source = ADAPTER_SOURCE;
    let site = include_str!("../browser_ui_site.rs");
    let diagnostics = include_str!("../diagnostics.rs");
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricDiagnostics.qml"),
    ]
    .concat();
    let dialogs = include_str!("../../qml/components/FerricFileDialogSurfaces.qml");
    assert!(source.contains("fn export_diagnostics"));
    assert!(source.contains("fn authorize_diagnostics_export"));
    assert!(source.contains("diagnostics_snapshot_payload"));
    assert!(source.contains("diagnostics_export_payload\n            .take()"));
    assert!(site.contains("this.diagnostics_export_payload = None;"));
    assert!(site.contains("reviewed_diagnostics_match("));
    assert!(
        site.contains("diagnostics_snapshot_payload = reviewable.then(|| serialized.clone())")
    );
    assert!(!site.contains("MAX_DIAGNOSTICS_TEXT"));
    assert!(!site.contains("serialized.truncate("));
    assert!(diagnostics.contains("MAX_REVIEWED_DIAGNOSTICS_BYTES"));
    assert!(source.contains("MAX_DIAGNOSTICS_EXPORT_BYTES"));
    assert!(source.contains("Diagnostics export requires an absolute local file path"));
    assert!(source.contains("Diagnostics export refuses to overwrite an existing file"));
    assert!(source.contains("atomic_write_private(path_ref, payload.as_bytes())"));
    assert!(site.contains("Diagnostics export failed"));
    assert!(!site.contains("Diagnostics export failed: {error}"));
    assert!(site.contains("Site report unavailable"));
    assert!(!site.contains("Site report unavailable: {error}"));
    assert!(site.contains("Site-data clear failed"));
    assert!(!site.contains("Site-data clear failed: {error}"));
    assert!(qml.contains("fileDialogSurfaces.openDiagnosticsExport()"));
    assert!(dialogs.contains("title: \"Choose diagnostics export destination\""));
    assert!(qml.contains("Save diagnostics preview"));
    assert!(qml.contains("browserUi.authorize_diagnostics_export(window.diagnosticsText)"));
    assert!(!qml.contains("onSaveRequested: {\n            window.refreshDiagnostics()"));
    assert!(qml.contains("browserUi.export_diagnostics(path)"));
}

#[test]
fn required_desktop_portals_gate_file_selection_asynchronously() {
    let source = ADAPTER_SOURCE;
    let portals = include_str!("../desktop_portals.rs");
    let qml = QML_SOURCE;
    assert!(source.contains("fn portal_capability_status"));
    assert!(source.contains("portal_capabilities"));
    assert!(portals.contains("struct PortalCapabilities"));
    assert!(source.contains("portal_probe_worker"));
    assert!(qml.contains("ui.desktop_portal_mode"));
    assert!(!qml.contains("config.desktop.portals"));
    assert!(qml.contains("probe_desktop_portals()"));
    assert!(qml.contains("browserUi.portal_capability_status(capability)"));
    assert!(!qml.contains("desktop_portal_status"));
    assert!(qml.contains("desktopPortalCapabilityStatus(requestUi, \"file_chooser\")"));
    assert!(qml.contains("pendingFileDialogWaitingForPortal"));
    assert!(qml.contains("pendingFileDialogWaitingForPortal = true"));
    assert!(qml.contains("pendingFileDialogPortalDeadlineMs"));
    assert!(qml.contains("Desktop portal check timed out; file selection cancelled"));
    assert!(qml.contains("pendingDesktopMediaPortalDeadlineMs"));
    assert!(qml.contains("popupFilePortalTimer"));
    assert!(qml.contains("function maybeOpenPendingFileDialog()"));
    assert!(qml.contains("popupWindow.maybeOpenPendingFileDialog()"));
    assert!(qml.contains("secondaryWindow.maybeOpenPendingFileDialog()"));
    assert!(qml.contains("secondaryWindow.pendingFileDialogWaitingForPortal = true"));
    assert!(qml.contains("secondaryWindow.pendingFileDialogPortalDeadlineMs"));
    assert!(qml.contains("Required desktop portal unavailable; file selection cancelled"));
    assert!(qml.contains("window.openPendingEngineFileDialog()"));
    assert!(qml.contains("desktopPortalCapabilityStatus(ui, \"screen_cast\")"));
    assert!(qml.contains("Required ScreenCast portal unavailable; screen sharing cancelled"));
    assert!(qml.contains("maybeOpenPendingDesktopMediaRequest"));
    assert!(qml.contains("desktopPortalCapabilityStatus(targetUi, \"open_uri\")"));
    assert!(qml.contains("pendingExternalUris"));
    assert!(qml.contains("processPendingExternalUris"));
    assert!(qml.contains("Desktop portal check timed out; external action cancelled"));
    assert!(qml.contains("Required OpenURI portal unavailable; external action cancelled"));
    assert!(qml.contains("function openExternalUri(ui, uri)"));
    assert!(qml.contains("desktopPortalCapabilityStatus(ui, \"notifications\")"));
    assert!(qml.contains("pendingPortalNotifications"));
    assert!(qml.contains("deferNotificationUntilPortal"));
    assert!(qml.contains("processPendingPortalNotifications"));
    assert!(qml.contains("closeWebNotificationsForOrigin"));
    assert!(qml.contains("pendingOrigin"));
    assert!(qml.contains("Desktop portal check timed out; notification cancelled"));
    assert!(qml.contains("Required Notification portal unavailable; notification cancelled"));
}

#[test]
fn security_deny_hosts_are_separate_from_adblock_bypasses() {
    let qml = QML_SOURCE;
    let interceptor = include_str!("../request_interceptor.cpp");
    assert!(qml.contains("securityDenyHosts"));
    assert!(qml.contains("security_deny_hosts"));
    assert!(interceptor.contains("securityDenyHosts"));
    assert!(interceptor.contains("security deny rule"));
    assert!(ADAPTER_SOURCE.contains("blocking.security-deny-host"));
    assert!(ADAPTER_SOURCE.contains("security_deny_rules"));
    let config = serde_json::to_value(Config::default()).expect("default config serializes");
    assert_eq!(
        config["blocking"]["security_deny_hosts"],
        serde_json::json!([])
    );
}

#[test]
fn request_interceptor_uses_bounded_engine_context_without_ui_fallback() {
    let interceptor = include_str!("../request_interceptor.cpp");
    let header = include_str!("../request_interceptor.h");
    let qml = QML_SOURCE;
    assert!(header.contains("std::shared_ptr<const PolicySnapshot> policy_"));
    assert!(interceptor.contains("info.requestUrl()"));
    assert!(interceptor.contains("info.firstPartyUrl()"));
    assert!(interceptor.contains("info.initiator()"));
    assert!(interceptor.contains("info.resourceType()"));
    assert!(interceptor.contains("info.navigationType()"));
    assert!(interceptor.contains("NavigationTypeRedirect"));
    assert!(interceptor.contains("contextKnown"));
    assert!(interceptor.contains("blockedRuleHosts"));
    assert!(interceptor.contains("blockedRuleListIds"));
    assert!(interceptor.contains("exceptionRuleHosts"));
    assert!(interceptor.contains("exceptionRuleListIds"));
    assert!(interceptor.contains("ferric_browser_adblock_check"));
    assert!(interceptor.contains("adblockResourceType"));
    assert!(header.contains("adblockEngineHandle"));
    assert!(qml.contains("blocking_adblock_handle"));
    assert!(interceptor.contains("list_id"));
    assert!(interceptor.contains("exception_list_id"));
    assert!(qml.contains("blocking_rule_hosts"));
    assert!(qml.contains("blocking_rule_list_ids"));
    assert!(qml.contains("blocking_exception_rule_hosts"));
    assert!(qml.contains("blocking_exception_rule_list_ids"));
    assert!(interceptor.contains("normalized.size() > 253"));
    assert!(!interceptor.contains("activeWebView"));
    assert!(interceptor.contains("scheduleEvidenceChanged()"));
    assert!(interceptor.contains("evidenceSignalPending_.exchange"));
    assert!(interceptor.contains("QMetaObject::invokeMethod"));
    assert!(interceptor.contains("Qt::QueuedConnection"));
}

#[test]
fn renderer_termination_is_reduced_before_qml_recovery_surface() {
    let source = include_str!("../renderer_lifecycle.rs");
    let qml = QML_SOURCE;
    assert!(source.contains("fn note_renderer_process_terminated"));
    assert!(source.contains("fn prepare_renderer_recovery"));
    assert!(source.contains("Event::RendererTerminated { target }"));
    assert!(qml.contains("ui.note_renderer_process_terminated(tabIndex)"));
    assert!(qml.contains("ui.prepare_renderer_recovery"));
    assert!(qml.contains("onRenderProcessTerminated"));
}

#[test]
fn blocking_status_counts_security_deny_rules_without_exposing_hosts() {
    let config = serde_json::json!({
        "blocking": {
            "security_deny_hosts": ["one.example", "*.two.example"]
        }
    });
    let serialized = serde_json::to_string(&config).expect("config serializes");
    assert_eq!(security_deny_rule_count(&serialized), 2);
    assert_eq!(security_deny_rule_count("{\"blocking\":{}}"), 0);
    assert_eq!(security_deny_rule_count("not json"), 0);
}

#[test]
fn background_link_navigation_captures_the_source_journey_node() {
    let source = ADAPTER_SOURCE;
    assert!(source.contains("journey_parent: Option<JourneyNodeId>"));
    assert!(source.contains("let journey_parent = source_tab"));
    assert!(source.contains("self.as_mut().mark_journey_parent(&effects, parent)"));
}

#[test]
fn ephemeral_profile_uses_off_the_record_memory_only_setup() {
    let qml = QML_SOURCE;
    assert!(qml.contains("property bool ephemeralProfile"));
    assert!(qml.contains("WebEngineProfilePrototype {"));
    assert!(qml.contains("storageName: profileHost.browserWindow.temporaryProfile"));
    assert!(qml.contains("persistentStoragePath: profileHost.browserWindow.temporaryProfile"));
    assert!(qml.contains("cachePath: profileHost.browserWindow.temporaryProfile"));
    assert!(
        qml.contains("window.browserProfile = window.browserProfilePrototypeObject.instance()")
    );
    assert!(qml.contains("title: window.ephemeralProfile"));
    assert!(qml.contains("? \"Ferric Browser · \" + window.profileLabel"));
    assert!(qml.contains("window.temporaryProfile && !window.ephemeralProfile"));
    assert!(qml.contains("window.ephemeralProfile,"));
    let source = ADAPTER_SOURCE;
    assert!(source.contains("PrivacyKind::Ephemeral"));
    assert!(source.contains("active_profile_is_transient"));
    assert!(source.contains("release_transient_resources"));
    assert!(source.contains("pending_profile_configuration = None"));
    assert!(source.contains("request_profile_open(ProfileOpenRequest"));
    assert!(!source.contains("ProfileSetupWorker"));
    assert!(source.contains("network_policy_worker = None"));
    assert!(source.contains("set_blocking_rule_hosts(QStringList::default())"));
    assert!(source.contains("set_blocking_rule_list_ids(QStringList::default())"));
    assert!(source.contains("set_blocking_exception_rule_hosts(QStringList::default())"));
    assert!(source.contains("set_blocking_exception_rule_list_ids(QStringList::default())"));
    assert!(source.contains("Durable profile resources remain owned"));
    assert!(source.contains("this.session_permissions.clear()"));
    assert!(!source.contains("this.profile_lock ="));
    assert!(source.contains("Session saving is unavailable for this profile"));
    assert!(source.contains("cannot use durable contexts"));
    assert!(source.contains("private or ephemeral switcher activation is disabled"));
    assert!(source.contains("Created a fresh ephemeral profile window; it is not durable"));
}

#[test]
fn ephemeral_hint_target_creates_a_typed_new_window_action() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
    assert!(source.contains("target == \"ephemeral\""));
    assert!(source.contains("ephemeral-window\\t{}"));
    assert!(qml.contains("function openEphemeralWindow(url, requestedToken)"));
    assert!(qml.contains("function ephemeralProfileForToken(token)"));
    assert!(qml.contains("property var ephemeralProfileOwners"));
    assert!(qml.contains("function retainEphemeralProfileOwner(profile, token)"));
    assert!(qml.contains("function releaseEphemeralProfileOwner(profile, token)"));
    assert!(qml.contains("Ephemeral profile owner token is already bound"));
    assert!(qml.contains("ephemeralInvocationToken"));
    assert!(qml.contains("windowSharedProfile: sharedProfile"));
    assert!(qml.contains("property var viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("action.indexOf(\"ephemeral-window\\t\") === 0"));
    assert!(qml.contains("window.registerBrowserWindow("));
    assert!(qml.contains("windowEphemeralProfile: true"));
    assert!(qml.contains("result.action === \"ephemeral\""));
    assert!(qml.contains("onClosing: function(close)"));
    assert!(qml.contains("windowShutdownPromptVisible"));
    assert!(qml.contains("secondaryUi.request_shutdown()"));
    assert!(qml.contains("hasActiveShutdownRequestsFor"));
    assert!(qml.contains("windowSharedProfile || secondaryProfile"));
}

#[test]
fn shutdown_waits_for_and_cancels_rust_owned_page_capture() {
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let callbacks = include_str!("../../qml/scripts/NativePageCaptureCallbacks.js");
    let shutdown_requests = presentation
        .split("function hasActiveShutdownRequestsFor(ui, hostWindow)")
        .nth(1)
        .expect("shutdown request accounting")
        .split("function cancelShutdownRequestsFor")
        .next()
        .expect("shutdown request accounting boundary");
    assert!(shutdown_requests.contains("ui.has_native_page_capture_request()"));
    assert!(shutdown_requests.contains("hostWindow === window && ui === browserUi"));

    let cancel_and_quit = presentation
        .split("function cancelDownloadsAndQuit()")
        .nth(1)
        .expect("cancel-and-quit path")
        .split("function refreshLibraryManager()")
        .next()
        .expect("cancel-and-quit boundary");
    assert!(cancel_and_quit.contains("window.cancelNativePageCaptureForShutdown()"));

    let wrapper = requests
        .split("function cancelNativePageCaptureForShutdown()")
        .nth(1)
        .expect("native capture shutdown wrapper")
        .split("function retireSupersededNativePageCapture")
        .next()
        .expect("native capture shutdown wrapper boundary");
    assert!(wrapper.contains("cancelPendingCaptureForShutdown"));
    assert!(callbacks.contains("browserUi.cancel_pending_native_page_capture_for_shutdown()"));
}

#[test]
fn shutdown_waits_for_cancelled_versioning_object_worker_jobs() {
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let surface = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let bridge = include_str!("../lib.rs");
    let capture = include_str!("../browser_ui_versioning/capture.rs");

    assert!(bridge.contains("fn has_pending_versioning_object_work"));
    assert!(capture.contains("BrowserApplication::has_pending_versioning_object_work"));

    for (function, boundary) in [
        ("function continueQuitRequest()", "function finalizeQuit()"),
        (
            "function finalizeQuit()",
            "function cancelDownloadsAndQuit()",
        ),
    ] {
        let shutdown = presentation
            .split(function)
            .nth(1)
            .expect("shutdown continuation exists")
            .split(boundary)
            .next()
            .expect("shutdown continuation boundary");
        assert!(shutdown.contains("has_pending_versioning_object_work()"));
        assert!(shutdown.contains("shutdownWaitingForDownloadFinishes = true"));
        assert!(shutdown.contains("scheduleRuntimeWork(50)"));
    }

    let runtime_work = surface
        .split("function processRuntimeWork()")
        .nth(1)
        .expect("runtime work pump exists")
        .split("function startNativePageCaptureSave")
        .next()
        .expect("runtime work pump boundary");
    assert!(runtime_work.contains("has_pending_versioning_object_work()"));
    assert!(runtime_work.contains("window.continueQuitRequest()"));
    assert!(runtime_work.contains("window.scheduleRuntimeWork(50)"));

    for (function, boundary) in [
        ("function continueQuitRequest()", "function finalizeQuit()"),
        (
            "function finalizeQuit()",
            "function cancelDownloadsAndQuit()",
        ),
    ] {
        let shutdown = secondary
            .split(function)
            .nth(1)
            .expect("secondary shutdown continuation exists")
            .split(boundary)
            .next()
            .expect("secondary shutdown continuation boundary");
        assert!(shutdown.contains("has_pending_versioning_object_work()"));
        assert!(shutdown.contains("scheduleRuntimeWork(50)"));
    }

    let secondary_runtime_work = secondary
        .split("function processRuntimeWork()")
        .nth(1)
        .expect("secondary runtime work pump exists")
        .split("Timer {")
        .next()
        .expect("secondary runtime work pump boundary");
    assert!(secondary_runtime_work.contains("has_pending_versioning_object_work()"));
    assert!(secondary_runtime_work.contains("secondaryWindow.continueQuitRequest()"));
}

#[test]
fn foreground_hint_tab_materializes_the_view_before_navigation() {
    let qml = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let foreground = qml
        .split("result.action === \"tab\"")
        .nth(1)
        .expect("foreground hint-tab result branch")
        .split("result.action === \"yank\"")
        .next()
        .expect("foreground hint-tab branch boundary");

    let sync = foreground
        .find("window.syncTabModel()")
        .expect("foreground hint tab must materialize its QML view");
    let navigate = foreground
        .find("window.executePendingEngineAction()")
        .expect("foreground hint tab must apply its queued navigation");
    assert!(
        sync < navigate,
        "the new tab view must exist before navigation"
    );
}

#[test]
fn tls_errors_are_blocked_by_default_and_only_allow_scoped_confirmation() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricCertificatePrompts.qml"),
    ]
    .concat();
    assert!(qml.contains("onCertificateError"));
    assert!(qml.contains("!error.overridable"));
    assert!(qml.contains("!error.isMainFrame"));
    assert!(qml.contains("ui, error, \"client-certificate\", \"rejectCertificate\", []"));
    assert!(qml.contains("ui, error, \"client-certificate\", \"acceptCertificate\", []"));
    assert!(qml.contains("Accept once"));
    assert!(qml.contains("for this request only"));
    assert!(qml.contains("googleCertificateHost"));
    assert!(!qml.contains("ignoreCertificateErrors"));
    assert!(!qml.contains("setIgnoreCertificateErrors"));
}

#[test]
fn webauthn_uses_engine_state_and_clears_pin_input() {
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricWebAuthPrompt.qml"),
    ]
    .concat();
    assert!(qml.contains("onWebAuthUxRequested"));
    assert!(qml.contains("WebEngineWebAuthUxRequest.SelectAccount"));
    assert!(qml.contains("WebEngineWebAuthUxRequest.CollectPin"));
    assert!(qml.contains("request.setSelectedAccount"));
    assert!(qml.contains("request.setPin(pin)"));
    assert!(qml.contains("webAuthPrompt.pin = \"\""));
    assert!(qml.contains("property alias pin: webAuthPinField.text"));
    assert!(qml.contains("ui, request, \"webauth\", \"cancel\", []"));
    assert!(qml.contains("request.retry()"));
    assert!(qml.contains("request.relyingPartyId"));
    assert!(!qml.contains("pinRequest.password"));
    assert!(!qml.contains("console.log(pin"));
}

#[test]
fn context_menus_use_engine_actions_and_bound_spellcheck_data() {
    let source = ADAPTER_SOURCE;
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricContextMenu.qml"),
        include_str!("../../qml/components/FerricHintOverlay.qml"),
        include_str!("../../qml/scripts/SpellcheckPresentation.js"),
    ]
    .concat();
    assert!(qml.contains("onContextMenuRequested"));
    assert!(qml.contains("request.accepted = true"));
    assert!(qml.contains("appendUserscriptActions"));
    assert!(qml.contains("ui.select_userscript_action_subject(subject)"));
    assert!(qml.contains("userscript_action_availability"));
    assert!(qml.contains("function showHintActions(label)"));
    assert!(qml.contains("hint-userscript-action"));
    assert!(qml.contains("select_hint_action"));
    assert!(qml.contains("Qt.RightButton"));
    assert!(source.contains("execute_registered_userscript_action_for_hint"));
    assert!(source.contains("if hint_activation"));
    assert!(qml.contains("view.triggerWebAction(webAction)"));
    assert!(qml.contains("replaceMisspelledWord"));
    assert!(qml.contains("spellCheckerSuggestions"));
    assert!(qml.contains("Download link"));
    assert!(qml.contains("browser.link.open"));
    assert!(qml.contains("Open link in new tab"));
    assert!(qml.contains("Open link in background tab"));
    assert!(qml.contains("Open link in new window"));
    assert!(qml.contains("contextMenu.openAt(menuHost, menuX, menuY)"));
    assert!(qml.contains("window.syncTabModel()"));
    assert!(qml.contains("browser.link.download"));
    assert!(qml.contains("browser.download.pause"));
    assert!(qml.contains("browser.download.resume"));
    assert!(qml.contains("browser.download.cancel"));
    assert!(qml.contains("browser.download.retry"));
    assert!(qml.contains("browser.selection.copy"));
    assert!(qml.contains("browser.selection.search"));
    assert!(qml.contains("browser.tab.pin"));
    assert!(qml.contains("browser.tab.mute"));
    assert!(qml.contains("browser.tab.undo"));
    assert!(qml.contains(":tab-clone"));
    assert!(qml.contains("function scrollScript"));
    assert!(qml.contains("function searchFindFlags(query, caseMode, backward)"));
    assert!(qml.contains("var characters = Array.from(text)"));
    assert!(qml.contains("character.toUpperCase()"));
    assert!(qml.contains("scroll-page\\t"));
    assert!(qml.contains("scroll-to\\t"));
    assert!(qml.contains("tab_id_for_index"));
    assert!(qml.contains("execute_ui_action"));
    assert!(qml.contains("Inspect element"));
    assert!(qml.contains("spellCheckLanguages"));
    assert!(qml.contains("function languageIsValid(language)"));
    assert!(qml.contains("i-klingon"));
    assert!(qml.contains("zh-min-nan"));
    assert!(qml.contains("(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|x(?:-[A-Za-z0-9]{1,8})+)"));
    assert!(qml.contains("function safeContextUrl(value)"));
    assert!(qml.contains("decodeURIComponent(key)"));
    assert!(qml.contains("private[_-]?key"));
    assert!(qml.contains("client[_-]?secret"));
    assert!(!qml.contains("Qt.openUrlExternally(link"));
}

#[test]
fn browser_owned_script_resource_has_a_versioned_bounded_contract() {
    let qml = QML_SOURCE;
    let script = include_str!("../../qml/scripts/BrowserScripts.js");
    assert!(qml.contains("import \"../scripts/BrowserScripts.js\" as BrowserScripts"));
    assert!(qml.contains("return BrowserScripts.scroll(kind, direction, half, count)"));
    assert!(qml.contains("BrowserScripts.scrollPosition()"));
    assert!(qml.contains("BrowserScripts.restoreScrollPosition(x, y)"));
    assert!(qml.contains("return BrowserScripts.selection()"));
    assert!(qml.contains("return BrowserScripts.editor()"));
    assert!(qml.contains("return BrowserScripts.editorApply(original, updated)"));
    assert!(qml.contains("return BrowserScripts.caret(operation, selecting)"));
    assert!(qml.contains("return BrowserScripts.downloadLink(url)"));
    assert!(qml.contains("return BrowserScripts.clearSiteData()"));
    assert!(qml.contains("return BrowserScripts.focusProbe()"));
    assert!(qml.contains("return BrowserScripts.focusObserverSource()"));
    assert!(qml.contains("return BrowserScripts.shutdownPageProbe()"));
    assert!(qml.contains("return BrowserScripts.hintCollector(family)"));
    assert!(qml.contains("return BrowserScripts.hintDirtyRevision()"));
    assert!(qml.contains("return BrowserScripts.hintStopTracking()"));
    assert!(qml.contains("return BrowserScripts.hintFresh(candidate)"));
    assert!(qml.contains("return BrowserScripts.hintFocus(elementId)"));
    assert!(qml.contains("return BrowserScripts.hintClick(elementId)"));
    assert!(qml.contains("BrowserScripts.pageUserscriptRun(scriptSource)"));
    assert!(qml.contains("BrowserScripts.pageUserscriptInstall(script.source)"));
    assert!(!qml.contains("function hintSelector(linksOnly)"));
    assert!(script.contains("var VERSION = \"11\""));
    assert!(qml.contains("function scrollTargetPolicyScript(policy)"));
    assert!(qml.contains("return BrowserScripts.scrollTargetPolicy(policy)"));
    assert!(qml.contains("function scrollTargetStatusScript()"));
    assert!(qml.contains("return BrowserScripts.scrollTargetStatus()"));
    assert!(qml.contains("function hintSetScrollTargetScript(elementId, framePath)"));
    assert!(qml.contains("return BrowserScripts.hintSetScrollTarget(elementId, framePath)"));
    assert!(qml.contains("function applyScrollTargetAction(ui, view, action, feedbackHost)"));
    assert!(qml.contains("Scroll target unavailable: no active page"));
    assert!(qml.contains("Scroll target command failed"));
    assert!(qml.contains("? \"automatic\" : \"document\""));
    assert!(qml.contains("? \"selected region\""));
    assert!(qml.contains("feedbackHost.showCommandNotice(message, isError)"));
    assert!(qml.contains("window.showCommandNotice(browserUi.status_text, !targetSelected)"));
    assert!(qml.contains("action.indexOf(\"scroll-target\\t\") === 0"));
    assert!(qml.contains("id: secondaryUi"));
    assert!(qml.contains("hint_chrome_available: false"));
    assert!(script.contains("window.__ferric_browserScrollTargetState"));
    assert!(!script.contains("window.__ferric_browserScrollTarget="));
    assert!(!script.contains("window.__ferric_browserScrollTarget&&"));
    assert!(script.contains("new WeakRef(element)"));
    assert!(script.contains("new WeakRef(element.ownerDocument)"));
    assert!(script.contains("deref.call(state.elementRef)"));
    assert!(script.contains("deref.call(state.ownerDocumentRef)"));
    assert!(script.contains("style.overflowX"));
    assert!(script.contains("style.overflowY"));
    assert!(script.contains("'overlay'"));
    assert!(script.contains("function ftMeaningfullyVisible(element)"));
    assert!(script.contains("frame.contentWindow!==childView"));
    assert!(script.contains("kind=family==='scrollables'?'scrollable':'aria'"));
    assert!(script.contains("kind=candidateKind==='scrollable'?'scrollable':'aria'"));
    assert!(
        script.contains("if(s.elements&&typeof s.elements.clear==='function')s.elements.clear()")
    );
    let target_start = script
        .find("function hintSetScrollTarget(elementId, framePath)")
        .expect("target assignment builder");
    let target_end = script[target_start..]
        .find("\n}")
        .map(|offset| target_start + offset)
        .expect("target assignment builder end");
    assert!(!script[target_start..target_end].contains(".focus("));
    assert!(script[target_start..target_end].contains("ftSetPolicy('element',element)"));
    assert!(script.contains("function pageUserscriptRun(source)"));
    assert!(script.contains("function pageUserscriptInstall(source)"));
    assert!(!qml.contains("var source = \"(function(){try{\" + scriptSource"));
    assert!(!qml.contains("installedScript.sourceCode = \"(function(){try{\" + script.source"));
    assert!(script.contains("function boundedCount(value)"));
    assert!(script.contains("Math.max(1, Math.min(9999"));
    assert!(script.contains("password fields are not copied"));
    assert!(script.contains("password fields are not editable externally"));
    assert!(script.contains("field changed while editor was open"));
    assert!(script.contains("invalid caret movement"));
    assert!(script.contains("a.rel='noreferrer'"));
    assert!(script.contains("service_workers:'unavailable'"));
    assert!(script.contains("window.__ferric_browserFocusState"));
    assert!(script.contains("elements.length > 128"));
    assert!(script.contains("function hintCollector(family)"));
    assert!(script.contains("function hintFresh(candidate)"));
    assert!(script.contains("function hintFocus(elementId)"));
    assert!(script.contains("function hintClick(elementId)"));
    assert!(script.contains("function focusObserverSource()"));
    assert!(script.contains("function formStateProbe()"));
    assert!(script.contains("function siteDataClearResult()"));
    assert!(script.contains("function scrollPosition()"));
    assert!(script.contains("function restoreScrollPosition(x, y)"));
    assert!(script.contains("function cosmeticFilter(css)"));
    assert!(script.contains("out.length>=5000"));
    assert!(script.contains("depth>8"));
    assert!(script.contains("r.bottom<=0||r.right<=0||r.top>=vh||r.left>=vw"));
    assert!(script.contains("x>=window.innerWidth||y>=window.innerHeight"));
    assert!(script.contains("r.top<window.innerHeight"));
    assert!(script.contains("state.resize.unobserve"));
    assert!(script.contains("function intersect(a,b)"));
    assert!(script.contains("trim().slice(0,512)"));
    assert!(script.contains("kind==='link'&&href===null"));
    assert!(script.contains("function linkedSurface(el,r)"));
    assert!(
        script.contains("family==='all'&&(kind==='image'||kind==='media')&&linkedSurface(el,r)")
    );
    assert!(script.contains("function hasPreferredLink(el,r)"));
    assert!(script.contains("String(current.href||current.getAttribute('href')||'')===href"));
}

#[test]
fn enhanced_hint_presentation_keeps_keyboard_policy_out_of_qml() {
    let qml = QML_SOURCE;
    let base = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let overlay = include_str!("../../qml/components/FerricHintOverlay.qml");
    let menu = include_str!("../../qml/components/FerricContextMenu.qml");

    assert!(qml.contains("update_hint_interaction(action, text || \"\")"));
    assert!(qml.contains("window.applyHintInteraction(\"text-mode\", \"\")"));
    assert!(qml.contains("window.applyHintInteraction(\"rotate\", \"\")"));
    assert!(qml.contains("return BrowserScripts.hintDirtyRevision()"));
    assert!(qml.contains("browserUi.mode !== \"hint\" || window.hintTrackingView !== view"));
    assert!(qml.contains("window.hintCollectionEpoch !== collectionEpoch"));
    assert!(qml.contains("function stopHintTracking()"));
    assert!(qml.contains("window.showCommandNotice(hintFailure, true)"));
    assert!(qml.contains("window.hintViewport = ({"));
    assert!(base.contains("property var hintTrackingView: null"));
    assert!(base.contains("property var hintViewport: ({ width: 0, height: 0 })"));
    assert!(base.contains("property int hintCollectionEpoch: 0"));
    assert!(requests.contains("function onHint_visibleChanged()"));
    assert!(requests.contains("window.activeWebView() !== window.hintTrackingView"));
    assert!(overlay.contains("unmatchedPolicy !== \"hide\""));
    assert!(overlay.contains("unmatchedPolicy === \"show\""));
    assert!(overlay.contains("<u>"));
    assert!(overlay.contains("Active hint target"));
    assert!(overlay.contains("function coordinateScale(renderedSize, sourceSize)"));
    assert!(overlay.contains("var targetX = source.x * scaleX"));
    assert!(overlay.contains("var options = [[targetX, targetY]"));
    assert!(menu.contains("sequence: \"Tab\""));
    assert!(menu.contains("hint.clean-yank"));
}

#[test]
fn page_backed_hints_close_after_the_renderer_consumes_the_retained_record() {
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");

    for (action, next_action) in [
        ("focus", "scroll-target"),
        ("scroll-target", "click"),
        ("click", "rejection"),
    ] {
        let start = presentation
            .find(&format!("result.action === \"{action}\""))
            .unwrap_or_else(|| panic!("missing {action} hint branch"));
        let end = if next_action == "rejection" {
            presentation[start..]
                .find("} else {")
                .map(|offset| start + offset)
                .expect("missing final hint rejection branch")
        } else {
            presentation[start..]
                .find(&format!("result.action === \"{next_action}\""))
                .map(|offset| start + offset)
                .unwrap_or_else(|| panic!("missing {next_action} hint branch"))
        };
        let branch = &presentation[start..end];
        let launch = branch
            .find("window.runBrowserScript(")
            .unwrap_or_else(|| panic!("missing {action} renderer launch"));
        let close = branch
            .find("window.closeHints()")
            .unwrap_or_else(|| panic!("missing {action} callback cleanup"));
        assert!(
            launch < close,
            "{action} must not clear its retained page record before launching the renderer action"
        );
    }
}

#[test]
fn hint_results_are_not_discarded_based_on_renderer_callback_latency() {
    let qml = QML_SOURCE;

    // QtWebEngine invokes runJavaScript callbacks only after the renderer has
    // completed the work. Rejecting a completed, valid result because it took
    // more than an arbitrary wall-clock threshold makes hints fail on large or
    // busy pages without bounding the renderer work itself.
    assert!(!qml.contains("Hint collection timed out"));
    assert!(!qml.contains("Hint target validation timed out"));
}

#[test]
fn extracted_runtime_scripts_are_registered_beneath_the_qml_resource_root() {
    // Characterization: the extracted components resolve ../scripts relative to
    // qml/components. Registering scripts at the module prefix (the previous
    // behavior) therefore made the packaged browser fail during QML startup.
    let build_script = include_str!("../../build.rs");
    assert!(build_script.contains("format!(\"qml/{relative_alias}\")"));
}

#[test]
fn screen_capture_keeps_scoped_indicator_and_reload_stop_boundary() {
    let qml = QML_SOURCE;
    let indicator = include_str!("../../qml/components/FerricCaptureIndicator.qml");
    let source = ADAPTER_SOURCE;
    assert!(qml.contains("onDesktopMediaRequested"));
    assert!(qml.contains("\"desktop-media\", \"selectScreen\""));
    assert!(qml.contains("\"desktop-media\", \"selectWindow\""));
    assert!(qml.contains("function recordCaptureSession"));
    assert!(qml.contains("browser-owned capture ledger"));
    assert!(qml.contains("activeCapture.status"));
    assert!(qml.contains("FerricCaptureIndicator"));
    assert!(indicator.contains("Capture indicator"));
    assert!(indicator.contains("browserWindow.captureSessions"));
    assert!(indicator.contains("indicator.stopRequested(modelData.id)"));
    assert!(qml.contains("function stopCaptureSession"));
    assert!(qml.contains("target.view.reload()"));
    assert!(qml.contains("clearCaptureSessionsForHost"));
    assert!(!qml.contains("PipeWire"));
    assert!(source.contains("if permission == \"screen-capture\""));
    assert!(source.contains("Screen-capture consent reset; active captures will be stopped"));
}

#[test]
fn site_ledger_exposes_bounded_userscript_metadata_without_sources() {
    let source = ADAPTER_SOURCE;
    assert!(source.contains("profile-userscript-manifests"));
    assert!(source.contains("matching_active_site"));
}

#[test]
fn site_doctor_userscript_inventory_discards_dynamic_failure_reason() {
    let source = include_str!("../browser_ui_query_site.rs");
    assert!(source.contains("Err(reason) => unavailable_userscript_inventory(&reason)"));
    assert!(!source.contains("bounded_navigation_failure_detail(&reason)"));
    assert!(!source.contains("\"reason\": reason"));
    assert!(source.contains("sanitize_untrusted_title(&tab.title)"));
    assert!(!source.contains("tab.title.replace(['\\n', '\\r'], \" \")"));
}

#[test]
fn site_data_clear_is_bound_to_its_originating_view() {
    let qml = QML_SOURCE;
    assert!(qml.contains("property var siteDataClearView"));
    assert!(qml.contains("var view = window.siteDataClearView"));
    assert!(qml.contains("clearSiteDataClearForView"));
    assert!(qml.contains("site-data clear cancelled by navigation"));
    assert!(qml.contains("window.siteDataClearUi = browserUi"));
}

#[test]
fn notifications_require_consent_and_push_is_explicitly_opt_in() {
    let qml = QML_SOURCE;
    let runtime_base = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    assert!(qml.contains("onPresentNotification"));
    assert!(qml.contains("notification.show()"));
    assert!(qml.contains("notification.close()"));
    assert!(qml.contains("permission_decision(origin, \"notifications\")"));
    assert!(qml.contains("activeWebNotifications"));
    assert!(qml.contains("notification.closed.connect"));
    assert!(qml.contains("previous.close()"));
    assert!(qml.contains("function notificationProfileScope(ui, privateProfile)"));
    assert!(qml.contains("window.notificationProfileScope(ui, privateProfile)"));
    assert!(qml.contains("function closeWebNotificationsForOrigin(origin)"));
    assert!(qml.contains("permissionParts[2] === \"notifications\""));
    assert!(qml.contains("NotificationPresenter"));
    assert!(qml.contains("notificationPresenter.present(notification, profileScope, origin)"));
    assert!(qml.contains("onNotificationUnavailable"));
    assert!(qml.contains("property var notificationOwners"));
    assert!(qml.contains("function notificationOwnerFor(notification)"));
    assert!(qml.contains("window.notificationOwnerFor(notification)"));
    assert!(qml.contains("rememberNotificationOwner(notification, ui)"));
    assert!(qml.contains("function activeWebNotification(notification)"));
    assert!(qml.contains("window.activeWebNotification(notification)"));
    assert!(qml.contains("Desktop notification service unavailable; using Qt fallback"));
    assert!(qml.contains("focusNotificationOrigin(profileScope, origin)"));
    assert!(qml.contains("closeWebNotifications()"));
    assert!(qml.contains("ui.feature_push_service_enabled"));
    assert!(qml.contains("if (privateProfile)"));
    assert!(!qml.contains("Qt.openUrlExternally(notification"));
    assert!(runtime_base.contains(
        "entry.ui.status_text = \"Desktop notification permission revoked\""
    ));
    assert!(runtime_base.contains("ui.status_text = \"Desktop notification blocked\""));
    assert!(runtime_base.contains("ui.status_text = \"Desktop notification shown\""));
    assert!(!runtime_base.contains("Desktop notification revoked for \" + pendingOrigin"));
    assert!(!runtime_base.contains("Desktop notification blocked for \" + origin"));
    assert!(!runtime_base.contains("Desktop notification shown for \" + origin"));
    assert!(!runtime_base.contains("Private desktop notification shown for \" + origin"));
}

#[test]
fn secondary_window_unavailable_action_status_does_not_echo_action_id() {
    let browser_window = include_str!("../../qml/components/FerricBrowserWindow.qml");
    assert!(browser_window.contains(
        "secondaryUi.status_text = \"Browser action is unavailable in this window\""
    ));
    assert!(!browser_window.contains("Browser action is unavailable in this window: "));
}

#[test]
fn permission_revocation_reloads_all_registered_matching_views() {
    let qml = QML_SOURCE;
    assert!(qml.contains("function permissionOriginForView(view)"));
    assert!(qml.contains("function reloadViewsForPermission(origin)"));
    assert!(qml.contains("entry.view"));
    assert!(qml.contains("window.removePermissionGroups"));
    assert!(qml.contains("candidates[k].view.reload()"));
    assert!(qml.contains("secondaryWindow, secondaryUi, secondaryWindow.activeView,"));
    assert!(qml.contains("Permission revoked; reloaded "));
}

#[test]
fn media_keys_and_mpris_use_one_engine_toggle_path() {
    let qml = QML_SOURCE;
    let mpris = include_str!("../mpris_controller.cpp");
    let mpris_header = include_str!("../mpris_controller.h");
    assert!(qml.contains("Qt.Key_MediaTogglePlayPause"));
    assert!(qml.contains("WebEngineView.ToggleMediaPlayPause"));
    assert!(qml.contains("function triggerMediaToggle(view)"));
    assert!(qml.contains("view.recentlyAudible !== true"));
    assert!(qml.contains("!/^https?:\\/\\//i.test(url)"));
    assert!(qml.contains("ui.feature_desktop_media_keys_enabled"));
    assert!(qml.contains("Media play/pause toggle sent to the page"));
    assert!(qml.contains("MprisController"));
    assert!(qml.contains("onMediaToggleRequested"));
    assert!(qml.contains("mprisController.update"));
    assert!(qml.contains("var url = view.url ? view.url.toString() : \"\""));
    assert!(qml.contains("browserUi.current_url"));
    assert!(qml.contains("function updateMprisForPrimaryView(view)"));
    assert!(qml.contains("window.updateMprisForPrimaryView(webView)"));
    assert!(qml.contains("onRecentlyAudibleChanged"));
    assert!(qml.contains("onAudioMutedChanged"));
    assert!(mpris.contains("org.mpris.MediaPlayer2.Player"));
    assert!(mpris.contains("PlayPause"));
    assert!(mpris.contains("org.mpris.MediaPlayer2.ferric-browser.instance"));
    assert!(mpris.contains("privateProfile"));
    assert!(mpris.contains("safeMetadataUrl"));
    assert!(mpris.contains("sensitiveQueryKey"));
    assert!(mpris.contains("access_token"));
    assert!(mpris.contains("refresh_token"));
    assert!(mpris.contains("client_secret"));
    assert!(mpris.contains("setFragment({})"));
    assert!(mpris_header.contains("QML_NAMED_ELEMENT(MprisController)"));
    assert!(!mpris_header.contains("void Stop()"));
    assert!(!qml.contains("Key_MediaNext"));
    assert!(!qml.contains("Key_MediaPrevious"));
}

#[test]
fn file_urls_encode_download_paths_without_leaking_raw_delimiters() {
    assert_eq!(
        path_to_file_url(Path::new("/home/tom/Downloads/a file#1.txt")),
        Ok("file:///home/tom/Downloads/a%20file%231.txt".into())
    );
    assert!(path_to_file_url(Path::new("relative/file.txt")).is_err());
    assert_eq!(
        configured_download_directory_path(&serde_json::json!({
            "downloads": {"directory": {"path": "/tmp/ferric-browser-downloads"}}
        })),
        PathBuf::from("/tmp/ferric-browser-downloads")
    );
    assert_eq!(
        configured_download_directory_path(&serde_json::json!({
            "downloads": {"directory": {"Path": "/tmp/ferric-browser-downloads"}}
        })),
        PathBuf::from("/tmp/ferric-browser-downloads")
    );
}

#[test]
fn xdg_user_dirs_resolve_downloads_without_shell_expansion() {
    let home = Path::new("/home/test-user");
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n", home),
        Some(PathBuf::from("/home/test-user/Downloads"))
    );
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Work\\x20Downloads\"\n", home),
        Some(PathBuf::from("/home/test-user/Work Downloads"))
    );
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"relative/Downloads\"\n", home),
        None
    );
    assert_eq!(
        parse_user_dirs_download(
            "XDG_DOWNLOAD_DIR=\"$HOME/Downloads; touch /tmp/pwned\"\n",
            home
        ),
        Some(PathBuf::from("/home/test-user/Downloads; touch /tmp/pwned"))
    );
    assert!(parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Down\\qloads\"\n", home).is_none());
}

#[test]
fn focus_overlay_controller_owns_capture_state_and_only_restores_live_targets() {
    let controller = include_str!("../../qml/components/FerricFocusOverlayController.qml");
    let runtime = concat!(
        include_str!("../../qml/components/FerricBrowserRuntimeBase.qml"),
        include_str!("../../qml/components/FerricBrowserRuntimeServices.qml"),
        include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml"),
        include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml"),
        include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml"),
        include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml"),
    );
    assert!(controller.contains("required property var browserWindow"));
    assert!(controller.contains("property var focusReturnStack"));
    assert!(controller.contains("function captureOverlayFocus"));
    assert!(controller.contains("function restoreOverlayFocus"));
    assert!(controller.contains("function focusTargetAvailable"));
    assert!(controller.contains("(controller.focusReturnStack || []).length > 0"));
    assert!(controller.contains("Qt.callLater"));
    assert!(runtime.contains("FerricFocusOverlayController"));
    assert!(runtime.contains("focusOverlayController.captureOverlayFocus"));
    assert!(runtime.contains("focusOverlayController.restoreOverlayFocus"));
}

#[test]
fn native_window_close_requires_a_user_decision_before_shutdown_checks() {
    let root = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let surface = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let popup = include_str!("../../qml/components/FerricPopupWindow.qml");

    let root_close = root
        .split("onClosing: function(close)")
        .nth(1)
        .expect("primary native close handler")
        .split("property string startupUrl")
        .next()
        .expect("primary close handler boundary");
    assert!(root_close.contains("close.accepted = false"));
    assert!(root_close.contains("window.beginQuitRequest(\":window-close\")"));

    let begin = presentation
        .split("function beginQuitRequest(commandText)")
        .nth(1)
        .expect("primary close confirmation entry point")
        .split("function continueQuitRequest()")
        .next()
        .expect("primary confirmation boundary");
    assert!(begin.contains("window.shutdownConfirmationVisible = true"));
    assert!(!begin.contains("window.checkPageStateBeforeQuit()"));

    let continuation = presentation
        .split("function continueQuitRequest()")
        .nth(1)
        .expect("primary post-confirmation continuation")
        .split("function finalizeQuit()")
        .next()
        .expect("primary continuation boundary");
    assert!(continuation.contains("window.checkPageStateBeforeQuit()"));
    assert!(surface.contains("promptVisible: window.shutdownConfirmationVisible"));
    assert!(surface.contains("window.continueQuitRequest()"));

    for (name, window_source) in [("secondary", secondary), ("popup", popup)] {
        assert!(
            window_source.contains("close.accepted = false"),
            "{name} must reject the native close event until confirmed"
        );
        assert!(
            window_source.contains("windowCloseConfirmationVisible = true"),
            "{name} must expose its close confirmation"
        );
        assert!(
            window_source.contains("promptVisible: popupWindow.windowCloseConfirmationVisible")
                || window_source
                    .contains("promptVisible: secondaryWindow.windowCloseConfirmationVisible"),
            "{name} must render the shared keyboard-first confirmation"
        );
    }

    assert!(root.contains("host.continueQuitRequest()"));
    assert!(root.contains("popup.continueQuitRequest()"));
}

#[test]
fn webengine_surface_recovery_is_frame_synchronized_and_wired_to_every_window() {
    let controller = include_str!("../../qml/components/FerricWebEngineSurfaceRecovery.qml");
    let primary = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let popup = include_str!("../../qml/components/FerricPopupWindow.qml");
    let devtools = include_str!("../../qml/components/FerricDevToolsWindow.qml");
    let root = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let executable = include_str!("../../../ferric-browser/src/main.rs");

    assert!(controller.contains("required property var hostWindow"));
    assert!(controller.contains("property var frameSource: hostWindow"));
    assert!(controller.contains("function onFrameSwapped()"));
    assert!(!controller.contains("Qt.callLater"));
    assert!(controller.contains("WebEngineView.LifecycleState.Active"));
    assert!(controller.contains("state.inactiveSincePresentation"));
    assert!(!controller.contains("Timer {"));
    assert!(!controller.contains("runJavaScript"));
    assert!(!controller.contains(".reload("));
    assert!(!controller.contains("Private"));

    for host in [primary, secondary, popup, devtools] {
        assert!(host.contains("FerricWebEngineSurfaceRecovery"));
        assert!(host.contains("nativeWayland"));
        assert!(host.contains("softwareRendering"));
    }
    assert!(primary.contains("window.activeWebView()"));
    assert!(primary.contains("attachedDevToolsLoader.item"));
    assert!(secondary.contains("secondaryWindow.activeView"));
    assert!(secondary.contains("secondaryDevToolsLoader.item"));
    assert!(popup.contains("views: [popupView]"));
    assert!(devtools.contains("views: [detachedDevToolsView]"));
    assert!(root.contains("property bool nativeWayland: false"));
    assert!(executable.contains("platform.starts_with(\"wayland\")"));
    assert!(executable.contains("QString::from(\"nativeWayland\")"));
}

#[test]
fn statusbar_visibility_is_mode_aware_in_every_browser_window() {
    let presentation = include_str!("../../qml/scripts/ChromePresentation.js");
    let root = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let primary = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let popup = include_str!("../../qml/components/FerricPopupWindow.qml");

    assert!(presentation.contains("function statusBarVisible(policy, mode)"));
    assert!(presentation.contains("mode === \"command\" || mode === \"search\""));
    assert!(presentation.contains("policy === \"in-mode\""));
    assert!(root.contains("property string statusbarMode: \"in-mode\""));
    assert!(root.contains("statusBarVisibleForMode(browserUi.mode)"));
    assert!(primary.contains("statusVisible: window.statusBarVisible"));
    assert!(secondary.contains("statusVisible: secondaryWindow.statusBarVisible"));
    assert!(secondary.contains("anchors.bottomMargin: secondaryWindow.bottomChromeHeight"));
    assert!(popup.contains("visible: popupWindow.statusBarVisible"));
    assert!(popup.contains("anchors.bottomMargin: popupWindow.statusBarVisible"));
}

#[test]
fn inherited_runtime_layers_cross_component_scopes_through_explicit_properties() {
    // Characterization: QML ids are file-local. Direct references to ids from
    // an ancestor runtime layer launch successfully only when exposed through
    // an explicit alias/property boundary.
    let runtime = QML_SOURCE;
    assert!(runtime.contains(
        "readonly property alias requestPresentationControllerObject: requestPresentationController"
    ));
    assert!(runtime.contains(
        "readonly property alias browserProfilePrototypeObject: browserProfilePrototype"
    ));
    assert!(runtime.contains(
        "readonly property var requestPresentationController: window.requestPresentationControllerObject"
    ));
    assert!(
        runtime
            .contains("readonly property var requestInterceptor: window.primaryRequestInterceptor")
    );
    for boundary in [
        "bindingOverlayTimer: window.bindingOverlayTimerObject",
        "engineUpdateNoticeTimer: window.engineUpdateNoticeTimerObject",
        "externalOpenPortalTimer: window.externalOpenPortalTimerObject",
        "notificationPresenter: window.notificationPresenterObject",
        "siteDataClearPollTimer: window.siteDataClearPollTimerObject",
    ] {
        assert!(
            runtime.contains(boundary),
            "missing QML boundary {boundary}"
        );
    }
    assert!(runtime.contains("browserUi: window.browserUi"));
    assert!(!runtime.contains("browserUi: browserUi"));
    assert!(!runtime.contains("onContext_route_jsonChanged"));
    assert!(!runtime.contains("onTheme_palette_jsonChanged"));
    assert!(!runtime.contains("onSystem_font_scale_jsonChanged"));
}

#[test]
fn chrome_presentation_controller_projects_browser_ui_facts_without_navigation_authority() {
    let controller = include_str!("../../qml/components/FerricChromePresentationController.qml");
    assert!(controller.contains("required property var browserWindow"));
    assert!(controller.contains("required property var browserUi"));
    assert!(controller.contains("function refreshChromeAppearance"));
    assert!(controller.contains("ChromePresentation.contrastReport"));
    assert!(!controller.contains("WebEngineView"));
    assert!(!controller.contains("runJavaScript"));
}

#[test]
fn window_registry_controller_keeps_ephemeral_owners_per_window_token() {
    let controller = include_str!("../../qml/components/FerricWindowRegistryController.qml");
    assert!(controller.contains("required property var browserWindow"));
    assert!(controller.contains("required property var browserUi"));
    assert!(controller.contains("function retainEphemeralProfileOwner"));
    assert!(controller.contains("function releaseEphemeralProfileOwner"));
    assert!(controller.contains("function ephemeralProfileForToken"));
    assert!(controller.contains("current.profile !== profile"));
    assert!(controller.contains("delete owners[requested]"));
}

#[test]
fn clean_branch_window_requires_distinct_off_record_profile_lease_before_navigation() {
    let controller = include_str!("../../qml/components/FerricWindowRegistryController.qml");
    let window = include_str!("../../qml/components/FerricBrowserWindow.qml");
    let root = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    assert!(controller.contains("function retainCleanBranchOwner(token, host, profile, view)"));
    assert!(controller.contains("profile === controller.browserWindow.browserProfile"));
    assert!(controller.contains("profile.offTheRecord !== true"));
    assert!(controller.contains("view.profile !== profile"));
    assert!(controller.contains("existing.profile === profile"));
    assert!(controller.contains("function releaseCleanBranchOwner(token, host, profile, view)"));
    assert!(window.contains("readonly property bool windowCleanBranch"));
    assert!(window.contains("windowEphemeralProfile || windowCleanBranch"));
    assert!(window.contains("rootWindow.retainCleanBranchOwner("));
    assert!(window.contains("Clean branch profile isolation unavailable"));
    assert!(window.contains("rootWindow.releaseCleanBranchOwner("));
    assert!(window.contains("property bool windowCleanBranchNativeBound: false"));
    assert!(window.contains("property bool windowCleanBranchNavigationAdmitted: false"));
    assert!(window.contains("&& !secondaryWindow.windowCleanBranchNavigationAdmitted"));
    assert!(window.contains("property bool windowCleanBranchLineagePublished: false"));
    assert!(window.contains("|| !secondaryWindow.windowCleanBranchLineagePublished"));
    assert!(window.contains("|| !secondaryWindow.windowCleanBranchNativeBound"));
    assert!(window.contains("|| secondaryWindow.windowStartupUrl !== \"about:blank\""));
    assert!(window.contains("if (secondaryWindow.windowCleanBranch) {\n            // A prepared child must stay inert"));
    assert!(root.contains("function retainCleanBranchOwner(token, host, profile, view)"));
}

#[test]
fn request_presentation_controller_bounds_dialog_text_and_has_no_resolution_authority() {
    let controller = include_str!("../../qml/components/FerricRequestPresentationController.qml");
    assert!(controller.contains("required property var browserWindow"));
    assert!(controller.contains("function boundedPageDialogText"));
    assert!(controller.contains("text.length > 4096"));
    assert!(controller.contains("function permissionCanRemember"));
    assert!(controller.contains("function pageDialogType"));
    assert!(!controller.contains("resolveQtRequest"));
    assert!(!controller.contains("runJavaScript"));
}

#[test]
fn versioning_adapter_keeps_root_qml_composition_only() {
    let root = include_str!("../../qml/Main.qml");
    assert!(root.contains("FerricPrimaryBrowserWindow"));
    assert!(!root.contains("versioning"));
    assert!(!root.contains("localStorage"));
    assert!(!root.contains("Settings"));
    let adapter = include_str!("../browser_ui_versioning.rs");
    let routing = include_str!("../browser_ui_versioning/capture.rs");
    assert!(routing.contains("present_versioning_effect"));
    assert!(!adapter.contains("StorageRequest"));
    assert!(!adapter.contains("VersioningCoordinator"));
}

#[test]
fn structural_restore_uses_a_tab_capable_blank_primary_window_for_multi_page_plans() {
    let entry = include_str!("../../qml/Main.qml");
    let wrapper = include_str!("../../qml/components/FerricVersionedRestoreWindow.qml");
    let runtime_base = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let runtime_surface = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let presentation = include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let restore_coordinator =
        include_str!("../../qml/components/FerricVersioningRestoreCoordinator.qml");
    let restore_host = include_str!("../../qml/components/FerricVersioningRestoreHost.qml");
    let adapter = include_str!("../browser_ui_versioning/restore.rs");
    let routing = include_str!("../browser_ui_library_versioning.rs");
    let application = include_str!("../../../ferric-browser-application/src/versioning/restore.rs");

    assert!(entry.contains("tabCapableWindowFactory: tabCapableWindowFactoryComponent"));
    assert!(entry.contains("id: tabCapableWindowFactoryComponent"));
    assert!(entry.contains("FerricVersionedRestoreWindow { }"));
    assert!(wrapper.contains("FerricPrimaryBrowserWindow {"));
    assert!(wrapper.contains("instanceRequestOwner: false"));
    assert!(runtime_base.contains("property bool instanceRequestOwner: true"));
    assert!(runtime_base.contains("function finishOwnedWindowClose()"));
    assert!(runtime_base.contains("if (window.instanceRequestOwner) {\n            Qt.quit()\n        } else {\n            window.close()"));
    assert!(!runtime_surface.contains("Qt.quit()"));
    assert!(runtime_surface.contains("if (window.instanceRequestOwner)"));
    assert!(runtime_surface.contains("if (window.instanceRequestOwner\n                && !window.temporaryProfile && !window.recoveryAvailable)"));
    assert!(runtime_base.contains("FerricVersioningWindowOwner"));
    assert!(runtime_base.contains("window.versioningWindowOwner.requestCloseAll()"));
    assert!(presentation.contains("}, ui, window.versioningWindowOwner)"));
    assert!(presentation.contains("versioningRestoreSharedProfile: sourceProfile"));
    assert!(presentation.contains("sourceProfile !== browserProfile"));
    assert!(runtime_base.contains("enabled: !window.versioningRestoreSharedProfile"));
    assert!(runtime_surface.contains("if (!window.versioningRestoreSharedInterceptor"));
    assert!(runtime_surface.contains("window.dispatchRoutedIpc(browserUi.poll_ipc())"));
    assert!(runtime_base.contains("function dispatchRoutedIpc(windowIds)"));
    assert!(runtime_base.contains("match.ui.poll_routed_ipc()"));
    assert!(runtime_base.contains("browserUi.reject_routed_ipc(id)"));
    assert!(runtime_surface.contains(
        "if (window.instanceRequestOwner) {\n            browserUi.activate_runtime_wake()"
    ));
    assert!(runtime_base.contains("property var tabCapableWindowFactory: null"));
    assert!(runtime_base.contains("property string versioningRestoreStartupCommitId"));
    assert!(runtime_base.contains("property string versioningOpenRestoreCommitId"));
    assert!(runtime_base.contains("property double versioningOpenRestoreOptionsAt"));
    assert!(runtime_base.contains("property bool versioningOpenRestoreMediaPositionsRequested"));
    assert!(runtime_base.contains("property bool versioningOpenRestoreEditableDraftsRequested"));
    assert!(runtime_surface.contains("request_versioned_pristine_tab_restore("));
    assert!(runtime_surface.contains("FerricVersioningRestoreHost"));
    assert!(
        runtime_surface
            .contains("versioningRestoreCoordinator.handleSidecarReady(action, allowEditable)")
    );
    assert!(restore_host.contains("Qt.resolvedUrl(\"FerricVersioningRestoreCoordinator.qml\")"));
    assert!(!restore_host.contains("sourceComponent:"));
    assert!(presentation.contains("var restoreFactory = window.tabCapableWindowFactory"));
    assert!(
        presentation
            .contains("String(hostWindow.versioningOpenRestoreCommitId || \"\") === fields[1]")
    );
    assert!(presentation.contains("optionsAgeMs <= 60000"));
    assert!(
        presentation.contains("Tab-capable restore window unavailable; current work remains open")
    );
    assert!(presentation.contains("versioningRestoreStartupCommitId: fields[1]"));
    assert!(presentation.contains("startupProfileOverridesJson: profileOverrides"));
    assert!(presentation.contains("startupConfigJson: String(ui.config_json"));
    assert!(presentation.contains("startupCliOverridesJson: String(ui.cli_overrides_json"));
    assert!(presentation.contains("startupContextsJson: String(ui.contexts_json"));
    assert!(restore_coordinator.contains("browserUi.tab_index_for_id(tabId)"));
    assert!(restore_coordinator.contains("hostWindow.activeWebView() === view"));
    assert!(restore_coordinator.contains("confirm_versioned_restore_editable_preview"));
    assert!(adapter.contains("pending_pristine_restore_tab_surface"));
    assert!(routing.contains("apply_restore_to_pristine_window_with_options("));
    assert!(application.contains("pub fn apply_restore_to_pristine_window_with_options("));
    assert!(routing.contains("format!(\"versioning-restore-window\\t{}\", plan.commit)"));
    assert!(!routing.contains("Multi-page versioned restore needs a tab-capable destination"));
    assert!(routing.contains("Restored {} structural pages"));
    assert!(routing.contains("only the selected safe page was loaded"));
}

#[test]
fn stored_archive_view_uses_exact_materialized_file_and_stays_release_gated() {
    let panel = include_str!("../../qml/components/FerricVersioningPanel.qml");
    let chrome = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let host = include_str!("../../qml/components/FerricVersioningArchiveHost.qml");
    let view = include_str!("../../qml/components/FerricVersioningArchiveView.qml");
    let adapter = include_str!("../browser_ui_versioning/restore.rs");
    let storage_router = include_str!("../browser_ui_library_versioning.rs");
    let interceptor = include_str!("../versioning_archive_interceptor.cpp");
    assert!(panel.contains("readonly property bool archiveOfflineReleaseEnabled: false"));
    assert!(panel.contains("request_versioned_archive_view(panel.payload.id)"));
    assert!(panel.contains(r#"String(browserUi.versioning_archive_view_url || "").length === 0"#));
    assert!(chrome.contains("FerricVersioningArchiveHost {"));
    assert!(chrome.contains("browserUi: window.browserUi"));
    assert!(
        host.contains("active: String(browserUi.versioning_archive_view_url || \"\").length > 0")
    );
    assert!(
        host.contains("authorizedArchiveUrl: archiveHost.browserUi.versioning_archive_view_url")
    );
    assert!(host.contains("sequence: \"Escape\""));
    assert!(host.contains("close_versioned_archive_view(operation)"));
    assert!(host.contains("textFormat: Text.PlainText"));
    assert!(host.contains("versioning_archive_source_url"));
    assert!(host.contains("Source (metadata only):"));
    assert!(host.contains("versioning_archive_captured_at_ms"));
    assert!(host.contains("versioning_archive_fidelity_label"));
    assert!(view.contains("offTheRecord: true"));
    assert!(view.contains("import io.github.ferricbrowser 1.0"));
    assert!(view.contains("activeFocusOnPress: true"));
    assert!(interceptor.contains("QStringLiteral(\"mhtml\")"));
    assert!(view.contains("persistentCookiesPolicy: WebEngineProfile.NoPersistentCookies"));
    assert!(view.contains("settings.javascriptEnabled: false"));
    assert!(view.contains("archiveBlanked()"));
    assert!(view.contains("archiveBlankingTimedOut()"));
    assert!(view.contains("blankWaitExpired"));
    assert!(view.contains("archiveHost.isExactAuthorizedArchiveUrl(request.url)"));
    assert!(view.contains("sanitizeArchiveTitle(archiveView.title)"));
    assert!(view.contains("[bidi]"));
    assert!(view.contains("\\u061c\\u200e-\\u200f"));
    assert!(view.contains("slice(0, 512)"));
    assert!(view.contains("archiveLoaded(archiveHost.archiveTitle)"));
    assert!(view.contains("new URL(String(candidate)).href"));
    assert!(adapter.contains("request_versioned_archive_view(window, commit)"));
    assert!(adapter.contains("close_versioned_archive_view(window, operation)"));
    assert!(storage_router.contains("versioned_archive_view_metadata"));
    assert!(storage_router.contains("archive_view_cleanup_retries < 2"));
}

#[test]
fn application_shutdown_waits_for_archive_blank_and_cleanup() {
    let runtime = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let chrome = include_str!("../../qml/components/FerricBrowserRuntimeChrome.qml");
    let host = include_str!("../../qml/components/FerricVersioningArchiveHost.qml");

    assert!(chrome.contains("versionedArchiveHostObject: versionedArchiveLoader"));
    assert!(runtime.contains("!browserUi.versioned_archive_shutdown_ready()"));
    assert!(runtime.contains("window.archiveShutdownCloseRequested = host.closeArchive()"));
    assert!(runtime.contains("host.item.closeRetryAvailable"));
    assert!(
        host.contains(
            "readonly property bool closeRetryAvailable: archiveSurface.blankWaitExpired"
        )
    );
}

#[test]
fn ui_action_status_does_not_echo_dynamic_action_ids_or_errors() {
    let actions = include_str!("../browser_ui_ui_actions.rs");

    for dynamic_status in [
        "format!(\"Executing {action_id}\")",
        "format!(\"{action_id} failed: {error}\")",
        "format!(\"Executing {resolved_id}\")",
        "format!(\"{resolved_id} failed: {error}\")",
    ] {
        assert!(
            !actions.contains(dynamic_status),
            "dynamic action status returned: {dynamic_status}"
        );
    }
    for fixed_status in [
        "External action started",
        "External action failed",
        "Userscript action started",
        "Userscript action failed",
        "Action started",
        "Action failed",
    ] {
        assert!(
            actions.contains(fixed_status),
            "missing fixed status: {fixed_status}"
        );
    }
}

#[test]
fn binding_rejection_status_uses_fixed_reason_without_command_text() {
    let input = include_str!("../../../ferric-browser-core/src/input.rs");
    let presentation = include_str!("../browser_ui_input.rs");

    assert!(input.contains("binding command does not accept a count"));
    assert!(!input.contains("format!(\"command {command} does not accept a count\")"));
    assert!(presentation.contains("BindingOutcome::Rejected { reason }"));
}

#[test]
fn learning_mode_binding_status_uses_only_the_registered_command_name() {
    let presentation = include_str!("../browser_ui_input.rs");
    let expansion = presentation
        .find("let expanded = match registry.expand_command(parsed)")
        .expect("binding command is expanded through the registry");
    let feedback_name = presentation
        .find("let registered_command_name = parsed.name.clone();")
        .expect("validated command name is captured for learning feedback");
    let dispatch = presentation
        .find("invocation: CommandInvocation::with_count(parsed, count)")
        .expect("expanded command is dispatched");
    let feedback = presentation
        .find("format!(\"Binding executed: {registered_command_name}\")")
        .expect("learning feedback uses the registered command name");

    assert!(expansion < feedback_name && feedback_name < dispatch && dispatch < feedback);
    assert!(!presentation.contains("format!(\"Binding executed: {command}\")"));
}

#[test]
fn hint_status_does_not_echo_configured_targets_or_typed_filters() {
    let source = include_str!("../browser_ui_navigation.rs");
    assert!(source.contains("Hints ready · type a label or click"));
    assert!(source.contains("Hint results updated"));
    assert!(source.contains("Hint link sent\""));
    assert!(source.contains("Hint userscript started\""));
    assert!(source.contains("Hint userscript action started"));
    assert!(source.contains("Rapid hint userscript started"));

    for private_status in [
        "format!(\"{target} · {typed} · {remaining}/{total}\")",
        "format!(\"Hint link sent to {target_name}\")",
        "format!(\"Hint userscript started: {script}\")",
        "Hint userscript action started: {action_id}",
        "Rapid hint userscript started: {script}",
        "{} hint(s) · type a label or click",
    ] {
        assert!(
            !source.contains(private_status),
            "caller/configuration data returned to status: {private_status}"
        );
    }

    // The structured result remains the explicit action response; only the
    // shared status-bar projection is details-free.
    assert!(source.contains("\"target\":target_name"));
    assert!(source.contains("\"userscript\":script"));
}

#[test]
fn selection_search_status_does_not_echo_configured_engine_name() {
    let source = include_str!("../browser_ui_edit_commands.rs");
    assert!(source.contains("Selection search requested\""));
    assert!(source.contains("Searching selection\""));
    assert!(source.contains("\"engine\": engine"));
    assert!(!source.contains("Selection search requested with {engine}"));
    assert!(!source.contains("Searching selection with {engine}"));
}

#[test]
fn command_and_ipc_diagnostics_do_not_echo_caller_values_or_route_ids() {
    let source = include_str!("../browser_ui_context_commands.rs");
    let context_runtime = include_str!("../browser_ui_contexts.rs");
    let ipc_route_validation = include_str!("../browser_ui_ipc_route_validation.rs");
    let browsing_commands = include_str!("../browser_ui_browsing_commands.rs");
    let switcher_query = include_str!("../browser_ui_query_switcher.rs");
    let switcher_actions = include_str!("../browser_ui_ipc_switcher_actions.rs");
    let edit_commands = include_str!("../browser_ui_edit_commands.rs");
    for fixed_diagnostic in [
        "window-new has an unsupported option",
        "context-create option requires a value",
        "context-create has an unsupported option",
        "context-route option requires a value",
        "context-route entry point is unsupported",
        "context-route entry point is duplicated",
        "context-route has an unsupported option",
        "context does not exist",
        "context route conflicts with an existing route; choose a unique priority",
    ] {
        assert!(source.contains(fixed_diagnostic), "missing {fixed_diagnostic}");
    }
    for dynamic_diagnostic in [
        "unknown window-new option: {option}",
        "{option} requires a value",
        "unknown context-create option: {option}",
        "unknown context-route option: {option}",
        "unsupported context route entry point: {value}",
        "context route entry point specified more than once: {value}",
        "context not found: {context_name}",
        "route conflicts with {} at priority",
        "Context route {id} added",
        "Context route {route_id} removed",
    ] {
        assert!(
            !source.contains(dynamic_diagnostic),
            "caller-controlled diagnostic returned: {dynamic_diagnostic}"
        );
    }
    assert!(source.contains("Context route added; prompt applies before navigation only"));
    assert!(source.contains("Context route removed; existing navigation chains were not changed"));
    assert!(context_runtime.contains("\"context does not exist\".to_owned()"));
    assert!(!context_runtime.contains("format!(\"context not found: {context_name}\")"));
    for fixed_diagnostic in [
        "requested context is unavailable",
        "requested profile is unavailable",
    ] {
        assert!(ipc_route_validation.contains(fixed_diagnostic));
    }
    assert!(!ipc_route_validation.contains("context not found: {context_name}"));
    assert!(!ipc_route_validation.contains("profile not found: {profile}"));
    assert!(browsing_commands.contains("search has an unsupported option"));
    assert!(!browsing_commands.contains("unknown search option: {value}"));
    assert!(switcher_query.contains("switcher.query scope is unsupported"));
    assert!(!switcher_query.contains("unknown switcher scope: {scope}"));
    for fixed_diagnostic in [
        "switcher.activate action is not allowed for this result kind",
        "switcher.activate result kind is unsupported",
    ] {
        assert!(switcher_actions.contains(fixed_diagnostic));
    }
    assert!(!switcher_actions.contains("switcher.activate action {action:?}"));
    assert!(!switcher_actions.contains("unknown switcher result kind: {kind}"));
    assert!(edit_commands.contains("caret-move direction is unsupported"));
    assert!(edit_commands.contains("mode-enter value is unsupported"));
    assert!(!edit_commands.contains("unknown caret direction: {direction}"));
    assert!(!edit_commands.contains("unknown mode: {mode}"));
}

#[test]
fn tab_suspend_and_discard_refusals_use_closed_page_data_free_reasons() {
    let presentation =
        include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    for (reason_function, next_function) in [
        ("function tabSuspensionBlockReason", "function requestTabSuspension"),
        ("function tabDiscardBlockReason", "function requestTabDiscard"),
    ] {
        let body = presentation
            .split_once(reason_function)
            .expect("tab lifecycle refusal reason exists")
            .1
            .split_once(next_function)
            .expect("reason function ends before its request handler")
            .0;
        let mut return_count = 0;
        for line in body.lines().map(str::trim) {
            let Some(expression) = line.strip_prefix("return ") else {
                continue;
            };
            let expression = expression.strip_suffix(';').unwrap_or(expression);
            assert!(
                (expression.starts_with('"') && expression.ends_with('"'))
                    || expression == "\"\"",
                "page-derived or computed refusal text must not be surfaced: {expression}"
            );
            return_count += 1;
        }
        assert!(return_count >= 2, "expected closed refusal reasons and allow case");
    }
}

#[test]
fn profile_download_routing_requires_a_validated_root_popup_snapshot() {
    let requests = include_str!("../../qml/components/FerricBrowserRuntimeRequests.qml");
    let runtime_base = include_str!("../../qml/components/FerricBrowserRuntimeBase.qml");
    let presentation =
        include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
    let routing = include_str!("../../qml/scripts/ProfileDownloadRouting.js");
    let popup_ownership = include_str!("../../qml/scripts/PopupWindowOwnership.js");

    let owner = requests
        .split_once("function profileDownloadOwner(download, inspectView)")
        .expect("profile-level download owner")
        .1
        .split_once("function popupEntriesForDownloadRouting()")
        .expect("root popup snapshot helper follows the owner")
        .0;
    assert_eq!(owner.matches("ProfileDownloadRouting.resolve(").count(), 4);
    assert_eq!(owner.matches("popupEntriesForDownloadRouting()").count(), 4);
    assert!(!owner.contains("window.popupWindowRegistry || []"));
    assert!(!owner.contains("window.browserWindowRegistry || []"));
    assert!(owner.contains("window.browserWindowRegistry, ordinaryPopupEntries"));
    assert!(owner.contains("window.browserWindowRegistry, permitPopupEntries"));
    assert!(owner.contains("window.browserWindowRegistry, unpermittedPopupEntries"));
    assert!(owner.contains("window.browserWindowRegistry, popupEntries"));

    let snapshot = requests
        .split_once("function popupEntriesForDownloadRouting()")
        .expect("root popup snapshot boundary")
        .1
        .split_once("function handleDownloadRequested(download)")
        .expect("root popup snapshot helper ends before request handling")
        .0;
    assert!(snapshot.contains("return window.rootPopupEntriesSnapshot()"));
    let root_snapshot = runtime_base
        .split_once("function rootPopupEntriesSnapshot()")
        .expect("shared root popup snapshot")
        .1
        .split_once("function focusExistingContextWindow(")
        .expect("root snapshot helper boundary")
        .0;
    assert!(root_snapshot.contains("owner.rootOwner()"));
    assert!(root_snapshot.contains("PopupWindowOwnership.snapshotPopupEntries(root.hostWindow)"));
    assert!(root_snapshot.contains("return null"));
    let notification_focus = runtime_base
        .split_once("function focusNotificationOrigin(")
        .expect("notification popup routing")
        .1
        .split_once("function rootPopupEntriesSnapshot()")
        .expect("notification focus boundary")
        .0;
    assert!(notification_focus.contains("window.rootPopupEntriesSnapshot()"));
    assert!(!notification_focus.contains("window.popupWindowRegistry || []"));
    let live_downloads = presentation
        .split_once("function liveDownloadForId(id)")
        .expect("download metrics owner lookup")
        .1
        .split_once("function downloadMetrics(")
        .expect("live download lookup boundary")
        .0;
    assert!(live_downloads.contains("window.rootPopupEntriesSnapshot()"));
    assert!(live_downloads.contains(
        "var browserEntries = window.browserWindowRegistry"
    ));
    assert!(live_downloads.contains("ProfileDownloadRouting.liveRequestForId("));
    assert!(live_downloads.contains(
        "id, window, browserEntries, popupEntries, window.browserProfile"
    ));
    assert!(presentation.contains(
        "import \"../scripts/ProfileDownloadRouting.js\" as ProfileDownloadRouting"
    ));
    assert!(!live_downloads.contains("window.browserWindowRegistry || []"));
    assert!(!live_downloads.contains("window.popupWindowRegistry || []"));
    assert!(routing.contains(
        "function liveRequestForId(id, sourceHost, browserEntries, popupEntries, profile)"
    ));
    assert!(routing.contains("browserEntries.length > 64"));
    assert!(routing.contains("popupEntries.length > 64"));
    assert!(routing.contains("var browserSnapshots = browserEntries.slice(0)"));
    assert!(routing.contains("var popupSnapshots = popupEntries.slice(0)"));
    assert!(routing.contains("function ownRecordValue(entry, key)"));
    assert!(routing.contains("Object.getOwnPropertyDescriptor(entry, key)"));
    assert!(routing.contains("ownRecordValue(browserEntry, \"host\")"));
    assert!(routing.contains("ownRecordValue(browserEntry, \"profile\")"));
    assert!(routing.contains("ownRecordValue(browserEntry, \"view\")"));
    assert!(routing.contains("ownRecordValue(popup, \"host\")"));
    assert!(routing.contains("ownRecordValue(popup, \"view\")"));
    assert!(popup_ownership.contains("function ownEntryValue(entry, key)"));
    assert!(popup_ownership.contains("Object.getOwnPropertyDescriptor(entry, key)"));
    assert!(routing.contains("var seenRegistryHosts = [sourceHost]"));
    assert!(routing.contains("var sourceBrowserProfile = sourceHost.browserProfile"));
    assert!(routing.contains("browserOwner.profile === profile"));
    assert!(routing.contains("popupOwner.profile === profile"));
    assert!(routing.contains(
        "typeof candidate !== \"object\" && typeof candidate !== \"function\""
    ));
    assert!(routing.contains("var sourceRegistryEntrySeen = false"));
    assert!(routing.contains("if (browserHost === sourceHost)"));
    assert!(routing.contains("seenPopupHosts.indexOf(popupHost) >= 0"));
    assert!(routing.contains("seenBrowserHosts.indexOf(browserHost) >= 0"));
    assert!(routing.contains("seenBrowserHosts.indexOf(popupHost) >= 0"));
    assert!(routing.contains("!download || !sourceHost || !profile"));
    assert!(routing.contains("var popupProfile = popupHost.popupProfile"));
    assert!(routing.contains("function isRecord(value)"));
    assert!(routing.contains("function indexedRequest(index, id)"));
    assert!(routing.contains("Object.prototype.hasOwnProperty.call(index, id)"));
    assert!(routing.contains(
        "if (!Array.isArray(sourceViews) || !consider(sourceHost, sourceViews))"
    ));
    assert!(routing.contains("var activeDownloads = host.activeDownloads"));
    assert!(routing.contains("var pendingDownloadRequests = host.pendingDownloadRequests"));
    assert!(routing.contains(
        "var downloadCompletionRequests = host.downloadCompletionRequests"
    ));
    assert!(routing.contains("!isRecord(activeDownloads)"));
    assert!(!routing.contains("host.activeDownloads || ({})"));
    assert!(!routing.contains("host.pendingDownloadRequests || ({})"));
    assert!(!routing.contains("host.downloadCompletionRequests || ({})"));
    assert!(routing.contains("return resolveUnchecked(download, view, sourceHost, restoredHost,"));
    assert!(routing.contains("catch (error) {\n        return undefined\n    }"));
}
