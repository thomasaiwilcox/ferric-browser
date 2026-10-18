use super::*;
use crate::{ProfileActivation, RuntimeInput};
use ferric_browser_config::Config;
use ferric_browser_core::PrivacyKind;
use ferric_browser_runtime::BrowserRuntime;
use ferric_browser_storage::{ProfileLibrarySnapshot, RootSpec, StorageOperation, StorageRoots};
use std::time::{Duration, Instant};
use uuid::Uuid;
const ORIGIN: &str = "https://blocked.example.test";
struct Roots(StorageRoots);
impl Roots {
    fn new() -> Self {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        roots.ensure().unwrap();
        Self(roots)
    }
}
impl Drop for Roots {
    fn drop(&mut self) {
        self.0.clone().cleanup().unwrap();
    }
}
fn app(roots: &Roots, id: Uuid, privacy: PrivacyKind) -> BrowserApplication {
    let mut config = Config::default();
    config.permissions.geolocation = PermissionDecision::Allow;
    app_with_config(roots, id, privacy, config)
}
fn app_with_config(
    roots: &Roots,
    id: Uuid,
    privacy: PrivacyKind,
    config: Config,
) -> BrowserApplication {
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "default").unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, config);
    app.activate_profile(ProfileActivation {
        name: "default".into(),
        id: Some(id),
        privacy,
        roots: Some(roots.0.clone()),
        lock: None,
        contexts: None,
        storage_path: (privacy == PrivacyKind::Normal)
            .then(|| roots.0.data.join(format!("{id}.sqlite"))),
    })
    .unwrap();
    app
}
fn session(app: &BrowserApplication) -> ProfileSession {
    app.active_profile.as_ref().unwrap().session.clone()
}
fn work(app: &BrowserApplication, command: &StorageCommand) -> PermissionWork {
    PermissionWork::prepare(Some(session(app)), command)
        .unwrap()
        .admitted()
}
fn rule(origin: &str, permission: &str, decision: &str) -> PermissionRule {
    PermissionRule {
        origin: origin.into(),
        permission: permission.into(),
        decision: decision.into(),
        expires_at: None,
        updated_at: 1,
    }
}
fn snapshot(rules: Vec<PermissionRule>) -> StorageCompletion {
    StorageCompletion::Library(Ok(ProfileLibrarySnapshot {
        history: vec![],
        command_history: vec![],
        bookmarks: vec![],
        quickmarks: vec![],
        downloads: vec![],
        permissions: rules,
        permissions_complete: true,
    }))
}
fn publish(app: &BrowserApplication, rules: Vec<PermissionRule>) {
    work(app, &StorageCommand::Library { limit: 1 }).finish(&snapshot(rules));
}
fn decision(app: &BrowserApplication) -> PermissionDecision {
    app.resolve_permission(ORIGIN, "geolocation").decision
}
#[test]
fn cold_preferences_do_not_inherit_an_allow_default() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    assert_eq!(decision(&app), PermissionDecision::Ask);
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    publish(&app, vec![]);
    assert_eq!(decision(&app), PermissionDecision::Allow);
    assert!(!app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
}
#[test]
fn unrelated_pending_permission_write_preserves_a_shared_deny() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let other = app.inherit_profile(runtime).unwrap();
    let pending = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule("https://other.example.test", "notifications", "deny")],
        },
    );
    for owner in [&app, &other] {
        assert_eq!(decision(owner), PermissionDecision::Deny);
        assert!(
            !owner
                .resolve_permission(ORIGIN, "geolocation")
                .refresh_needed
        );
    }
    pending.finish(&StorageCompletion::PermissionBatch(Ok(1)));
    assert_eq!(decision(&other), PermissionDecision::Deny);
}
#[test]
fn snapshots_queued_before_or_during_a_write_cannot_remove_its_barrier() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    let old = work(&app, &StorageCommand::Library { limit: 1 });
    let mutation = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    let during = work(&app, &StorageCommand::Library { limit: 1 });
    old.finish(&snapshot(vec![]));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    mutation.finish(&StorageCompletion::PermissionBatch(Ok(1)));
    during.finish(&snapshot(vec![]));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    assert!(!app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
}
#[test]
fn reset_waits_for_a_fresh_snapshot_and_failed_reset_retains_denial() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    let command = StorageCommand::PermissionReset {
        origin: ORIGIN.into(),
        permissions: vec!["geolocation".into()],
    };
    let reset = work(&app, &command);
    assert_eq!(decision(&app), PermissionDecision::Ask);
    reset.finish(&StorageCompletion::failed(
        StorageOperation::PermissionReset,
        "injected write failure",
    ));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    work(&app, &command).finish(&StorageCompletion::PermissionReset(Ok(true)));
    assert_eq!(decision(&app), PermissionDecision::Ask);
    publish(&app, vec![]);
    assert_eq!(decision(&app), PermissionDecision::Allow);
}
#[test]
fn rejected_admission_restores_policy_and_cancelled_work_requires_refresh() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    let command = StorageCommand::PermissionBatch {
        rules: vec![rule(ORIGIN, "geolocation", "deny")],
    };
    drop(PermissionWork::prepare(Some(session(&app)), &command));
    assert_eq!(decision(&app), PermissionDecision::Allow);
    drop(work(&app, &command));
    assert_eq!(decision(&app), PermissionDecision::Ask);
    publish(&app, vec![]);
    assert_eq!(decision(&app), PermissionDecision::Allow);
}
#[test]
fn durable_preferences_do_not_cross_roots_profiles_or_private_sessions() {
    let roots = Roots::new();
    let other_roots = Roots::new();
    let id = Uuid::new_v4();
    let normal = app(&roots, id, PrivacyKind::Normal);
    publish(&normal, vec![rule(ORIGIN, "geolocation", "deny")]);
    let other_root = app(&other_roots, id, PrivacyKind::Normal);
    let other_profile = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let private = app(&roots, id, PrivacyKind::Private);
    assert_eq!(decision(&normal), PermissionDecision::Deny);
    assert_eq!(decision(&other_root), PermissionDecision::Ask);
    assert_eq!(decision(&other_profile), PermissionDecision::Ask);
    assert_eq!(decision(&private), PermissionDecision::Allow);
    assert!(
        PermissionWork::prepare(
            Some(session(&private)),
            &StorageCommand::Library { limit: 1 }
        )
        .is_none()
    );
}
#[test]
fn configured_rules_and_explicit_session_decisions_keep_their_precedence() {
    let roots = Roots::new();
    let mut app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    app.remember_session_permission(ORIGIN, "geolocation", &PermissionDecision::Allow)
        .unwrap();
    assert_eq!(decision(&app), PermissionDecision::Allow);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    assert_eq!(decision(&app), PermissionDecision::Deny);
    let mut config = app.configuration.effective().clone();
    config
        .permission_rules
        .push(ferric_browser_config::PermissionRuleConfig {
            id: "explicit".into(),
            profile: "default".into(),
            origin: ORIGIN.into(),
            permission: "geolocation".into(),
            decision: PermissionDecision::Allow,
        });
    app.replace_configuration(config).unwrap();
    assert_eq!(decision(&app), PermissionDecision::Allow);
    assert_eq!(
        app.resolve_permission(ORIGIN, "screen-capture").decision,
        PermissionDecision::Ask
    );
    assert_eq!(
        app.resolve_permission(ORIGIN, "unsupported").decision,
        PermissionDecision::Deny
    );
}
#[test]
fn expired_rules_do_not_extend_grants_or_denials() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let mut expired = rule(ORIGIN, "geolocation", "deny");
    expired.expires_at = Some(0);
    publish(&app, vec![expired]);
    assert_eq!(decision(&app), PermissionDecision::Allow);
}
fn wait(app: &mut BrowserApplication, ticket: crate::StorageTicket) -> StorageCompletion {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        for effect in app.poll_storage_effects() {
            if effect.ticket == ticket {
                return effect.completion;
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn real_storage_commands_update_the_shared_application_policy() {
    let roots = Roots::new();
    let mut app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let ticket = app
        .submit_storage(StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        })
        .unwrap();
    assert_eq!(decision(&app), PermissionDecision::Deny);
    assert!(matches!(
        wait(&mut app, ticket),
        StorageCompletion::PermissionBatch(Ok(1))
    ));
    let read = app
        .submit_storage(StorageCommand::Library { limit: 1 })
        .unwrap();
    assert!(matches!(
        wait(&mut app, read),
        StorageCompletion::Library(Ok(_))
    ));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    assert!(!app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    let reset = app
        .submit_storage(StorageCommand::PermissionReset {
            origin: ORIGIN.into(),
            permissions: vec!["geolocation".into()],
        })
        .unwrap();
    assert_eq!(decision(&app), PermissionDecision::Ask);
    assert!(matches!(
        wait(&mut app, reset),
        StorageCompletion::PermissionReset(Ok(true))
    ));
    let read = app
        .submit_storage(StorageCommand::Library { limit: 1 })
        .unwrap();
    assert!(matches!(
        wait(&mut app, read),
        StorageCompletion::Library(Ok(_))
    ));
    assert_eq!(decision(&app), PermissionDecision::Allow);
    app.dispatch_runtime(RuntimeInput::Lifecycle(
        ferric_browser_core::Event::RequestShutdown,
    ))
    .unwrap();
    assert_eq!(decision(&app), PermissionDecision::Deny);
}

#[test]
fn late_snapshot_updates_only_its_original_profile_session() {
    let roots = Roots::new();
    let other_roots = Roots::new();
    let mut owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let old_peer = owner.inherit_profile(runtime).unwrap();
    let old_read = work(&owner, &StorageCommand::Library { limit: 1 });
    let id = Uuid::new_v4();
    owner
        .activate_profile(ProfileActivation {
            name: "other".into(),
            id: Some(id),
            privacy: PrivacyKind::Normal,
            roots: Some(other_roots.0.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(other_roots.0.data.join("other.sqlite")),
        })
        .unwrap();
    old_read.finish(&snapshot(vec![rule(ORIGIN, "geolocation", "deny")]));
    assert_eq!(decision(&old_peer), PermissionDecision::Deny);
    assert_eq!(decision(&owner), PermissionDecision::Ask);
}
#[test]
fn failed_reads_preserve_denials_and_failed_writes_do_not_enable_defaults() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let mutation = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    mutation.finish(&StorageCompletion::failed(
        StorageOperation::PermissionBatch,
        "injected failure",
    ));
    assert_eq!(decision(&app), PermissionDecision::Ask);
    work(&app, &StorageCommand::Library { limit: 1 }).finish(&StorageCompletion::failed(
        StorageOperation::Library,
        "injected read failure",
    ));
    assert_eq!(decision(&app), PermissionDecision::Ask);
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    work(&app, &StorageCommand::Library { limit: 1 }).finish(&StorageCompletion::failed(
        StorageOperation::Library,
        "injected unrelated read failure",
    ));
    assert_eq!(decision(&app), PermissionDecision::Deny);
}
#[test]
fn exhausted_observation_epoch_cannot_enable_an_allow_default() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    session(&app).0.durable.lock().unwrap().epoch = u64::MAX;
    let mutation = work(
        &app,
        &StorageCommand::PermissionReset {
            origin: ORIGIN.into(),
            permissions: vec!["geolocation".into()],
        },
    );
    mutation.finish(&StorageCompletion::PermissionReset(Ok(false)));
    publish(&app, vec![]);
    assert_eq!(decision(&app), PermissionDecision::Ask);
}

fn exact_snapshot(rules: Vec<PermissionRule>) -> StorageCompletion {
    StorageCompletion::PermissionLookup(Ok(ferric_browser_storage::PermissionRulesSnapshot {
        rules,
        complete: true,
    }))
}
fn wait_ticket(app: &mut BrowserApplication, ticket: crate::StorageTicket) -> StorageCompletion {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        for effect in app.poll_storage_effects() {
            if effect.ticket == ticket {
                return effect.completion;
            }
        }
        assert!(Instant::now() < deadline, "storage completion timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn a_truncated_list_requires_exact_scope_before_using_an_allow_default() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let mut incomplete = snapshot(vec![rule(
        "https://other.example.test",
        "notifications",
        "deny",
    )]);
    if let StorageCompletion::Library(Ok(snapshot)) = &mut incomplete {
        snapshot.permissions_complete = false;
    }
    work(&app, &StorageCommand::Library { limit: 1 }).finish(&incomplete);
    assert_eq!(decision(&app), PermissionDecision::Ask);
    assert!(app.resolve_permission(ORIGIN, "geolocation").pending);
    let lookup = work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    );
    assert!(!app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    assert!(app.resolve_permission(ORIGIN, "geolocation").pending);
    lookup.finish(&exact_snapshot(vec![rule(ORIGIN, "geolocation", "deny")]));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    assert!(!app.resolve_permission(ORIGIN, "geolocation").pending);
    assert!(
        app.resolve_permission("https://absent.example.test", "geolocation")
            .pending
    );
    work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: "https://absent.example.test".into(),
        },
    )
    .finish(&exact_snapshot(vec![]));
    assert_eq!(
        app.resolve_permission("https://absent.example.test", "geolocation")
            .decision,
        PermissionDecision::Allow
    );
    assert_eq!(decision(&app), PermissionDecision::Deny);
}
#[test]
fn exact_reads_reject_wrong_scope_duplicates_and_stale_replies() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&exact_snapshot(vec![rule(
        "https://other.example.test",
        "geolocation",
        "allow",
    )]));
    assert_eq!(decision(&app), PermissionDecision::Ask);
    let stale = work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    );
    let mutation = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    stale.finish(&exact_snapshot(vec![]));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    mutation.finish(&StorageCompletion::PermissionBatch(Ok(1)));
    work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&exact_snapshot(vec![
        rule(ORIGIN, "geolocation", "allow"),
        rule(ORIGIN, "geolocation", "deny"),
    ]));
    assert_eq!(decision(&app), PermissionDecision::Deny);
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
}
#[test]
fn rejected_cancelled_and_failed_origin_reads_release_shared_loading_state() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let command = StorageCommand::PermissionLookup {
        origin: ORIGIN.into(),
    };
    let rejected = PermissionWork::prepare(Some(session(&app)), &command).unwrap();
    assert!(!app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    drop(rejected);
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    drop(work(&app, &command));
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    work(&app, &command).finish(&StorageCompletion::failed(
        StorageOperation::PermissionLookup,
        "unavailable",
    ));
    assert!(app.resolve_permission(ORIGIN, "geolocation").refresh_needed);
    assert_eq!(decision(&app), PermissionDecision::Ask);
}
#[test]
fn a_cold_owner_resolves_an_older_denial_beyond_the_library_limit() {
    let roots = Roots::new();
    let id = Uuid::new_v4();
    let mut first = app(&roots, id, PrivacyKind::Normal);
    let mut rules = vec![rule(ORIGIN, "geolocation", "deny")];
    for index in 0..1000 {
        let mut value = rule(
            &format!("https://noise-{index}.example.test"),
            "notifications",
            "deny",
        );
        value.updated_at = index + 10;
        rules.push(value);
    }
    for batch in rules.chunks(16) {
        let ticket = first
            .submit_storage(StorageCommand::PermissionBatch {
                rules: batch.to_vec(),
            })
            .unwrap();
        assert!(matches!(
            wait_ticket(&mut first, ticket),
            StorageCompletion::PermissionBatch(Ok(_))
        ));
    }
    drop(first);
    let mut cold = app(&roots, id, PrivacyKind::Normal);
    let ticket = cold
        .submit_storage(StorageCommand::Library { limit: 1000 })
        .unwrap();
    let StorageCompletion::Library(Ok(snapshot)) = wait_ticket(&mut cold, ticket) else {
        panic!("library read failed")
    };
    assert_eq!(snapshot.permissions.len(), 1000);
    assert!(!snapshot.permissions_complete);
    assert!(
        !snapshot
            .permissions
            .iter()
            .any(|rule| rule.origin == ORIGIN)
    );
    assert_eq!(decision(&cold), PermissionDecision::Ask);
    assert!(cold.resolve_permission(ORIGIN, "geolocation").pending);
    let ticket = cold
        .refresh_permission_preferences(ORIGIN)
        .unwrap()
        .unwrap();
    assert!(
        cold.refresh_permission_preferences(ORIGIN)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        wait_ticket(&mut cold, ticket),
        StorageCompletion::PermissionLookup(Ok(_))
    ));
    assert_eq!(decision(&cold), PermissionDecision::Deny);
    assert!(!cold.resolve_permission(ORIGIN, "geolocation").pending);
    let origin = "https://unused.example.test";
    let ticket = cold
        .refresh_permission_preferences(origin)
        .unwrap()
        .unwrap();
    assert!(matches!(
        wait_ticket(&mut cold, ticket),
        StorageCompletion::PermissionLookup(Ok(_))
    ));
    let allowed = cold.resolve_permission(origin, "geolocation");
    assert_eq!(allowed.decision, PermissionDecision::Allow);
    assert!(!allowed.pending);
}

#[test]
fn combined_capabilities_wait_for_saved_policy_and_deny_wins() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    assert!(
        app.resolve_permission(ORIGIN, "camera-and-microphone")
            .pending
    );
    publish(
        &app,
        vec![
            rule(ORIGIN, "camera", "allow"),
            rule(ORIGIN, "microphone", "allow"),
        ],
    );
    assert_eq!(
        app.resolve_permission(ORIGIN, "camera-and-microphone"),
        PermissionResolution::known(PermissionDecision::Allow)
    );
    let pending = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "microphone", "deny")],
        },
    );
    let value = app.resolve_permission(ORIGIN, "camera-and-microphone");
    assert_eq!(value.decision, PermissionDecision::Deny);
    assert!(!value.pending);
    assert!(!app.saved_permission_preferences(ORIGIN).complete);
    drop(pending);
}

#[test]
fn exact_site_presentation_uses_the_same_authority_and_expiry_as_requests() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    assert!(!app.saved_permission_preferences(ORIGIN).complete);
    let mut expired = rule(ORIGIN, "camera", "allow");
    expired.expires_at = Some(1);
    work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&exact_snapshot(vec![
        rule(ORIGIN, "geolocation", "deny"),
        expired,
    ]));
    let saved = app.saved_permission_preferences(ORIGIN);
    assert!(saved.complete);
    assert_eq!(saved.rules, vec![rule(ORIGIN, "geolocation", "deny")]);
    assert!(
        !app.saved_permission_preferences("https://different.example.test")
            .complete
    );
}

#[test]
fn cache_eviction_forgets_absence_before_a_default_allow_can_apply() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    assert_eq!(decision(&app), PermissionDecision::Allow);
    {
        let session = session(&app);
        let mut cache = session.0.durable.lock().unwrap();
        for index in 0..=MAX_CACHED_PERMISSION_ENTRIES {
            cache
                .ready_origins
                .insert(format!("https://site-{index}.example.test"));
        }
        cache.bound();
        assert!(cache.rules.is_empty());
        assert!(cache.ready_origins.is_empty());
    }
    let resolution = app.resolve_permission(ORIGIN, "geolocation");
    assert_eq!(resolution.decision, PermissionDecision::Ask);
    assert!(resolution.pending);
    assert!(!app.saved_permission_preferences(ORIGIN).complete);
    work(
        &app,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&exact_snapshot(vec![]));
    assert_eq!(decision(&app), PermissionDecision::Allow);
}

#[test]
fn native_url_origins_match_saved_rules_and_session_choices() {
    let roots = Roots::new();
    let mut app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let native_origin = format!("{ORIGIN}/");
    publish(&app, vec![rule(ORIGIN, "geolocation", "deny")]);
    assert_eq!(
        app.resolve_permission(&native_origin, "geolocation")
            .decision,
        PermissionDecision::Deny
    );
    publish(&app, vec![]);
    app.remember_session_permission(&native_origin, "camera", &PermissionDecision::Deny)
        .unwrap();
    assert_eq!(
        app.resolve_permission(ORIGIN, "camera").decision,
        PermissionDecision::Deny
    );
}

#[test]
fn unmatchable_origins_never_inherit_a_configured_allow() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    for origin in [
        "null",
        "file://",
        "data:text/html,example",
        "http://example.test",
        "https://blocked.example.test/path",
        "https://blocked.example.test//",
    ] {
        let resolution = app.resolve_permission(origin, "geolocation");
        assert_eq!(
            resolution,
            PermissionResolution::known(PermissionDecision::Ask),
            "{origin}"
        );
    }
}

#[test]
fn unmatchable_origins_preserve_configured_denials() {
    let roots = Roots::new();
    let mut config = Config::default();
    config.permissions.geolocation = PermissionDecision::Deny;
    let app = app_with_config(&roots, Uuid::new_v4(), PrivacyKind::Normal, config);
    for origin in ["null", "file://", "https://example.test/path"] {
        assert_eq!(
            app.resolve_permission(origin, "geolocation"),
            PermissionResolution::known(PermissionDecision::Deny)
        );
    }
}

#[test]
fn pending_denial_survives_cache_eviction() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&app, vec![]);
    let pending = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    assert_eq!(decision(&app), PermissionDecision::Deny);
    {
        let session = session(&app);
        let mut cache = session.0.durable.lock().unwrap();
        for index in 0..=MAX_CACHED_PERMISSION_ENTRIES {
            cache
                .ready_origins
                .insert(format!("https://noise-{index}.example.test"));
        }
        cache.bound();
    }
    assert_eq!(
        decision(&app),
        PermissionDecision::Deny,
        "cache eviction must retain a denial whose write is still pending"
    );
    drop(pending);
}

#[test]
fn continuous_completed_writes_cannot_pin_unbounded_cache_barriers() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let anchor = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    for index in 0..MAX_CACHED_PERMISSION_ENTRIES + 32 {
        let origin = format!("https://completed-{index}.example.test");
        work(
            &app,
            &StorageCommand::PermissionBatch {
                rules: vec![rule(&origin, "notifications", "deny")],
            },
        )
        .finish(&StorageCompletion::PermissionBatch(Ok(1)));
    }
    assert_eq!(decision(&app), PermissionDecision::Deny);
    {
        let session = session(&app);
        let cache = session.0.durable.lock().unwrap();
        assert_eq!(cache.pending_epochs.values().sum::<usize>(), 1);
        assert!(cache.barriers.len() <= MAX_CACHED_PERMISSION_ENTRIES);
        assert!(cache.capture().is_none());
    }
    anchor.finish(&StorageCompletion::PermissionBatch(Ok(1)));
    assert!(
        session(&app)
            .0
            .durable
            .lock()
            .unwrap()
            .pending_epochs
            .is_empty()
    );
}

#[test]
fn restored_completed_barriers_do_not_become_live_after_rejection() {
    let roots = Roots::new();
    let app = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let old = work(
        &app,
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "deny")],
        },
    );
    let rejected = PermissionWork::prepare(
        Some(session(&app)),
        &StorageCommand::PermissionBatch {
            rules: vec![rule(ORIGIN, "geolocation", "allow")],
        },
    )
    .unwrap();
    old.finish(&StorageCompletion::PermissionBatch(Ok(1)));
    drop(rejected);
    let session = session(&app);
    let mut cache = session.0.durable.lock().unwrap();
    assert!(cache.pending_epochs.is_empty());
    for index in 0..=MAX_CACHED_PERMISSION_ENTRIES {
        cache
            .ready_origins
            .insert(format!("https://old-{index}.example.test"));
    }
    cache.bound();
    assert!(cache.barriers.is_empty());
}

#[test]
fn normal_profile_without_storage_does_not_claim_no_saved_preferences() {
    let roots = Roots::new();
    let mut owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    owner.close_profile();
    owner
        .activate_profile(ProfileActivation {
            name: "default".into(),
            id: None,
            privacy: PrivacyKind::Normal,
            roots: Some(roots.0.clone()),
            lock: None,
            contexts: None,
            storage_path: None,
        })
        .unwrap();
    let preferences = owner.saved_permission_preferences(ORIGIN);
    assert!(
        !preferences.complete,
        "Unavailable storage cannot establish absence of saved rules"
    );
    assert_eq!(preferences.status, PermissionPreferencesStatus::Unavailable);
    assert_eq!(decision(&owner), PermissionDecision::Ask);
    assert!(!owner.resolve_permission(ORIGIN, "geolocation").pending);
}

#[test]
fn transient_profiles_have_no_durable_permission_preferences_to_load() {
    let roots = Roots::new();
    for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
        let owner = app(&roots, Uuid::new_v4(), privacy);
        let preferences = owner.saved_permission_preferences(ORIGIN);
        assert!(preferences.complete);
        assert_eq!(preferences.status, PermissionPreferencesStatus::NotStored);
        assert!(preferences.rules.is_empty());
    }
}

#[test]
fn saved_preference_availability_tracks_real_reads_and_failures() {
    let roots = Roots::new();
    let owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let status = || owner.saved_permission_preferences(ORIGIN).status;
    assert_eq!(status(), PermissionPreferencesStatus::Unchecked);
    let read = PermissionWork::prepare(
        Some(session(&owner)),
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .unwrap();
    assert_eq!(status(), PermissionPreferencesStatus::Loading);
    drop(read); // Rejected submission is not a failed storage read.
    assert_eq!(status(), PermissionPreferencesStatus::Unchecked);
    let read = work(
        &owner,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    );
    read.finish(&StorageCompletion::failed(
        StorageOperation::PermissionLookup,
        "injected read failure",
    ));
    assert_eq!(status(), PermissionPreferencesStatus::Unavailable);
    assert!(!owner.saved_permission_preferences(ORIGIN).complete);
    assert_eq!(decision(&owner), PermissionDecision::Ask);
}

#[test]
fn failed_exact_reads_back_off_and_recover_without_a_restart() {
    let roots = Roots::new();
    let mut owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    work(
        &owner,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&StorageCompletion::failed(
        StorageOperation::PermissionLookup,
        "injected read failure",
    ));
    assert!(
        owner
            .refresh_permission_preferences(ORIGIN)
            .unwrap()
            .is_none()
    );
    {
        let shared = session(&owner);
        let mut cache = shared.0.durable.lock().unwrap();
        assert!(cache.loading_origins.is_empty());
        cache.failed_reads.insert(
            ORIGIN.into(),
            Instant::now()
                .checked_sub(PERMISSION_READ_RETRY_DELAY)
                .unwrap(),
        );
    }
    assert!(
        owner
            .refresh_permission_preferences(ORIGIN)
            .unwrap()
            .is_some()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.saved_permission_preferences(ORIGIN).status != PermissionPreferencesStatus::Ready {
        assert!(
            Instant::now() < deadline,
            "healthy storage read should recover availability"
        );
        let _ = owner.poll_storage_effects();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(owner.saved_permission_preferences(ORIGIN).complete);
    assert_eq!(decision(&owner), PermissionDecision::Allow);
}

#[test]
fn failed_reads_do_not_discard_confirmed_saved_choices() {
    let roots = Roots::new();
    let owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    publish(&owner, vec![rule(ORIGIN, "geolocation", "deny")]);
    work(
        &owner,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&StorageCompletion::failed(
        StorageOperation::PermissionLookup,
        "injected read failure",
    ));
    assert_eq!(
        owner.saved_permission_preferences(ORIGIN).status,
        PermissionPreferencesStatus::Ready
    );
    assert_eq!(decision(&owner), PermissionDecision::Deny);
}

#[test]
fn failed_read_metadata_is_shared_and_bounded() {
    let roots = Roots::new();
    let owner = app(&roots, Uuid::new_v4(), PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let peer = owner.inherit_profile(runtime).unwrap();
    work(
        &owner,
        &StorageCommand::PermissionLookup {
            origin: ORIGIN.into(),
        },
    )
    .finish(&StorageCompletion::failed(
        StorageOperation::PermissionLookup,
        "injected read failure",
    ));
    assert_eq!(
        peer.saved_permission_preferences(ORIGIN).status,
        PermissionPreferencesStatus::Unavailable
    );
    for index in 0..=MAX_CACHED_PERMISSION_ENTRIES {
        let origin = format!("https://site-{index}.example.test");
        work(&owner, &StorageCommand::PermissionLookup { origin }).finish(
            &StorageCompletion::failed(StorageOperation::PermissionLookup, "injected read failure"),
        );
    }
    assert!(
        session(&owner).0.durable.lock().unwrap().failed_reads.len()
            <= MAX_CACHED_PERMISSION_ENTRIES
    );
    assert!(!peer.saved_permission_preferences(ORIGIN).complete);
}
