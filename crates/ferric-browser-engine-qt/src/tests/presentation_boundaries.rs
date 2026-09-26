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
    assert!(qml.contains("function addressPresentation(value, maxCharacters)"));
    assert!(qml.contains("var origin = safe.slice(0, authorityEnd)"));
    assert!(qml.contains("var separator = tail.charAt(0) === \"/\" ? \"/…\" : \"…\""));
    assert!(qml.contains("Accessible.description: browserUi.display_url"));
    assert!(qml.contains("Accessible.description: secondaryUi.display_url"));
    assert!(qml.contains("addressEditing = activeFocus"));
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
    assert!(qml.contains("navigation_failure_requested_url"));
    assert!(qml.contains("navigation_failure_url"));
    assert!(qml.contains("navigation_failure_kind"));
    assert!(qml.contains("navigation_failed_with_details"));
    assert!(qml.contains("Retry is intentionally not offered"));
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
    let keyboard_presentation =
        include_str!("../../qml/scripts/KeyboardPresentation.js");
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
    assert!(qml.contains("storageName: window.temporaryProfile ? \"\""));
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
    let source = [
        ADAPTER_SOURCE,
        include_str!("../browser_ui_context_navigation.rs"),
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
    assert_eq!(qml.matches("JSON.parse(").count(), 12);
    assert_eq!(qml.matches("JSON.stringify(").count(), 29);
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
    assert!(source.contains("#[qproperty(QString, editor_completion_stderr)]"));
    assert!(source.contains("fn take_editor_completion(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("var editorToken = browserUi.editor_completion_token"));
    assert!(qml.contains("window.editorApplyScript(editorOriginal, editorUpdated)"));
    assert!(!qml.contains("JSON.parse(editorCompletion)"));
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
    assert!(source.contains("fn restore_context_membership"));
    assert!(source.contains("focused_member"));
    assert!(source.contains("restored_tab_descriptors"));
    assert!(source.contains("Runtime tab IDs are deliberately not reused"));
    assert!(source.contains("selected_effects = Some(effects)"));
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
    assert!(
        qml.contains(
            "popupWindow, popupView,\n                      popupWindow.popupPrivateProfile"
        )
    );
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
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricDiagnostics.qml"),
    ]
    .concat();
    let dialogs = include_str!("../../qml/components/FerricFileDialogSurfaces.qml");
    assert!(source.contains("fn export_diagnostics"));
    assert!(source.contains("Diagnostics export requires reviewing the visible preview first"));
    assert!(source.contains("diagnostics_preview_ready = false"));
    assert!(source.contains("diagnostics_preview_payload = Some(serialized.clone())"));
    assert!(source.contains("MAX_DIAGNOSTICS_EXPORT_BYTES"));
    assert!(source.contains("Diagnostics export requires an absolute local file path"));
    assert!(source.contains("Diagnostics export refuses to overwrite an existing file"));
    assert!(source.contains("atomic_write_private(path_ref, payload.as_bytes())"));
    assert!(qml.contains("fileDialogSurfaces.openDiagnosticsExport()"));
    assert!(dialogs.contains("title: \"Choose diagnostics export destination\""));
    assert!(qml.contains("Save diagnostics preview"));
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
    assert!(qml.contains("storageName: window.temporaryProfile ? \"\""));
    assert!(qml.contains("persistentStoragePath: window.temporaryProfile"));
    assert!(qml.contains("cachePath: window.temporaryProfile"));
    assert!(qml.contains(
        "window.browserProfile = window.browserProfilePrototypeObject.instance()"
    ));
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
    assert!(source.contains("cannot save durable sessions"));
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
    assert!(sync < navigate, "the new tab view must exist before navigation");
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
    assert!(script.contains("var VERSION = \"9\""));
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
    assert!(script.contains(
        "family==='all'&&(kind==='image'||kind==='media')&&linkedSurface(el,r)"
    ));
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
    let presentation =
        include_str!("../../qml/components/FerricBrowserRuntimePresentation.qml");
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
    let controller =
        include_str!("../../qml/components/FerricWebEngineSurfaceRecovery.qml");
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
    assert!(runtime.contains(
        "readonly property var requestInterceptor: window.primaryRequestInterceptor"
    ));
    for boundary in [
        "bindingOverlayTimer: window.bindingOverlayTimerObject",
        "engineUpdateNoticeTimer: window.engineUpdateNoticeTimerObject",
        "externalOpenPortalTimer: window.externalOpenPortalTimerObject",
        "notificationPresenter: window.notificationPresenterObject",
        "siteDataClearPollTimer: window.siteDataClearPollTimerObject",
    ] {
        assert!(runtime.contains(boundary), "missing QML boundary {boundary}");
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
