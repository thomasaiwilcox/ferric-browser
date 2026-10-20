use super::{BTreeSet, Path, fs};

pub(super) fn check_architecture_boundaries() -> Result<(), String> {
    check_identity_and_layer_boundaries()?;
    check_qt_adapter_boundaries()?;
    check_core_and_error_boundaries()
}

pub(super) fn check_identity_and_layer_boundaries() -> Result<(), String> {
    let retired_markers = [
        ["Rust", "Browser"].concat(),
        ["rust", "browser"].concat(),
        ["io.github.", "rust", "browser"].concat(),
    ];
    for root in [Path::new("crates"), Path::new("packaging")] {
        visit_files(root, &mut |path, source| {
            if let Some(marker) = retired_markers
                .iter()
                .find(|marker| source.contains(marker.as_str()))
            {
                return Err(format!(
                    "retired identity {marker:?} is not allowed in production file {}",
                    path.display()
                ));
            }
            Ok(())
        })?;
    }

    for root in [
        Path::new("crates/ferric-browser-core/src"),
        Path::new("crates/ferric-browser-runtime/src"),
        Path::new("crates/ferric-browser-application/src"),
    ] {
        visit_files(root, &mut |path, source| {
            for marker in ["cxx_qt", "QString", "QObject", "QAbstractListModel"] {
                if source.contains(marker) {
                    return Err(format!(
                        "Qt-independent crate boundary violation: {} contains {marker}",
                        path.display()
                    ));
                }
            }
            Ok(())
        })?;
    }

    let application = fs::read_to_string("crates/ferric-browser-application/src/lib.rs")
        .map_err(|error| format!("could not inspect the application boundary: {error}"))?;
    for marker in [
        "pub enum ApplicationInput",
        "pub enum ApplicationEffect",
        "pub fn handle(",
    ] {
        if application.contains(marker) {
            return Err(format!(
                "application concerns must use domain-specific methods, not a catch-all boundary: {marker}"
            ));
        }
    }

    Ok(())
}

pub(super) fn qt_adapter_core_mutation_marker(source: &str) -> Option<&'static str> {
    // The adapter's `state` field now holds the opaque BrowserApplication.
    // Mutable application-service calls (including download metadata) are
    // legitimate. Raw reducer ownership and model writes remain forbidden.
    [
        ".tabs.get_mut(",
        ".windows.get_mut(",
        ".profiles.get_mut(",
        ".journey.clear_matching(",
        ".active_window =",
        ".last_focused_window =",
        ".state.handle(",
    ]
    .into_iter()
    .find(|marker| {
        source.match_indices(*marker).any(|(start, _)| {
            !marker.ends_with(" =") || !source[start + marker.len()..].starts_with('=')
        })
    })
}

pub(super) fn check_qt_adapter_boundaries() -> Result<(), String> {
    let reducer_ownership_markers = ["ApplicationState", "RuntimeOwner", "StateView"];
    let prose_error_markers = ["error.contains(", "error.starts_with(", "error.ends_with("];
    visit_files(
        Path::new("crates/ferric-browser-engine-qt/src"),
        &mut |path, source| {
            if path.extension().is_none_or(|extension| extension != "rs") {
                return Ok(());
            }
            if path.file_name().is_some_and(|name| name == "tests.rs") {
                return Ok(());
            }
            let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
            if let Some(marker) = qt_adapter_core_mutation_marker(production_source) {
                return Err(format!(
                    "Qt adapter mutates application state outside the reducer: {} contains {marker}",
                    path.display()
                ));
            }
            if let Some(marker) = reducer_ownership_markers
                .iter()
                .find(|marker| production_source.contains(**marker))
            {
                return Err(format!(
                    "Qt adapter must use BrowserApplication read projections: {} contains {marker}",
                    path.display()
                ));
            }
            if let Some(marker) = prose_error_markers
                .iter()
                .find(|marker| production_source.contains(**marker))
            {
                return Err(format!(
                    "Qt adapter must not classify public errors from prose: {} contains {marker}",
                    path.display()
                ));
            }
            Ok(())
        },
    )?;
    check_qt_adapter_root()
}

pub(super) fn check_qt_adapter_root() -> Result<(), String> {
    // The generated interface and implementation are separate compilation
    // inputs. Inspect both, while applying implementation budgets separately.
    let adapter = fs::read_to_string("crates/ferric-browser-engine-qt/src/lib.rs")
        .map_err(|error| format!("could not inspect Qt adapter root: {error}"))?;
    if adapter.lines().count() > 3_500 {
        return Err("Qt adapter root must remain a narrow bridge below 3,500 lines".to_owned());
    }
    let production_adapter = adapter
        .split_once("\n#[cfg(test)]\nmod tests;")
        .map(|(production, _)| production)
        .ok_or_else(|| "could not locate Qt adapter test-module boundary".to_owned())?;
    let bridge = fs::read_to_string("crates/ferric-browser-engine-qt/src/bridge.rs")
        .map_err(|error| format!("could not inspect the generated Qt interface: {error}"))?;
    if !bridge.contains("#[cxx_qt::bridge]") || production_adapter.contains("#[cxx_qt::bridge]") {
        return Err("Qt declarations must remain isolated in bridge.rs".to_owned());
    }
    let adapter_source = format!("{production_adapter}\n{bridge}");
    let production_adapter = adapter_source.as_str();
    if !production_adapter.contains("state: Option<BrowserApplication>") {
        return Err(
            "Qt adapter must retain the opaque BrowserApplication service boundary".to_owned(),
        );
    }
    for marker in [
        "dispatch_command(",
        "dispatch_command_for(",
        "dispatch_for_legacy(",
        "reduce(",
    ] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter bypasses the runtime command boundary: {marker}"
            ));
        }
    }
    for marker in ["state: Option<ApplicationState>", ".into_state()"] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter must retain BrowserRuntime ownership instead of core state: {marker}"
            ));
        }
    }
    for marker in [
        "storage_worker: Option<ProfileStoreWorker>",
        "ProfileStoreWorker::spawn(",
        "profile_setup_worker:",
        "ProfileSetupWorker",
        "ProfileActivation",
        "open_profile_store(",
        "load_profile_runtime_overrides(",
        "load_runtime_overrides(",
        "ProfileRegistry::open(",
        "ContextRegistry::open(",
        "StorageCompletions",
        "poll_completions(",
        "pending_history_writes",
        "pending_permission_writes",
        "pending_mark_writes",
        "pending_download_writes",
        "pending_journey_writes",
    ] {
        if production_adapter.contains(marker) {
            return Err(format!(
                "Qt adapter must not own a profile metadata worker: {marker}"
            ));
        }
    }

    let browser_ui_state = production_adapter
        .split("pub struct BrowserUiRust")
        .nth(1)
        .and_then(|source| source.split("struct ContextSnapshot").next())
        .ok_or_else(|| "could not locate Qt BrowserUi state boundary".to_owned())?;
    for marker in [
        "Option<ProfileStore>",
        "Option<ProfileLock>",
        "Option<ContextRegistry>",
        "Option<ProfileStoreWorker>",
    ] {
        if browser_ui_state.contains(marker) {
            return Err(format!(
                "Qt BrowserUi state must project application-owned profile resources, not own {marker}"
            ));
        }
    }
    if !production_adapter.contains("state: Option<BrowserApplication>") {
        return Err("Qt adapter must keep browser ownership behind BrowserApplication".to_owned());
    }
    check_shared_ui_contracts(production_adapter)
}

// This is a deliberately linear audit checklist; keeping related boundary
// assertions together makes the architectural contract reviewable.
#[allow(clippy::too_many_lines)]
pub(super) fn check_shared_ui_contracts(production_adapter: &str) -> Result<(), String> {
    let config_projection =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/config_projection.rs")
            .map_err(|error| format!("could not inspect Qt config projection: {error}"))?;
    for marker in [
        "config_value_with_layers",
        "apply_runtime_overrides(",
        "resolve_runtime_override_layers",
        "presentation_config_with_layers",
    ] {
        if production_adapter.contains(marker) || config_projection.contains(marker) {
            return Err(format!(
                "Qt adapter must not define configuration-layer policy: {marker}"
            ));
        }
    }
    let ipc_decoder =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/ipc_command_decoder.rs")
            .map_err(|error| format!("could not inspect the Qt IPC command decoder: {error}"))?;
    if !ipc_decoder.contains("ferric_browser_ipc::decode_command_invocation") {
        return Err("Qt command decoding must use the shared typed IPC invocation".to_owned());
    }
    let cli = fs::read_to_string("crates/ferric-browser/src/main.rs")
        .map_err(|error| format!("could not inspect the CLI command encoder: {error}"))?;
    if !cli.contains("encode_command as command_params") {
        return Err("CLI command encoding must use the shared IPC command codec".to_owned());
    }
    let ipc_codec = fs::read_to_string("crates/ferric-browser-ipc/src/command_codec.rs")
        .map_err(|error| format!("could not inspect the shared IPC command codec: {error}"))?;
    if !ipc_codec.contains("crate::CommandEnvelope::new") {
        return Err("IPC command encoding must produce the shared typed envelope".to_owned());
    }
    for module in [
        "command_codec_navigation.rs",
        "command_codec_tabs_windows.rs",
        "command_codec_configuration.rs",
        "command_codec_library.rs",
        "command_codec_content.rs",
        "command_codec_contexts.rs",
        "command_codec_automation.rs",
        "command_argument_decoder.rs",
        "command_argument_validation.rs",
    ] {
        let path = Path::new("crates/ferric-browser-ipc/src").join(module);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not inspect IPC family module {module}: {error}"))?;
        if source.lines().count() > 1_000 {
            return Err(format!(
                "IPC family module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }
    if production_adapter.contains("settings_row_") {
        return Err("Qt settings state must use structured rows, not parallel arrays".to_owned());
    }
    let background_workers = fs::read_to_string(
        "crates/ferric-browser-engine-qt/src/background_workers.rs",
    )
    .map_err(|error| format!("could not inspect extracted Qt background workers: {error}"))?;
    if background_workers.lines().count() > 1_000
        || !background_workers.contains("pub(super) struct PrintWorker")
        || production_adapter.contains("struct PrintWorker")
        || production_adapter.contains("struct UserscriptManagerWorker")
    {
        return Err(
            "Qt background workers must remain extracted from the QObject bridge root".to_owned(),
        );
    }
    let script_jobs =
        fs::read_to_string("crates/ferric-browser-application/src/userscript_jobs.rs")
            .map_err(|error| format!("could not inspect application userscript jobs: {error}"))?;
    if !script_jobs.contains("pub(crate) struct UserscriptManagerWorker")
        || background_workers.contains("struct UserscriptManagerWorker")
    {
        return Err("Userscript jobs must be owned by the application profile.".into());
    }
    let browser_ui_state =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/browser_ui_state.rs")
            .map_err(|error| format!("could not inspect extracted Qt browser state: {error}"))?;
    if browser_ui_state.lines().count() > 1_000
        || !browser_ui_state.contains("pub(super) enum ProfilePersistence")
        || !browser_ui_state.contains("pub(super) struct SwitcherQueryCache")
        || production_adapter.contains("enum ProfilePersistence")
        || production_adapter.contains("struct SwitcherQueryCache")
    {
        return Err(
            "Qt browser presentation state must remain extracted from the QObject bridge root"
                .to_owned(),
        );
    }
    for module in [
        "userscript.rs",
        "userscript_discovery.rs",
        "userscript_install.rs",
        "userscript_manifest.rs",
        "userscript_protocol.rs",
        "userscript_storage.rs",
    ] {
        let path = Path::new("crates/ferric-browser-application/src").join(module);
        let source = fs::read_to_string(&path).map_err(|error| {
            format!("could not inspect extracted userscript module {module}: {error}")
        })?;
        if source.lines().count() > 1_000 {
            return Err(format!(
                "userscript module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }
    let qml_root = fs::read_to_string("crates/ferric-browser-engine-qt/qml/Main.qml")
        .map_err(|error| format!("could not inspect the QML root: {error}"))?;
    let primary_window = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricPrimaryBrowserWindow.qml",
    )
    .map_err(|error| format!("could not inspect the primary browser window: {error}"))?;
    let profile_session_surfaces = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricProfileSessionSurfaces.qml",
    )
    .map_err(|error| format!("could not inspect profile/session surfaces: {error}"))?;
    let focus_overlay_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricFocusOverlayController.qml",
    )
    .map_err(|error| format!("could not inspect focus overlay controller: {error}"))?;
    let webengine_surface_recovery = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricWebEngineSurfaceRecovery.qml",
    )
    .map_err(|error| format!("could not inspect WebEngine surface recovery: {error}"))?;
    let chrome_presentation_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricChromePresentationController.qml",
    )
    .map_err(|error| format!("could not inspect chrome presentation controller: {error}"))?;
    let window_registry_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml",
    )
    .map_err(|error| format!("could not inspect window registry controller: {error}"))?;
    let request_presentation_controller = fs::read_to_string(
        "crates/ferric-browser-engine-qt/qml/components/FerricRequestPresentationController.qml",
    )
    .map_err(|error| format!("could not inspect request presentation controller: {error}"))?;
    for module in [
        "FerricWindowLifecycleController.qml",
        "FerricPageServices.qml",
        "FerricWindowPresentationController.qml",
        "FerricWindowRequestsController.qml",
        "FerricCommandEntryController.qml",
        "FerricRuntimeController.qml",
    ] {
        let path = Path::new("crates/ferric-browser-engine-qt/qml/components").join(module);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not inspect QML controller {module}: {error}"))?;
        if !source.contains("required property var host") || source.contains("FerricBrowserRuntime")
        {
            return Err(format!(
                "QML controller {module} must declare its host and must not inherit window behavior"
            ));
        }
    }
    if !qml_root.contains("FerricPrimaryBrowserWindow")
        || !primary_window.contains("ApplicationWindow {")
        || !profile_session_surfaces.contains("FerricProfileManager")
        || !profile_session_surfaces.contains("FerricSessionManager")
        || !profile_session_surfaces.contains("signal createProfileRequested")
        || !profile_session_surfaces.contains("signal saveSessionRequested")
        || !focus_overlay_controller.contains("required property var browserWindow")
        || !focus_overlay_controller.contains("function captureOverlayFocus")
        || !focus_overlay_controller.contains("function restoreOverlayFocus")
        || !webengine_surface_recovery.contains("required property var hostWindow")
        || !webengine_surface_recovery.contains("function onFrameSwapped()")
        || webengine_surface_recovery.contains("Qt.callLater")
        || webengine_surface_recovery.contains("Timer {")
        || webengine_surface_recovery.contains("runJavaScript")
        || webengine_surface_recovery.contains(".reload(")
        || !chrome_presentation_controller.contains("required property var browserWindow")
        || !chrome_presentation_controller.contains("required property var browserUi")
        || !chrome_presentation_controller.contains("function refreshChromeAppearance")
        || !window_registry_controller.contains("required property var browserWindow")
        || !window_registry_controller.contains("function retainEphemeralProfileOwner")
        || !window_registry_controller.contains("function releaseEphemeralProfileOwner")
        || !request_presentation_controller.contains("required property var browserWindow")
        || !request_presentation_controller.contains("function boundedPageDialogText")
        || !request_presentation_controller.contains("function pageDialogType")
        || request_presentation_controller.contains("resolveQtRequest")
    {
        return Err(
            "the primary QML coordinator must remain below 2,500 lines and delegate presentation intents"
                .to_owned(),
        );
    }

    for module in [
        "browser_ui_query_profiles.rs",
        "browser_ui_query_bindings.rs",
        "browser_ui_query_configuration.rs",
        "browser_ui_query_permissions.rs",
        "browser_ui_query_switcher.rs",
        "browser_ui_query_site.rs",
        "browser_ui_query_diagnostics.rs",
    ] {
        let path = Path::new("crates/ferric-browser-engine-qt/src").join(module);
        let source = fs::read_to_string(&path).map_err(|error| {
            format!("could not inspect extracted Qt query module {module}: {error}")
        })?;
        if source.lines().count() > 1_000 {
            return Err(format!(
                "Qt query module {module} exceeds the 1,000-line implementation budget"
            ));
        }
    }

    Ok(())
}

pub(super) fn check_core_and_error_boundaries() -> Result<(), String> {
    let model = fs::read_to_string("crates/ferric-browser-core/src/model.rs")
        .map_err(|error| format!("could not inspect core state model: {error}"))?;
    for marker in [
        "pub(crate) profiles:",
        "pub(crate) windows:",
        "pub(crate) tabs:",
        "pub(crate) journey:",
        "pub(crate) active_window:",
        "pub(crate) revision:",
    ] {
        if model.contains(marker) {
            return Err(format!(
                "application state must remain reducer-private: {marker}"
            ));
        }
    }
    for (path, marker) in [
        ("crates/ferric-browser/src/main.rs", "fn classify_cli_error"),
        (
            "crates/ferric-browser-engine-qt/src/lib.rs",
            "fn ipc_error_code",
        ),
    ] {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("could not inspect public error boundary {path}: {error}"))?;
        if source.contains(marker) {
            return Err(format!(
                "public error codes must be carried by typed errors, not inferred from prose: {path} contains {marker}"
            ));
        }
    }
    let ipc_dispatch_source =
        fs::read_to_string("crates/ferric-browser-engine-qt/src/browser_ui_ipc_dispatch.rs")
            .map_err(|error| format!("could not inspect Qt IPC dispatch: {error}"))?;
    let ipc_dispatch = ipc_dispatch_source
        .split("fn handle_ipc_request")
        .nth(1)
        .ok_or_else(|| "could not locate Qt IPC dispatch boundary".to_owned())?;
    for marker in ["error.contains(", "error.starts_with("] {
        if ipc_dispatch.contains(marker) {
            return Err(format!(
                "IPC dispatch must select public error codes from typed errors, not {marker}"
            ));
        }
    }
    Ok(())
}

pub(super) fn visit_files(
    root: &Path,
    visitor: &mut dyn FnMut(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?
            {
                pending.push(
                    entry
                        .map_err(|error| format!("could not enumerate source: {error}"))?
                        .path(),
                );
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "cpp" | "h" | "qml" | "toml" | "desktop" | "sh")
        ) {
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            visitor(&path, &source)?;
        }
    }
    Ok(())
}

pub(super) fn check_release_freshness_policy() -> Result<(), String> {
    let policy_path = Path::new("docs/RELEASE_POLICY.md");
    let policy = fs::read_to_string(policy_path)
        .map_err(|error| format!("could not read {}: {error}", policy_path.display()))?;
    for marker in [
        "Qt and QtWebEngine release/security advisories",
        "Chromium security notes and the QtWebEngine backport/patch model",
        "Rust dependency updates from the locked graph and their advisories",
        "supported distribution package updates and known-bad ranges",
        "applicable critical vulnerabilities",
        "mitigation or explicit release block",
        "A Chromium base version alone",
        "does not establish a security-patch level",
        "Offline repository tests and Rust dependency scanners do not",
        "constitute an engine-security review",
        "The browser never",
        "updates itself, runs a package manager",
    ] {
        if !policy.contains(marker) {
            return Err(format!(
                "release freshness policy is missing required rule: {marker}"
            ));
        }
    }

    let evidence_path = Path::new("docs/testing/m0-167-release-freshness.md");
    let evidence = fs::read_to_string(evidence_path)
        .map_err(|error| format!("could not read {}: {error}", evidence_path.display()))?;
    for marker in [
        "Status: in progress",
        "mandatory release-candidate review",
        "A real advisory review",
        "remain release responsibilities",
    ] {
        if !evidence.contains(marker) {
            return Err(format!(
                "release freshness evidence is missing required marker: {marker}"
            ));
        }
    }
    Ok(())
}

pub(super) fn check_ui_thread_contract() -> Result<(), String> {
    let paths = [
        Path::new("crates/ferric-browser-engine-qt/src"),
        Path::new("crates/ferric-browser-engine-qt/qml"),
    ];
    let forbidden = ["block_on(", "futures::executor", "tokio::runtime"];
    for root in paths {
        let mut files = vec![root.to_owned()];
        while let Some(path) = files.pop() {
            let metadata = fs::metadata(&path)
                .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if metadata.is_dir() {
                for entry in fs::read_dir(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?
                {
                    files.push(
                        entry
                            .map_err(|error| format!("could not enumerate source: {error}"))?
                            .path(),
                    );
                }
                continue;
            }
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "qml")
            ) {
                continue;
            }
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            if let Some(token) = forbidden.iter().find(|token| source.contains(**token)) {
                return Err(format!(
                    "UI-thread contract violation: {} contains forbidden blocking executor call {token}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn check_accessibility_contract() -> Result<(), String> {
    let root = Path::new("crates/ferric-browser-engine-qt/qml");
    let mut source = String::new();
    visit_files(root, &mut |_, contents| {
        source.push_str(contents);
        source.push('\n');
        Ok(())
    })?;
    let required = [
        "Accessible.name:",
        "Accessible.description:",
        "Accessible.role: Accessible.Dialog",
        "Accessible.role: Accessible.EditableText",
        "Accessible.role: Accessible.List",
        "Accessible.role: Accessible.MenuItem",
        "Accessible.role: Accessible.PageTabList",
        "Accessible.role: Accessible.PageTab",
        "Accessible.role: Accessible.StatusBar",
        "Accessible.description: \"Current result count\"",
        "Accessible.selected:",
        "focus: visible",
        "event.key === Qt.Key_Escape",
    ];
    for marker in required {
        if !source.contains(marker) {
            return Err(format!(
                "accessibility contract violation: {} is missing from {}",
                marker,
                root.display()
            ));
        }
    }
    if source.contains("Accessible.name: \"\"") {
        return Err(format!(
            "accessibility contract violation: an empty Accessible.name exists in {}",
            root.display()
        ));
    }
    Ok(())
}

pub(super) fn check_renderer_failure_contract() -> Result<(), String> {
    let root = Path::new("crates/ferric-browser-engine-qt/qml/components");
    let mut source = String::new();
    for module in [
        "FerricPrimaryBrowserWindow.qml",
        "FerricWindowLifecycleController.qml",
        "FerricPageServices.qml",
        "FerricWindowPresentationController.qml",
        "FerricWindowRequestsController.qml",
        "FerricCommandEntryController.qml",
        "FerricRuntimeController.qml",
    ] {
        let path = root.join(module);
        source.push_str(&fs::read_to_string(&path).map_err(|error| {
            format!(
                "could not read renderer contract module {}: {error}",
                path.display()
            )
        })?);
        source.push('\n');
    }
    for marker in [
        "function rendererTerminationName(status)",
        "WebEngineView.CrashedTerminationStatus",
        "WebEngineView.KilledTerminationStatus",
        "WebEngineView.AbnormalTerminationStatus",
        "function handleRendererProcessTerminated(",
        "ui.note_renderer_process_terminated(tabIndex)",
        "view.rendererFailureAt > 0",
        "now - view.rendererFailureAt <= 60000",
        "function reloadRendererFailure()",
        "ui.prepare_renderer_recovery(window.rendererFailureTabIndex)",
        "repeated crashes will not auto-reload",
        "onRenderProcessTerminated",
    ] {
        if !source.contains(marker) {
            return Err(format!(
                "renderer failure contract violation: {marker} is missing from {}",
                root.display()
            ));
        }
    }
    let handler = source
        .split_once("function handleRendererProcessTerminated(")
        .and_then(|(_, remainder)| remainder.split_once("function showRendererFailureForTab("))
        .map(|(handler, _)| handler)
        .ok_or_else(|| "renderer failure handler boundaries are malformed".to_owned())?;
    if handler.contains("view.reload()") {
        return Err(
            "renderer failure contract violation: termination callback must not auto-reload".into(),
        );
    }
    Ok(())
}

pub(super) fn check_compatibility_report() -> Result<(), String> {
    let path = Path::new("docs/testing/compatibility-qualification.md");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    for marker in [
        "Application commit/package:",
        "Qt build and Chromium security-patch level:",
        "Distribution/compositor/GPU baseline:",
        "Qualification date:",
        "Credential and external-effects policy:",
    ] {
        if !source.contains(marker) {
            return Err(format!(
                "compatibility qualification report is missing `{marker}`"
            ));
        }
    }
    let scenarios = [
        "G-01", "G-02", "G-03", "G-04", "G-05", "G-06", "G-07", "G-08", "G-09", "G-10", "G-11",
        "G-12", "G-13", "G-14", "G-15", "G-16", "G-17",
    ];
    let mut seen = BTreeSet::new();
    for scenario in scenarios {
        let row = source
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("| {scenario} |")))
            .ok_or_else(|| format!("compatibility qualification report omits {scenario}"))?;
        let fields = row.split('|').map(str::trim).collect::<Vec<_>>();
        if fields.len() < 4 {
            return Err(format!(
                "compatibility report row for {scenario} is malformed"
            ));
        }
        let status = fields[2];
        if !matches!(status, "pass" | "fail" | "blocked" | "not-run") {
            return Err(format!(
                "compatibility report row for {scenario} has invalid status `{status}`"
            ));
        }
        if !seen.insert(scenario) {
            return Err(format!("compatibility report duplicates {scenario}"));
        }
    }
    Ok(())
}

pub(super) fn check_compatibility_diagnosis() -> Result<(), String> {
    let path = Path::new("docs/testing/compatibility-diagnosis.md");
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    for heading in [
        "## Application and engine identity",
        "## Observed site and symptom",
        "## Sanitized reproduction",
        "## Minimal Qt reference comparison",
        "## Upstream issue and authorization",
        "## Scoped workaround",
        "## Removal condition",
    ] {
        if !source.contains(heading) {
            return Err(format!("compatibility diagnosis is missing {heading}"));
        }
    }
    for safety_marker in [
        "Do not record credentials",
        "Do not record cookies",
        "Do not copy a qutebrowser workaround",
    ] {
        if !source.contains(safety_marker) {
            return Err(format!(
                "compatibility diagnosis is missing safety rule {safety_marker}"
            ));
        }
    }
    Ok(())
}

pub(super) fn check_capability_matrix(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let header = "| Feature | Public API/module | Minimum | Compiled support | Runtime probe | Tested package | Status | Limitation/test |";
    if !source.lines().any(|line| line.trim() == header) {
        return Err(format!(
            "capability matrix {} has an unexpected table header",
            path.display()
        ));
    }
    let allowed_statuses = [
        "available",
        "in-progress",
        "not-tested",
        "unknown",
        "unavailable",
        "blocked",
    ];
    let mut features = BTreeSet::new();
    for line in source.lines().filter(|line| line.starts_with('|')) {
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if fields.iter().all(|field| field.chars().all(|c| c == '-')) {
            continue;
        }
        if fields.first() == Some(&"Feature") {
            continue;
        }
        if fields.len() != 8 {
            return Err(format!(
                "capability matrix row `{}` has {} fields; expected 8",
                fields.first().copied().unwrap_or("<empty>"),
                fields.len()
            ));
        }
        if fields.iter().any(|field| field.is_empty()) {
            return Err(format!(
                "capability matrix row `{}` contains an empty field",
                fields[0]
            ));
        }
        if !allowed_statuses.contains(&fields[6]) {
            return Err(format!(
                "capability matrix feature `{}` has invalid status `{}`",
                fields[0], fields[6]
            ));
        }
        if !features.insert(fields[0]) {
            return Err(format!(
                "capability matrix duplicates feature `{}`",
                fields[0]
            ));
        }
    }
    for required in [
        "WebAuthn transports",
        "Screen/window capture",
        "System audio sharing",
        "Codecs/DRM",
        "Hardware decode",
        "PDF and printing",
        "Web notifications",
        "Push service",
        "Spellcheck",
        "Primary selection",
        "Per-site settings",
    ] {
        if !features.contains(required) {
            return Err(format!(
                "capability matrix omits required feature `{required}`"
            ));
        }
    }
    Ok(())
}

pub(super) fn check_ui_automation_boundary() -> Result<(), String> {
    let roots = [
        Path::new("crates/ferric-browser-engine-qt/src"),
        Path::new("crates/ferric-browser-engine-qt/qml"),
        Path::new("crates/ferric-browser/src"),
        Path::new("packaging"),
    ];
    // Keep these as split literals so this guard does not match its own source.
    let forbidden = [
        ["QTWEBENGINE_", "REMOTE_DEBUGGING"].concat(),
        ["remote", "-debugging-port"].concat(),
        ["remote", "-debugging-address"].concat(),
        ["automation", "-socket"].concat(),
    ];
    for root in roots {
        let mut files = vec![root.to_owned()];
        while let Some(path) = files.pop() {
            let metadata = fs::metadata(&path)
                .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if metadata.is_dir() {
                for entry in fs::read_dir(&path)
                    .map_err(|error| format!("could not read {}: {error}", path.display()))?
                {
                    files.push(
                        entry
                            .map_err(|error| format!("could not enumerate source: {error}"))?
                            .path(),
                    );
                }
                continue;
            }
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "cpp" | "h" | "qml" | "toml" | "desktop")
            ) {
                continue;
            }
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let lower = source.to_ascii_lowercase();
            if let Some(token) = forbidden
                .iter()
                .find(|token| lower.contains(&token.to_lowercase()))
            {
                return Err(format!(
                    "UI automation boundary violation: {} contains production automation marker {token}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}
