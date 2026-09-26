use super::*;
use crate::qobject::BrowserUi;
use ferric_browser_ipc::ErrorCode;
use input_validation::MAX_JSEVAL_SCRIPT_BYTES;
use process_output::redact_process_stderr_token;
use std::thread;

const ADAPTER_SOURCE: &str = concat!(
    include_str!("lib.rs"),
    include_str!("browser_ui_browsing_commands.rs"),
    include_str!("browser_ui_command_dispatch.rs"),
    include_str!("browser_ui_command_workflows.rs"),
    include_str!("browser_ui_configuration.rs"),
    include_str!("browser_ui_content_tools.rs"),
    include_str!("browser_ui_context_commands.rs"),
    include_str!("browser_ui_contexts.rs"),
    include_str!("browser_ui_controls.rs"),
    include_str!("browser_ui_downloads_permissions.rs"),
    include_str!("browser_ui_edit_commands.rs"),
    include_str!("browser_ui_external_actions.rs"),
    include_str!("browser_ui_input.rs"),
    include_str!("browser_ui_ipc.rs"),
    include_str!("browser_ui_ipc_dispatch.rs"),
    include_str!("browser_ui_ipc_preparation.rs"),
    include_str!("browser_ui_ipc_route_validation.rs"),
    include_str!("browser_ui_ipc_runtime_dispatch.rs"),
    include_str!("browser_ui_ipc_userscripts.rs"),
    include_str!("browser_ui_library.rs"),
    include_str!("browser_ui_journey_commands.rs"),
    include_str!("browser_ui_maintenance.rs"),
    include_str!("browser_ui_navigation.rs"),
    include_str!("browser_ui_navigation_lifecycle.rs"),
    include_str!("browser_ui_process_actions.rs"),
    include_str!("browser_ui_persistence.rs"),
    include_str!("browser_ui_popups.rs"),
    include_str!("browser_ui_profile_setup.rs"),
    include_str!("browser_ui_queries.rs"),
    include_str!("browser_ui_query_bindings.rs"),
    include_str!("browser_ui_query_configuration.rs"),
    include_str!("browser_ui_query_diagnostics.rs"),
    include_str!("browser_ui_query_permissions.rs"),
    include_str!("browser_ui_query_profiles.rs"),
    include_str!("browser_ui_query_site.rs"),
    include_str!("browser_ui_query_switcher.rs"),
    include_str!("browser_ui_repeat_macros.rs"),
    include_str!("browser_ui_runtime.rs"),
    include_str!("browser_ui_runtime_config.rs"),
    include_str!("browser_ui_sessions.rs"),
    include_str!("browser_ui_site.rs"),
    include_str!("browser_ui_spatial.rs"),
    include_str!("browser_ui_storage_writes.rs"),
    include_str!("browser_ui_tab_state.rs"),
    include_str!("browser_ui_tabs.rs"),
    include_str!("browser_ui_ui_actions.rs"),
    include_str!("browser_ui_web_actions.rs"),
    include_str!("browser_ui_ipc_operations.rs"),
    include_str!("browser_ui_ipc_switcher_actions.rs"),
);

const QML_SOURCE: &str = concat!(
    include_str!("../qml/Main.qml"),
    include_str!("../qml/components/FerricPrimaryBrowserWindow.qml"),
    include_str!("../qml/components/FerricBrowserRuntimeBase.qml"),
    include_str!("../qml/components/FerricBrowserRuntimeServices.qml"),
    include_str!("../qml/components/FerricBrowserRuntimePresentation.qml"),
    include_str!("../qml/components/FerricBrowserRuntimeRequests.qml"),
    include_str!("../qml/components/FerricBrowserRuntimeChrome.qml"),
    include_str!("../qml/components/FerricBrowserRuntimeSurface.qml"),
    include_str!("../qml/components/FerricFocusOverlayController.qml"),
    include_str!("../qml/components/FerricChromePresentationController.qml"),
    include_str!("../qml/components/FerricWindowRegistryController.qml"),
    include_str!("../qml/components/FerricRequestPresentationController.qml"),
    include_str!("../qml/components/FerricWebEngineSurfaceRecovery.qml"),
    include_str!("../qml/components/FerricSpatialGridOverlay.qml"),
    include_str!("../qml/components/FerricProfileSessionSurfaces.qml"),
    include_str!("../qml/components/FerricBrowserWindow.qml"),
    include_str!("../qml/components/FerricPopupWindow.qml"),
    include_str!("../qml/components/FerricDevToolsWindow.qml"),
);

fn bootstrap_application(
    privacy: PrivacyKind,
    profile_label: &str,
) -> Option<(BrowserApplication, WindowId, TabId)> {
    BrowserApplication::bootstrap(Config::default(), privacy, profile_label).ok()
}

include!("tests/command_protocol.rs");
include!("tests/components_and_workers.rs");
include!("tests/commands_and_actions.rs");
include!("tests/presentation_boundaries.rs");
include!("tests/context_and_library.rs");
include!("tests/action_runtime.rs");
