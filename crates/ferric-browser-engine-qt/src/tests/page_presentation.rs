use ferric_browser_application::BrowserApplication;
use ferric_browser_application::{PageOpenIntent, PageOpenPrivacy};
use ferric_browser_config::{
    Config, ContextDefinition, ContextRoute, ContextsConfig, TabsPresentation,
};
use ferric_browser_core::PrivacyKind;
use ferric_browser_core::{CommandSource, DispatchTarget};
use ferric_browser_runtime::{RuntimeCommand, RuntimeInput};

use crate::{
    IpcOpenTarget, IpcRoute, ValidatedUrl, browser_ui_context_navigation,
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
        routes: vec![ContextRoute {
            id: "route-1".into(),
            pattern: "https://example.test/*".into(),
            context: "research".into(),
            priority: 1,
            behavior: "confirm".into(),
            entry_points: vec!["explicit-open".into()],
        }],
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

    let mut config = Config::default();
    config.tabs.presentation = TabsPresentation::Windows;
    let (mut application, window, tab) =
        BrowserApplication::bootstrap(config, PrivacyKind::Normal, "normal-profile")
            .expect("bootstrap normal test profile");
    let route = IpcRoute {
        selector: DispatchTarget::Tab(tab),
        open_target: IpcOpenTarget::BackgroundTab,
        profile: None,
        context: None,
        external_open: false,
        source: CommandSource::Ipc,
    };
    let (target, owner_window, owner_token, foreground) =
        browser_ui_context_navigation::capture_context_route_target(
            &application,
            Some(window),
            "window-owner",
            true,
            &route,
            window,
        )
        .expect("capture target and foreground intent as queue does");
    assert_eq!(owner_window, window);
    assert_eq!(owner_token, "window-owner");
    assert!(!foreground);
    let url = ValidatedUrl::parse("https://example.test/").expect("valid route URL");
    let pending = browser_ui_context_navigation::pending_context_route_for_target(
        &contexts,
        &url,
        "explicit-open",
        target,
        window,
        window,
        owner_token,
        foreground,
    )
    .expect("queue matching context route");
    assert_eq!(pending.route_id, "route-1");
    assert_eq!(pending.context_name, "research");
    assert_eq!(pending.profile_name, "research-profile");
    assert_eq!(pending.target, target);
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
    let mut queued_for_accept = Some(pending.clone());
    let accepted = browser_ui_context_navigation::take_current_context_route_record(
        &mut queued_for_accept,
        Some(&application),
        Some(window),
        "window-owner",
        true,
    )
    .expect("current queued target reaches accept completion");
    assert_eq!(accepted.target, target);
    assert!(!accepted.foreground);
    assert!(queued_for_accept.is_none());

    let mut queued_for_dismiss = Some(pending.clone());
    let dismissed = browser_ui_context_navigation::take_current_context_route_record(
        &mut queued_for_dismiss,
        Some(&application),
        Some(window),
        "window-owner",
        true,
    )
    .expect("current queued target reaches dismiss completion");
    assert_eq!(dismissed.target, target);
    assert!(!dismissed.foreground);
    assert!(queued_for_dismiss.is_none());

    let mut replaced_owner = Some(pending.clone());
    assert!(
        browser_ui_context_navigation::take_current_context_route_record(
            &mut replaced_owner,
            Some(&application),
            Some(window),
            "replaced-window-owner",
            true,
        )
        .is_none()
    );
    assert!(replaced_owner.is_none());

    application
        .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Navigate {
            target,
            url: ValidatedUrl::parse("https://example.test/replaced")
                .expect("valid navigation URL"),
        }))
        .expect("replace source document");
    assert_ne!(application.capture_target(tab), Some(target));
    for _completion in ["accept", "dismiss"] {
        let mut queued = Some(pending.clone());
        let mut dispatched_actions = 0;
        if browser_ui_context_navigation::take_current_context_route_record(
            &mut queued,
            Some(&application),
            Some(window),
            "window-owner",
            true,
        )
        .is_some()
        {
            dispatched_actions += 1;
        }
        assert_eq!(dispatched_actions, 0);
        assert!(queued.is_none());
    }
    assert_eq!(application.windows()[&window].context, None);

    let (private_application, private_window, private_tab) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "private-profile")
            .expect("bootstrap private test profile");
    let private_target = private_application
        .capture_target(private_tab)
        .expect("capture private source target");
    let private_pending = browser_ui_context_navigation::pending_context_route_for_target(
        &contexts,
        &url,
        "explicit-open",
        private_target,
        private_window,
        private_window,
        "private-owner".into(),
        false,
    )
    .expect("queue private profile route before target policy check");
    let mut private_queue = Some(private_pending);
    assert!(
        browser_ui_context_navigation::take_current_context_route_record(
            &mut private_queue,
            Some(&private_application),
            Some(private_window),
            "private-owner",
            true,
        )
        .is_none()
    );
    assert!(private_queue.is_none());
}
