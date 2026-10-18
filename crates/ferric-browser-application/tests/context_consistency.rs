use ferric_browser_application::{
    BrowserApplication, ContextCommand, ProfileOpenRequest, ProfileStorage,
};
use ferric_browser_config::{Config, ContextDefinition, ContextsConfig, RuntimeOverrides};
use ferric_browser_core::{Event, PrivacyKind};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};
use ferric_browser_storage::{ContextMember, ContextRegistryError, RootSpec, StorageRoots};
use std::time::{Duration, Instant};

struct TestRoots(StorageRoots);
impl TestRoots {
    fn new() -> Self {
        Self(StorageRoots::resolve(RootSpec::Temporary).unwrap())
    }
}
impl Drop for TestRoots {
    fn drop(&mut self) {
        self.0.clone().cleanup().unwrap();
    }
}
fn work() -> ContextsConfig {
    ContextsConfig {
        schema_version: 3,
        contexts: vec![ContextDefinition {
            name: "work".into(),
            label: "Work".into(),
            profile: "default".into(),
            sessions: vec![],
            workspace: None,
            accent: None,
            default_target: "reuse".into(),
        }],
        routes: vec![],
    }
}
fn open(roots: &TestRoots, contexts: ContextsConfig) -> BrowserApplication {
    let (mut app, _, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default").unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "default".into(),
        label: "Default".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(RootSpec::Base(
            roots.0.temporary_root().unwrap().to_owned(),
        )),
        base_configuration: Config::default(),
        initial_profile_overrides: RuntimeOverrides::default(),
        command_line_overrides: RuntimeOverrides::default(),
        config_path: None,
        contexts,
        context_configuration_error: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = app.poll_profile_open() {
            let effect = result.unwrap();
            assert!(effect.storage_error.is_none(), "{:?}", effect.storage_error);
            assert!(effect.context_error.is_none(), "{:?}", effect.context_error);
            return app;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn create_manual(app: &mut BrowserApplication) {
    app.change_context(ContextCommand::Create {
        name: "manual".into(),
        label: "Manual".into(),
        profile: "default".into(),
        workspace: None,
    })
    .unwrap();
}
#[test]
fn removal_while_closed_removes_only_configured_contexts() -> Result<(), String> {
    let roots = TestRoots::new();
    let mut app = open(&roots, work());
    create_manual(&mut app);
    drop(app);
    let app = open(&roots, ContextsConfig::default());
    assert!(app.contexts().unwrap().iter().any(|c| c.name == "manual"));
    if app.contexts().unwrap().iter().any(|c| c.name == "work") {
        return Err("An offline deletion remains offered after restart".into());
    }
    Ok(())
}
#[test]
fn delayed_reload_result_cannot_replace_a_newer_catalog() -> Result<(), String> {
    let roots = TestRoots::new();
    let mut app = open(&roots, work());
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let mut other = app.inherit_profile(runtime).unwrap();
    let mut next = work();
    next.contexts[0].label = "After work".into();
    let effect = app
        .prepare_context_reload(&work(), next)
        .unwrap()
        .unwrap()
        .run()
        .unwrap();
    create_manual(&mut other);
    assert!(
        app.refresh_context_catalog(&other.context_catalog().unwrap())
            .is_some()
    );
    assert!(app.contexts().unwrap().iter().any(|c| c.name == "manual"));
    let _ = app.apply_context_reload(effect);
    if !app.contexts().unwrap().iter().any(|c| c.name == "manual") {
        return Err("A late worker reply erased a newer committed context choice".into());
    }
    Ok(())
}

#[test]
fn removing_a_definition_clears_the_logical_window_context() -> Result<(), String> {
    let roots = TestRoots::new();
    let mut app = open(&roots, work());
    let window = app.active_window().unwrap();
    app.dispatch_runtime(RuntimeInput::UserDecision(Event::SetWindowContext {
        window,
        context: Some("work".into()),
    }))
    .unwrap();
    let effect = app
        .prepare_context_reload(&work(), ContextsConfig::default())
        .unwrap()
        .unwrap()
        .run()
        .unwrap();
    app.apply_context_reload(effect).unwrap();
    assert!(app.contexts().unwrap().is_empty());
    if app.windows()[&window].context.is_some() {
        return Err("The logical window still claims membership in a deleted context".into());
    }
    Ok(())
}

#[test]
fn command_deletion_clears_membership_and_emits_checkpoint_effects() {
    let roots = TestRoots::new();
    let mut app = open(&roots, work());
    create_manual(&mut app);
    let window = app.active_window().unwrap();
    app.dispatch_runtime(RuntimeInput::UserDecision(Event::SetWindowContext {
        window,
        context: Some("manual".into()),
    }))
    .unwrap();
    let effect = app
        .change_context(ContextCommand::Remove {
            name: "manual".into(),
        })
        .unwrap();
    assert_eq!(app.windows()[&window].context, None);
    assert!(
        app.contexts()
            .unwrap()
            .iter()
            .any(|context| context.name == "work")
    );
    assert!(
        !app.contexts()
            .unwrap()
            .iter()
            .any(|context| context.name == "manual")
    );
    assert!(
        effect
            .runtime_effects
            .iter()
            .any(|effect| matches!(effect, ferric_browser_runtime::RuntimeEffect::Storage(_)))
    );
    assert!(effect.runtime_effects.iter().any(|effect| matches!(
        effect,
        ferric_browser_runtime::RuntimeEffect::AppliedEvent(Event::ClearWindowContexts { .. })
    )));
}

#[test]
fn configured_deletion_keeps_the_window_binding_and_saved_membership() {
    let roots = TestRoots::new();
    let mut app = open(&roots, work());
    let window = app.active_window().unwrap();
    app.dispatch_runtime(RuntimeInput::UserDecision(Event::SetWindowContext {
        window,
        context: Some("work".into()),
    }))
    .unwrap();
    let current = &app.windows()[&window];
    let members = vec![ContextMember {
        window_id: window.to_string(),
        tabs: current.tabs.iter().map(ToString::to_string).collect(),
        selected_tab: current.active_tab.map(|tab| tab.to_string()),
        tab_descriptors: vec![],
        selected_index: None,
    }];
    app.change_context(ContextCommand::SaveMembership {
        name: "work".into(),
        members,
    })
    .unwrap();
    let before = app.contexts().unwrap().to_vec();
    let revision = app.revision();
    let profile = app.profile_snapshot();
    assert!(matches!(
        app.change_context(ContextCommand::Remove {
            name: "work".into()
        }),
        Err(ContextRegistryError::DefinedInConfiguration)
    ));
    assert_eq!(app.contexts().unwrap(), before);
    assert_eq!(app.windows()[&window].context.as_deref(), Some("work"));
    assert_eq!(app.revision(), revision);
    assert_eq!(app.profile_snapshot(), profile);
}
