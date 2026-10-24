use ferric_browser_application::BrowserApplication;
use ferric_browser_application::{PageOpenIntent, PageOpenPrivacy};
use ferric_browser_config::{Config, ContextDefinition, ContextsConfig, TabsPresentation};
use ferric_browser_core::PrivacyKind;
use ferric_browser_runtime::{RuntimeCommand, RuntimeInput};

use crate::{
    IpcOpenTarget, PendingContextRoute, ValidatedUrl, browser_ui_context_navigation,
    browser_ui_ipc::page_open_intent,
};

#[test]
fn t103_open_targets_map_once_and_keep_background_inactive() {
    let windows = [
        (IpcOpenTarget::Current, PageOpenIntent::CurrentPage),
        (
            IpcOpenTarget::Tab,
            PageOpenIntent::NewPage { foreground: true },
        ),
        (
            IpcOpenTarget::BackgroundTab,
            PageOpenIntent::NewPage { foreground: false },
        ),
        (
            IpcOpenTarget::Window,
            PageOpenIntent::NewWindow {
                privacy: PageOpenPrivacy::Normal,
            },
        ),
        (
            IpcOpenTarget::PrivateWindow,
            PageOpenIntent::NewWindow {
                privacy: PageOpenPrivacy::Private,
            },
        ),
    ];

    for (destination, expected) in windows {
        assert_eq!(
            page_open_intent(&TabsPresentation::Windows, destination),
            expected,
            "destination {destination:?} must map once in windows presentation"
        );
    }

    for destination in [
        IpcOpenTarget::Current,
        IpcOpenTarget::Tab,
        IpcOpenTarget::BackgroundTab,
        IpcOpenTarget::Window,
        IpcOpenTarget::PrivateWindow,
    ] {
        assert_eq!(
            page_open_intent(&TabsPresentation::Internal, destination),
            PageOpenIntent::ExistingPresentation,
            "internal presentation must preserve existing effects for {destination:?}"
        );
    }

    assert_eq!(
        page_open_intent(&TabsPresentation::Windows, IpcOpenTarget::BackgroundTab),
        PageOpenIntent::NewPage { foreground: false },
        "background intent must never be promoted to foreground"
    );

    let PageOpenIntent::NewPage { foreground } =
        page_open_intent(&TabsPresentation::Windows, IpcOpenTarget::BackgroundTab)
    else {
        panic!("background target must reach the new-page dispatcher");
    };
    let dispatched = super::planned_tab_open_command("https://example.test/", foreground);
    assert_eq!(dispatched.name, "tab-open");
    assert_eq!(
        dispatched.arguments,
        vec![
            "--background".to_owned(),
            "--".to_owned(),
            "https://example.test/".to_owned(),
        ]
    );
}

#[test]
fn t104_context_routes_keep_scope_and_reject_stale_targets_before_completion() {
    let contexts = ContextsConfig {
        schema_version: 1,
        contexts: vec![
            ContextDefinition {
                name: "personal".into(),
                label: "Shared label".into(),
                profile: "normal-profile".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "tab".into(),
            },
            ContextDefinition {
                name: "research".into(),
                label: "Shared label".into(),
                profile: "research-profile".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "window".into(),
            },
        ],
        routes: Vec::new(),
    };
    assert_eq!(
        browser_ui_context_navigation::context_definition_by_route_name(&contexts, "personal")
            .map(|context| context.profile.as_str()),
        Some("normal-profile")
    );
    assert_eq!(
        browser_ui_context_navigation::context_definition_by_route_name(&contexts, "research")
            .map(|context| context.profile.as_str()),
        Some("research-profile")
    );

    let (mut application, window, tab) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "normal-profile")
            .expect("bootstrap normal test profile");
    let target = application
        .capture_target(tab)
        .expect("capture source target");
    let pending = PendingContextRoute {
        route_id: "route-1".into(),
        behavior: "confirm".into(),
        context_name: "research".into(),
        profile_name: "research-profile".into(),
        url: "https://example.test/".into(),
        target,
        target_window: window,
        owner_window: window,
        owner_token: "window-owner".into(),
        foreground: false,
        open_route: None,
    };
    assert!(!pending.foreground);
    assert!(
        browser_ui_context_navigation::context_route_host_intent_supported(
            false,
            pending.foreground
        )
    );
    assert!(
        !browser_ui_context_navigation::context_route_host_intent_supported(
            true,
            pending.foreground
        )
    );
    assert!(browser_ui_context_navigation::context_route_host_intent_supported(true, true));
    assert!(
        browser_ui_context_navigation::context_route_target_is_current(
            Some(&application),
            Some(window),
            "window-owner",
            true,
            &pending,
        )
    );
    assert!(
        !browser_ui_context_navigation::context_route_target_is_current(
            Some(&application),
            Some(window),
            "replaced-window-owner",
            true,
            &pending,
        )
    );

    application
        .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Navigate {
            target,
            url: ValidatedUrl::parse("https://example.test/replaced".into())
                .expect("valid navigation URL"),
        }))
        .expect("replace source document");
    assert_ne!(application.capture_target(tab), Some(target));
    assert!(
        !browser_ui_context_navigation::context_route_target_is_current(
            Some(&application),
            Some(window),
            "window-owner",
            true,
            &pending,
        )
    );
    assert_eq!(application.windows()[&window].context, None);

    let (private_application, private_window, private_tab) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "private-profile")
            .expect("bootstrap private test profile");
    let private_pending = PendingContextRoute {
        target: private_application
            .capture_target(private_tab)
            .expect("capture private source target"),
        target_window: private_window,
        owner_window: private_window,
        owner_token: "private-owner".into(),
        ..pending
    };
    assert!(
        !browser_ui_context_navigation::context_route_target_is_current(
            Some(&private_application),
            Some(private_window),
            "private-owner",
            true,
            &private_pending,
        )
    );
}
