use super::*;
use ferric_browser_config::Config;
use ferric_browser_runtime::BrowserRuntime;
use ferric_browser_storage::RootSpec;

fn application(roots: &StorageRoots, privacy: PrivacyKind, id: Uuid) -> BrowserApplication {
    std::fs::create_dir_all(&roots.data).expect("data root");
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "permissions").expect("runtime");
    let mut application = BrowserApplication::from_runtime(runtime, Config::default());
    application
        .activate_profile(ProfileActivation {
            name: "permissions".into(),
            id: (privacy == PrivacyKind::Normal).then_some(id),
            privacy,
            roots: Some(roots.clone()),
            lock: None,
            contexts: None,
            storage_path: (privacy == PrivacyKind::Normal)
                .then(|| roots.data.join("browser.sqlite")),
        })
        .expect("profile");
    application
}
fn inherit(source: &BrowserApplication, privacy: PrivacyKind) -> BrowserApplication {
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "child").expect("runtime");
    source.inherit_profile(runtime).expect("inherit")
}

#[test]
fn inherited_windows_observe_live_decisions_and_combined_resets_for_every_privacy_kind() {
    for privacy in [
        PrivacyKind::Normal,
        PrivacyKind::Private,
        PrivacyKind::Ephemeral,
    ] {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut source = application(&roots, privacy, Uuid::new_v4());
        source
            .remember_session_permission(
                "https://EXAMPLE.test:443",
                "camera-and-microphone",
                &PermissionDecision::Allow,
            )
            .expect("grant");
        let mut child = inherit(&source, privacy);
        assert!(source.shares_permission_session(&child));
        assert_eq!(
            child.session_permission("https://example.test", "camera"),
            Some(PermissionDecision::Allow)
        );
        assert_eq!(
            child.session_permission("https://example.test", "microphone"),
            Some(PermissionDecision::Allow)
        );
        source
            .remember_session_permission(
                "https://example.test",
                "camera",
                &PermissionDecision::Deny,
            )
            .expect("deny");
        assert_eq!(
            child.session_permission("https://example.test", "camera"),
            Some(PermissionDecision::Deny)
        );
        assert!(
            child
                .reset_session_permission("https://example.test", "camera-and-microphone")
                .expect("reset")
        );
        for permission in ["camera", "microphone"] {
            assert_eq!(
                source.session_permission("https://example.test", permission),
                None
            );
        }
        child
            .remember_session_permission(
                "https://example.test",
                "clipboard",
                &PermissionDecision::Allow,
            )
            .expect("clipboard");
        source.close_profile();
        assert_eq!(
            child.session_permission("https://example.test", "clipboard"),
            Some(PermissionDecision::Allow)
        );
        assert!(!source.shares_permission_session(&child));
        child.close_profile();
        roots.cleanup().expect("cleanup");
    }
}

#[test]
fn durable_session_identity_includes_roots_and_private_sessions_are_independent() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
    let unrelated_roots = StorageRoots::resolve(RootSpec::Temporary).expect("other roots");
    let id = Uuid::new_v4();
    let mut source = application(&roots, PrivacyKind::Normal, id);
    let mut existing = application(&roots, PrivacyKind::Normal, id);
    let mut other_id = application(&roots, PrivacyKind::Normal, Uuid::new_v4());
    let mut other_roots = application(&unrelated_roots, PrivacyKind::Normal, id);
    let mut private_one = application(&roots, PrivacyKind::Private, id);
    let mut private_two = application(&roots, PrivacyKind::Private, id);
    source
        .remember_session_permission(
            "https://example.test",
            "geolocation",
            &PermissionDecision::Allow,
        )
        .expect("grant");
    assert!(existing.shares_permission_session(&source));
    assert_eq!(
        existing.session_permission("https://example.test", "geolocation"),
        Some(PermissionDecision::Allow)
    );
    for unrelated in [&other_id, &other_roots, &private_one, &private_two] {
        assert!(!unrelated.shares_permission_session(&source));
        assert_eq!(
            unrelated.session_permission("https://example.test", "geolocation"),
            None
        );
    }
    private_one
        .remember_session_permission(
            "https://example.test",
            "geolocation",
            &PermissionDecision::Deny,
        )
        .expect("private grant");
    assert!(!private_one.shares_permission_session(&private_two));
    assert_eq!(
        private_two.session_permission("https://example.test", "geolocation"),
        None
    );
    source.close_profile();
    existing.close_profile();
    other_id.close_profile();
    other_roots.close_profile();
    private_one.close_profile();
    private_two.close_profile();
    // The registry retains only weak references, so a genuinely new profile
    // session cannot recover a grant after the last application owner leaves.
    let mut reopened = application(&roots, PrivacyKind::Normal, id);
    assert_eq!(
        reopened.session_permission("https://example.test", "geolocation"),
        None
    );
    reopened.close_profile();
    roots.cleanup().expect("cleanup");
    unrelated_roots.cleanup().expect("cleanup other");
}

#[test]
fn remembered_permissions_validate_inputs_and_never_cache_capture_consent() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
    let mut source = application(&roots, PrivacyKind::Private, Uuid::new_v4());
    for permission in ["screen-capture", "unknown"] {
        assert!(
            source
                .remember_session_permission(
                    "https://example.test",
                    permission,
                    &PermissionDecision::Allow
                )
                .is_err()
        );
        assert!(
            source
                .reset_session_permission("https://example.test", permission)
                .is_err()
        );
        assert_eq!(
            source.session_permission("https://example.test", permission),
            None
        );
    }
    for origin in [
        "http://example.test",
        "https://example.test/path",
        "https://name@example.test",
    ] {
        assert!(
            source
                .remember_session_permission(origin, "camera", &PermissionDecision::Allow)
                .is_err()
        );
    }
    assert!(
        source
            .remember_session_permission("https://example.test", "camera", &PermissionDecision::Ask)
            .is_err()
    );
    source
        .remember_session_permission(
            "https://example.test",
            "camera-and-microphone",
            &PermissionDecision::Allow,
        )
        .expect("grant both");
    assert!(
        source
            .reset_session_permission("https://example.test", "camera")
            .expect("reset camera")
    );
    assert_eq!(
        source.session_permission("https://example.test", "camera"),
        None
    );
    assert_eq!(
        source.session_permission("https://example.test", "microphone"),
        Some(PermissionDecision::Allow)
    );
    assert!(
        source
            .reset_session_permission("https://example.test", "camera-and-microphone")
            .expect("reset remainder")
    );
    assert!(
        !source
            .reset_session_permission("https://example.test", "camera-and-microphone")
            .expect("repeat reset")
    );
    source
        .remember_session_permission(
            "https://example.test",
            "local-fonts",
            &PermissionDecision::Deny,
        )
        .expect("fonts");
    assert!(!format!("{:?}", source.active_profile).contains("example.test"));
    source.close_profile();
    assert!(
        source
            .remember_session_permission(
                "https://example.test",
                "camera",
                &PermissionDecision::Allow
            )
            .is_err()
    );
    roots.cleanup().expect("cleanup");
}
