use ferric_browser_application::{
    BrowserApplication, ConfigurationLayers, InteractionRuleEdit, ProfileOpenRequest,
    ProfileStorage,
};
use ferric_browser_config::{
    Config, ContextsConfig, InteractionPolicyPatch, MAX_SITE_RULES, PolicySourceIdentity, SiteRule,
};
use ferric_browser_core::PrivacyKind;
use ferric_browser_runtime::BrowserRuntime;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const PATTERN: &str = "https://elsewhere.test/*";
fn patch(raw: &str) -> InteractionPolicyPatch {
    toml::from_str(raw).unwrap()
}
fn rule(id: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: PATTERN.into(),
        priority: -7,
        set: BTreeMap::new(),
        interaction: Some(patch("enabled=false\nhint_page_keys=['p']")),
    }
}
fn open(layers: ConfigurationLayers) -> BrowserApplication {
    let (mut app, _, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "work").unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "work".into(),
        label: "Work".into(),
        privacy: PrivacyKind::Private,
        storage: ProfileStorage::Transient,
        base_configuration: layers.base.clone(),
        base_authored: layers.base_authored.clone(),
        initial_profile_overrides: layers.profile.clone(),
        command_line_overrides: layers.command_line.clone(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = app.poll_profile_open() {
            assert!(result.unwrap().configuration_error.is_none());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    app.update_configuration(layers.clone()).unwrap();
    app.activate_configuration(layers).unwrap();
    app
}
fn create(id: &str) -> InteractionRuleEdit {
    InteractionRuleEdit::Create {
        id: id.into(),
        pattern: PATTERN.into(),
        priority: -7,
        interaction: patch("enabled=true"),
        acknowledge_lower_replacement: false,
    }
}
fn replace(id: &str) -> InteractionRuleEdit {
    InteractionRuleEdit::Replace {
        id: id.into(),
        expected_pattern: PATTERN.into(),
        interaction: patch("[labels]\nmin_chars=2"),
    }
}
fn unset(id: &str) -> InteractionRuleEdit {
    InteractionRuleEdit::Unset {
        id: id.into(),
        expected_pattern: PATTERN.into(),
    }
}

#[path = "interaction_edits/interchange.rs"]
mod interchange;

fn draft_edit(id: &str) -> ferric_browser_core::SitePolicyEdit {
    ferric_browser_core::SitePolicyEdit::Rule {
        target: ferric_browser_core::SitePolicyRuleRef {
            source: ferric_browser_core::SitePolicySource::Generated,
            id: id.into(),
        },
        new: false,
        pattern: None,
        priority: None,
        remove_interaction: false,
        enabled: None,
    }
}

static PREVIEW_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn capture_keeps_the_retained_preview_and_configuration_unchanged() {
    let _guard = PREVIEW_TEST_LOCK.lock().unwrap();
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("capture-rule"));
    let app = open(layers);
    let draft = app
        .open_site_policy_draft(&draft_edit("capture-rule"))
        .unwrap();
    let before = app.configuration_snapshot();
    let preview = app
        .prepare_site_policy_preview(&draft.id, draft.revision, None)
        .unwrap();
    let capture = app
        .prepare_site_policy_capture(&draft.id, draft.revision, "https://elsewhere.test/page")
        .unwrap();
    assert!(capture.is_current(&app));
    assert_eq!(
        app.site_policy_draft(&draft.id)
            .unwrap()
            .preview
            .unwrap()
            .id,
        preview.receipt().id
    );
    assert_eq!(app.configuration_snapshot(), before);
}

#[test]
fn preview_uses_the_prospective_compiler_without_publication_and_has_bounded_lifetimes() {
    let _guard = PREVIEW_TEST_LOCK.lock().unwrap();
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("preview-rule"));
    let app = open(layers);
    let opened = app
        .open_site_policy_draft(&draft_edit("preview-rule"))
        .unwrap();
    let before = app.configuration_snapshot();
    let mut proposal = opened.rules[0].proposed.clone().unwrap();
    proposal.interaction = Some(patch("enabled=true\n[labels]\nauto_follow='full-match'"));
    let edited = app
        .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposal)])
        .unwrap();
    let preview = app
        .prepare_site_policy_preview(
            &edited.id,
            edited.revision,
            Some("https://elsewhere.test/page"),
        )
        .unwrap();
    assert!(preview.receipt().valid);
    assert_eq!(preview.receipt().kind, "static-only");
    assert!(!preview.receipt().activation_permitted);
    assert!(preview.is_current(&app));
    assert_eq!(
        preview.receipt().candidate_revision,
        Some(preview.snapshot().unwrap().sources().revision())
    );
    let policy = preview
        .snapshot()
        .unwrap()
        .interaction()
        .unwrap()
        .policy()
        .unwrap();
    assert!(policy.enabled());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(
        app.configuration_inspection_inputs()
            .sources()
            .reload_layers()
            .runtime
            .site_rules[0]
            .interaction
            .as_ref()
            .unwrap()
            .enabled,
        Some(false)
    );
    assert!(
        app.site_policy_draft(&edited.id)
            .unwrap()
            .operation
            .is_none()
    );
    let mut previews = vec![preview];
    for _ in 1..ferric_browser_application::MAX_SITE_POLICY_PREVIEWS {
        let static_preview = app
            .prepare_site_policy_preview(&edited.id, edited.revision, None)
            .unwrap();
        assert!(static_preview.snapshot().is_none());
        previews.push(static_preview);
    }
    assert!(
        app.prepare_site_policy_preview(&edited.id, edited.revision, None)
            .err()
            .unwrap()
            .contains("eight")
    );
    previews.pop();
    let replacement = app
        .prepare_site_policy_preview(&edited.id, edited.revision, None)
        .unwrap();
    let mut bad = edited.rules[0].proposed.clone().unwrap();
    bad.interaction = Some(patch("enabled=true\n[labels]\nalphabet='a'"));
    let invalid = app
        .update_site_policy_draft(&edited.id, edited.revision, vec![Some(bad)])
        .unwrap();
    assert!(!replacement.is_current(&app));
    assert!(previews.iter().all(|preview| !preview.is_current(&app)));
    drop(replacement);
    drop(previews);
    assert!(
        app.prepare_site_policy_preview(&edited.id, edited.revision, None)
            .is_err()
    );
    let failed = app
        .prepare_site_policy_preview(
            &invalid.id,
            invalid.revision,
            Some("https://elsewhere.test/page"),
        )
        .unwrap();
    assert!(!failed.receipt().valid);
    assert!(failed.snapshot().is_none());
    assert!(!failed.receipt().message.is_empty());
    assert!(failed.is_current(&app));
    assert_eq!(app.configuration_snapshot(), before);
    app.discard_site_policy_draft(&invalid.id).unwrap();
    assert!(!failed.is_current(&app));
}

#[test]
fn draft_retarget_is_nonactivating_and_preserves_the_full_ordinary_snapshot() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut original = rule("draft-rule");
    original
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    layers.runtime.site_rules.push(original.clone());
    let app = open(layers);
    let before = app.configuration_snapshot();
    let opened = app
        .open_site_policy_draft(&draft_edit("draft-rule"))
        .unwrap();
    assert!(!opened.dirty);
    let mut proposed = opened.rules[0].proposed.clone().unwrap();
    proposed.pattern = "https://changed.test/*".into();
    proposed.priority = 9;
    proposed.interaction.as_mut().unwrap().enabled = Some(true);
    let changed = app
        .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed.clone())])
        .unwrap();
    assert!(changed.dirty);
    assert!(changed.dirty_fields.contains(&"draft-rule.pattern".into()));
    assert!(
        app.update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed.clone())])
            .is_err()
    );
    let valid = app
        .validate_site_policy_draft(&opened.id, changed.revision)
        .unwrap();
    assert!(valid.validation.unwrap().valid);
    let prepared = app
        .prepare_site_policy_draft(&opened.id, changed.revision)
        .unwrap();
    assert_eq!(prepared.changes()[0].original.as_ref(), Some(&original));
    assert_eq!(prepared.changes()[0].proposed.as_ref(), Some(&proposed));
    assert_eq!(app.configuration_snapshot(), before);
    proposed
        .set
        .insert("content.zoom".into(), toml::Value::Float(2.0));
    assert!(
        app.update_site_policy_draft(&opened.id, changed.revision, vec![Some(proposed)])
            .is_err()
    );
    assert!(
        app.update_site_policy_draft(&opened.id, changed.revision, vec![None])
            .is_err()
    );
    assert_eq!(
        app.site_policy_draft(&opened.id).unwrap().revision,
        changed.revision
    );
}

#[test]
fn draft_validation_failure_keeps_edits_and_never_adopts_a_new_config_capture() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("draft-rule"));
    let mut app = open(layers.clone());
    let opened = app
        .open_site_policy_draft(&draft_edit("draft-rule"))
        .unwrap();
    let mut proposed = opened.rules[0].proposed.clone().unwrap();
    proposed.priority = 5;
    let changed = app
        .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed.clone())])
        .unwrap();
    layers.temporary.site_rules.push(rule("concurrent"));
    app.update_configuration(layers).unwrap();
    let valid = app
        .validate_site_policy_draft(&opened.id, changed.revision)
        .unwrap();
    assert!(!valid.validation.unwrap().valid);
    assert!(valid.dirty);
    assert_eq!(valid.rules[0].proposed.as_ref(), Some(&proposed));
    assert!(
        app.prepare_site_policy_draft(&opened.id, changed.revision)
            .is_err()
    );
    let rebased = app
        .rebase_site_policy_draft(&opened.id, changed.revision)
        .unwrap();
    assert!(rebased.revision > changed.revision);
    assert!(rebased.rebased_from.is_some());
    assert_eq!(rebased.rules[0].proposed.as_ref(), Some(&proposed));
    assert!(
        app.rebase_site_policy_draft(&opened.id, changed.revision)
            .is_err()
    );
    assert!(
        app.validate_site_policy_draft(&opened.id, rebased.revision)
            .unwrap()
            .validation
            .unwrap()
            .valid
    );
    let mut removed = rebased.rules[0].proposed.clone().unwrap();
    removed.interaction = None;
    let removed = app
        .update_site_policy_draft(&opened.id, rebased.revision, vec![Some(removed)])
        .unwrap();
    assert!(removed.rules[0].proposed.is_none());
    assert!(
        app.prepare_site_policy_draft(&opened.id, removed.revision)
            .unwrap()
            .changes()[0]
            .proposed
            .is_none()
    );
}

#[test]
fn explicit_rebase_preserves_local_fields_and_adopts_concurrent_settings() {
    for local_list in [false, true] {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.runtime.site_rules.push(rule("draft-rule"));
        let mut app = open(layers.clone());
        let opened = app
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .unwrap();
        let mut proposed = opened.rules[0].proposed.clone().unwrap();
        proposed.priority = 8;
        proposed.interaction.as_mut().unwrap().enabled = Some(true);
        if local_list {
            proposed.interaction.as_mut().unwrap().hint_page_keys = Some(vec!["l".into()]);
        }
        let changed = app
            .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed)])
            .unwrap();
        let current = &mut layers.runtime.site_rules[0];
        current
            .set
            .insert("content.zoom".into(), toml::Value::Float(1.75));
        current.interaction.as_mut().unwrap().hint_page_keys = Some(vec!["x".into()]);
        app.update_configuration(layers).unwrap();
        let before = app.configuration_snapshot();
        let rebased = app
            .rebase_site_policy_draft(&opened.id, changed.revision)
            .unwrap();
        let proposed = rebased.rules[0].proposed.as_ref().unwrap();
        assert_eq!(proposed.priority, 8);
        assert_eq!(
            proposed.set.get("content.zoom"),
            Some(&toml::Value::Float(1.75))
        );
        let patch = proposed.interaction.as_ref().unwrap();
        assert_eq!(patch.enabled, Some(true));
        assert_eq!(
            patch.hint_page_keys.as_ref().unwrap(),
            &vec![if local_list {
                "l".to_owned()
            } else {
                "x".to_owned()
            }]
        );
        assert!(
            app.prepare_site_policy_draft(&opened.id, rebased.revision)
                .is_ok()
        );
        assert_eq!(app.configuration_snapshot(), before);
    }
}

#[test]
fn profile_draft_cap_is_shared_but_windows_and_retirement_keep_their_own_drafts() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("draft-rule"));
    let mut app = open(layers);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Private, "child").unwrap();
    let child = app.inherit_profile(runtime).unwrap();
    let first = app
        .open_site_policy_draft(&draft_edit("draft-rule"))
        .unwrap();
    assert!(child.site_policy_draft(&first.id).is_err());
    assert!(child.discard_site_policy_draft(&first.id).is_err());
    for _ in 1..8 {
        app.open_site_policy_draft(&draft_edit("draft-rule"))
            .unwrap();
    }
    for _ in 0..8 {
        child
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .unwrap();
    }
    assert!(
        app.open_site_policy_draft(&draft_edit("draft-rule"))
            .is_err()
    );
    assert!(
        child
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .is_err()
    );
    app.close_profile();
    for _ in 0..8 {
        child
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .unwrap();
    }
    assert!(
        child
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .is_err()
    );
}

#[test]
fn reviewed_lower_copy_requires_acknowledgement_and_preserves_ordinary_settings() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut original = rule("copied-rule");
    original
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    layers.base.site_rules.push(original.clone());
    let app = open(layers);
    let capture = app.capture_interaction_edits().unwrap();
    let mut proposed = original.clone();
    proposed.pattern = "https://retarget.test/*".into();
    proposed.priority = 4;
    let operation = |proposed, acknowledge_lower_replacement| InteractionRuleEdit::Reviewed {
        id: original.id.clone(),
        expected_pattern: original.pattern.clone(),
        proposed: Some(proposed),
        acknowledge_lower_replacement,
    };
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[operation(proposed.clone(), false)]
        )
        .is_err()
    );
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[operation(proposed.clone(), true)],
        )
        .unwrap();
    assert_eq!(prepared.changes()[0].seed.as_ref().unwrap().rule, original);
    assert_eq!(prepared.changes()[0].proposed.as_ref(), Some(&proposed));
    proposed.set.clear();
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[operation(proposed, true)]
        )
        .is_err()
    );
}

#[test]
fn draft_apply_reserves_one_request_and_prewrite_failure_keeps_newer_edits() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("draft-rule"));
    let app = open(layers);
    let opened = app
        .open_site_policy_draft(&draft_edit("draft-rule"))
        .unwrap();
    let (prepared, observer) = app
        .begin_site_policy_draft_apply(&opened.id, opened.revision, false)
        .unwrap();
    assert!(observer.is_current(&app));
    assert!(
        app.begin_site_policy_draft_apply(&opened.id, opened.revision, false)
            .is_err()
    );
    let mut proposed = opened.rules[0].proposed.clone().unwrap();
    proposed.priority = 9;
    let changed = app
        .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed.clone())])
        .unwrap();
    assert!(!observer.is_current(&app));
    drop((prepared, observer));
    let retained = app.site_policy_draft(&opened.id).unwrap();
    assert_eq!(retained.revision, changed.revision);
    assert_eq!(retained.rules[0].proposed.as_ref(), Some(&proposed));
    assert_eq!(
        serde_json::to_value(retained.operation).unwrap()["state"],
        "failed"
    );
    let (_, observer) = app
        .begin_site_policy_draft_apply(&opened.id, changed.revision, false)
        .unwrap();
    app.discard_site_policy_draft(&opened.id).unwrap();
    assert!(!observer.is_current(&app));
}

#[test]
fn older_admitted_publication_does_not_overwrite_a_newer_draft() {
    use ferric_browser_application::SitePolicyApplyOutcome;
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(rule("draft-rule"));
    let mut app = open(layers);
    let opened = app
        .open_site_policy_draft(&draft_edit("draft-rule"))
        .unwrap();
    let mut proposed = opened.rules[0].proposed.clone().unwrap();
    proposed.priority = 3;
    let reviewed = app
        .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed.clone())])
        .unwrap();
    let (prepared, mut observer) = app
        .begin_site_policy_draft_apply(&opened.id, reviewed.revision, false)
        .unwrap();
    observer.admitted();
    assert!(app.discard_site_policy_draft(&opened.id).is_err());
    proposed.priority = 7;
    let newer = app
        .update_site_policy_draft(&opened.id, reviewed.revision, vec![Some(proposed.clone())])
        .unwrap();
    app.publish_configuration_update(prepared.into_prepared(), || Ok(()))
        .unwrap();
    assert!(
        observer
            .finish(
                Some(&app),
                SitePolicyApplyOutcome::Published {
                    durability_uncertain: false
                }
            )
            .is_none()
    );
    let retained = app.site_policy_draft(&opened.id).unwrap();
    assert_eq!(retained.revision, newer.revision);
    assert_eq!(retained.rules[0].proposed.as_ref(), Some(&proposed));
    assert!(retained.dirty);
    assert!(
        app.prepare_site_policy_draft(&opened.id, newer.revision)
            .is_err()
    );
    let rebased = app
        .rebase_site_policy_draft(&opened.id, newer.revision)
        .unwrap();
    assert_eq!(rebased.rules[0].proposed.as_ref().unwrap().priority, 7);
}

#[test]
fn session_draft_apply_separates_publication_from_explicit_activation() {
    use ferric_browser_application::SitePolicyApplyOutcome;
    for activate in [false, true] {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.runtime.site_rules.push(rule("draft-rule"));
        let mut app = open(layers);
        let opened = app
            .open_site_policy_draft(&draft_edit("draft-rule"))
            .unwrap();
        let mut proposed = opened.rules[0].proposed.clone().unwrap();
        proposed.priority = 6;
        let reviewed = app
            .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed)])
            .unwrap();
        let (prepared, observer) = app
            .begin_site_policy_draft_apply(&opened.id, reviewed.revision, activate)
            .unwrap();
        assert!(!observer.persist());
        app.publish_configuration_update(prepared.into_prepared(), || Ok(()))
            .unwrap();
        assert_eq!(app.configuration().site_rules[0].priority, -7);
        let activation = observer.finish(
            Some(&app),
            SitePolicyApplyOutcome::Published {
                durability_uncertain: false,
            },
        );
        assert_eq!(activation.is_some(), activate);
        if let Some(activation) = activation {
            let prepared = activation.prepare_activation(&app).unwrap();
            app.publish_configuration_update(prepared, || Ok(()))
                .unwrap();
            activation.finish_activation(Some(&app), true);
            assert_eq!(app.configuration().site_rules[0].priority, 6);
        }
        let retained = app.site_policy_draft(&opened.id).unwrap();
        assert!(!retained.dirty);
        assert_eq!(retained.operation.as_ref().unwrap().saved, Some(false));
        assert_eq!(retained.operation.as_ref().unwrap().active, activate);
        assert!(
            app.prepare_site_policy_draft(&opened.id, reviewed.revision)
                .is_ok()
        );
    }
}

#[test]
fn exact_create_requires_acknowledgement_and_preserves_the_lower_full_snapshot() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut lower = rule("same");
    lower
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    layers.base.site_rules.push(lower.clone());
    let mut custom = rule("custom");
    custom
        .set
        .insert("content.images".into(), toml::Value::Boolean(false));
    layers.runtime.site_rules.push(custom.clone());
    let mut higher = rule("same");
    higher.pattern = "https://never.test/*".into();
    layers.temporary.site_rules.push(higher.clone());
    let app = open(layers);
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let capture = app.capture_interaction_edits().unwrap();
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[create("same")]
        )
        .is_err()
    );
    let mut operation = create("same");
    if let InteractionRuleEdit::Create {
        acknowledge_lower_replacement,
        ..
    } = &mut operation
    {
        *acknowledge_lower_replacement = true;
    }
    let prepared = app
        .prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[operation])
        .unwrap();
    assert!(prepared.changed());
    let desired = prepared.prepared().policy_sources();
    assert_eq!(desired.desired_layers().runtime.site_rules[0], custom);
    let created = &desired.desired_layers().runtime.site_rules[1];
    let mut expected = lower.clone();
    expected.interaction = Some(patch("enabled=true"));
    assert_eq!(created, &expected);
    assert_eq!(prepared.changes()[0].original, None);
    assert_eq!(prepared.changes()[0].proposed, Some(expected));
    let seed = prepared.changes()[0].seed.as_ref().unwrap();
    assert_eq!(seed.rule, lower);
    assert_eq!(seed.origin.source(), &PolicySourceIdentity::InMemory);
    let winner = prepared.changes()[0].desired_winner.as_ref().unwrap();
    assert_eq!(winner.rule, higher);
    assert_eq!(winner.origin.source(), &PolicySourceIdentity::Temporary);
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn lower_mixed_purpose_envelope_changes_and_exact_id_pattern_collisions_reject() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut lower = rule("lower");
    lower
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    layers.base.site_rules.push(lower);
    layers.runtime.site_rules.push(rule("existing"));
    let app = open(layers);
    let capture = app.capture_interaction_edits().unwrap();
    let before = app.configuration_policy_sources();
    for change_pattern in [false, true] {
        let mut op = create("lower");
        if let InteractionRuleEdit::Create {
            acknowledge_lower_replacement,
            pattern,
            priority,
            ..
        } = &mut op
        {
            *acknowledge_lower_replacement = true;
            if change_pattern {
                *pattern = "https://retargeted.test/*".into();
            } else {
                *priority = 99;
            }
        }
        assert!(
            app.prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[op])
                .is_err()
        );
    }
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[create("existing")]
        )
        .is_err()
    );
    let wrong = InteractionRuleEdit::Replace {
        id: "existing".into(),
        expected_pattern: "https://other.test/*".into(),
        interaction: patch("enabled=true"),
    };
    assert!(
        app.prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[wrong])
            .is_err()
    );
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[unset("missing")]
        )
        .is_err()
    );
    assert_eq!(app.configuration_policy_sources(), before);
}

#[test]
fn replace_unset_and_pruning_preserve_ordinary_settings_and_show_the_desired_winner() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut mixed = rule("mixed");
    mixed
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    let mut lower = rule("only");
    lower
        .set
        .insert("content.images".into(), toml::Value::Boolean(false));
    layers.base.site_rules.push(lower.clone());
    layers.runtime.site_rules = vec![mixed.clone(), rule("neighbor"), rule("only")];
    let mut app = open(layers);
    let capture = app.capture_interaction_edits().unwrap();
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[replace("mixed")],
        )
        .unwrap();
    let mut expected = mixed;
    expected.interaction = Some(patch("[labels]\nmin_chars=2"));
    assert_eq!(prepared.changes()[0].proposed, Some(expected));
    app.publish_configuration_update(prepared.into_prepared(), || Ok(()))
        .unwrap();
    let original = app.configuration_layers().runtime.clone();
    let capture = app.capture_interaction_edits().unwrap();
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[unset("mixed"), unset("only")],
        )
        .unwrap();
    let desired = prepared.prepared().policy_sources();
    let rules = &desired.desired_layers().runtime.site_rules;
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].id, "mixed");
    assert_eq!(rules[0].interaction, None);
    assert_eq!(rules[0].set, original.site_rules[0].set);
    assert_eq!(rules[0].priority, -7);
    assert_eq!(rules[1], original.site_rules[1]);
    assert_eq!(prepared.changes()[1].proposed, None);
    assert_eq!(
        prepared.changes()[1].desired_winner.as_ref().unwrap().rule,
        lower
    );
    assert_eq!(app.configuration_layers().runtime, original);
}

#[test]
fn atomic_batch_validates_final_capacity_and_preserves_existing_order() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules = (0..MAX_SITE_RULES)
        .map(|i| rule(&format!("old-{i}")))
        .collect();
    let app = open(layers);
    let capture = app.capture_interaction_edits().unwrap();
    let before = app.configuration_policy_sources();
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[create("new"), unset("old-255")],
        )
        .unwrap();
    let desired = prepared.prepared().policy_sources();
    let rules = &desired.desired_layers().runtime.site_rules;
    assert_eq!(rules.len(), MAX_SITE_RULES);
    assert_eq!(
        &rules[..255],
        &before.desired_layers().runtime.site_rules[..255]
    );
    assert_eq!(rules[255].id, "new");
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[unset("old-0"), replace("old-0")]
        )
        .is_err()
    );
    let mut invalid = create("invalid");
    if let InteractionRuleEdit::Create { pattern, .. } = &mut invalid {
        *pattern = "file:///tmp/*".into();
    }
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[unset("old-0"), invalid]
        )
        .is_err()
    );
    assert!(
        app.prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[])
            .is_err()
    );
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &vec![unset("old-0"); 257]
        )
        .is_err()
    );
    assert_eq!(app.configuration_policy_sources(), before);
}

#[test]
fn capture_requires_a_real_owner_and_rejects_foreign_local_stale_and_reload_races() {
    assert!(
        BrowserApplication::new(Config::default())
            .capture_interaction_edits()
            .is_err()
    );
    let mut app = open(ConfigurationLayers::new(Config::default()));
    let other = open(ConfigurationLayers::new(Config::default()));
    assert_eq!(app.configuration_revision(), other.configuration_revision());
    let foreign = other.capture_interaction_edits().unwrap();
    assert!(
        app.prepare_interaction_edits(&foreign, &PolicySourceIdentity::Temporary, &[create("new")])
            .is_err()
    );
    let capture = app.capture_interaction_edits().unwrap();
    let prepared = app
        .prepare_interaction_edits(&capture, &PolicySourceIdentity::Temporary, &[create("new")])
        .unwrap();
    let before = app.configuration_snapshot();
    let reservation = app.reserve_configuration_reload().unwrap();
    let wrote = std::cell::Cell::new(false);
    assert!(
        app.publish_configuration_update(prepared.into_prepared(), || {
            wrote.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!wrote.get());
    assert_eq!(app.configuration_snapshot(), before);
    app.release_configuration_reload(reservation).unwrap();
    let stale = app
        .prepare_interaction_edits(&capture, &PolicySourceIdentity::Temporary, &[create("new")])
        .unwrap();
    app.activate_configuration(app.configuration_layers().clone())
        .unwrap();
    let current = app.configuration_snapshot();
    assert!(
        app.prepare_interaction_edits(&capture, &PolicySourceIdentity::Temporary, &[create("new")])
            .is_err()
    );
    assert!(
        app.publish_configuration_update(stale.into_prepared(), || {
            wrote.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!wrote.get());
    assert_eq!(app.configuration_snapshot(), current);
}

#[test]
fn restoring_shared_values_does_not_make_an_old_capture_current_again() {
    let app = open(ConfigurationLayers::new(Config::default()));
    let capture = app.capture_interaction_edits().unwrap();
    let before = app.configuration_snapshot();
    let shared = capture.shared_profile_revision();
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Private, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let original = peer.configuration_layers().clone();
    let mut changed = original.clone();
    changed.runtime.set_literal("content.zoom", "1.25").unwrap();
    peer.update_configuration(changed).unwrap();
    peer.update_configuration(original).unwrap();
    assert_eq!(app.configuration_snapshot(), before);
    assert!(
        app.prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[create("new")])
            .is_err()
    );
    let fresh = app.capture_interaction_edits().unwrap();
    assert!(fresh.shared_profile_revision() > shared);
    assert_eq!(
        fresh.configuration_revision(),
        capture.configuration_revision()
    );
    app.prepare_interaction_edits(&fresh, &PolicySourceIdentity::Generated, &[create("new")])
        .unwrap();
}

#[test]
fn temporary_and_private_generated_edits_stay_with_the_existing_memory_coordinator() {
    let mut app = open(ConfigurationLayers::new(Config::default()));
    assert!(!app.profile_snapshot().unwrap().durable);
    assert!(app.profile_snapshot().unwrap().roots.is_none());
    let capture = app.capture_interaction_edits().unwrap();
    let prepared = app
        .prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[create("new")])
        .unwrap();
    assert!(
        app.request_configuration_write(prepared.into_prepared())
            .is_err()
    );
    assert!(!app.configuration_write_pending());
    assert!(app.configuration_layers().runtime.site_rules.is_empty());
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Temporary,
            &[create("temp")],
        )
        .unwrap();
    app.publish_configuration_update(prepared.into_prepared(), || Ok(()))
        .unwrap();
    assert!(app.configuration_layers().runtime.site_rules.is_empty());
    assert_eq!(
        app.configuration_layers().temporary.site_rules[0].id,
        "temp"
    );
    assert!(app.profile_snapshot().unwrap().roots.is_none());
}

#[test]
fn authored_and_unsupported_sources_reject_without_rerouting_edits() {
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let mut layers = ConfigurationLayers::new(loaded.config);
    layers.base_authored = loaded.authored;
    let app = open(layers);
    let capture = app.capture_interaction_edits().unwrap();
    let before = app.configuration_policy_sources();
    for source in [
        PolicySourceIdentity::Authored { document_id: 0 },
        PolicySourceIdentity::Authored {
            document_id: u32::MAX,
        },
        PolicySourceIdentity::InMemory,
        PolicySourceIdentity::CommandLine,
        PolicySourceIdentity::Profile {
            name: "work".into(),
        },
    ] {
        assert!(
            app.prepare_interaction_edits(&capture, &source, &[create("new")])
                .is_err()
        );
    }
    assert_eq!(app.configuration_policy_sources(), before);
}

#[test]
fn invalid_late_interaction_patch_rejects_the_whole_batch_without_installing_or_writing() {
    let mut app = open(ConfigurationLayers::new(Config::default()));
    let window = app.active_window().unwrap();
    let url = ferric_browser_core::ValidatedUrl::parse("https://elsewhere.test/live").unwrap();
    let context = ferric_browser_config::SiteMatchContext::for_url(&url).unwrap();
    let installed = app
        .refresh_interaction_snapshot(window, url.as_str())
        .unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let capture = app.capture_interaction_edits().unwrap();
    let mut invalid = create("invalid-policy");
    if let InteractionRuleEdit::Create { interaction, .. } = &mut invalid {
        interaction.labels = Some(ferric_browser_config::InteractionLabelsPatch {
            min_chars: Some(0),
            ..Default::default()
        });
    }
    let error = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[create("valid-first"), invalid],
        )
        .unwrap_err();
    assert!(error.to_string().contains("min_chars"));
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
    assert!(std::sync::Arc::ptr_eq(
        app.cached_interaction_snapshot(window, &context).unwrap(),
        &installed
    ));
    assert!(!app.configuration_write_pending());
    assert_eq!(
        app.capture_interaction_edits()
            .unwrap()
            .shared_profile_revision(),
        capture.shared_profile_revision()
    );
}

#[test]
fn durable_exact_edit_uses_the_existing_writer_and_remains_reload_scoped() {
    use ferric_browser_storage::{RootSpec, StorageRoots};
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let base = roots.data.parent().unwrap().to_owned();
    drop(roots);
    let (mut app, _, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "durable").unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "default".into(),
        label: "Default".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(RootSpec::Base(base.clone())),
        base_configuration: Config::default(),
        base_authored: std::sync::Arc::default(),
        initial_profile_overrides: ferric_browser_config::RuntimeOverrides::default(),
        command_line_overrides: ferric_browser_config::RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = app.poll_profile_open() {
            let effect = result.unwrap();
            assert!(effect.configuration_error.is_none());
            assert!(effect.storage_error.is_none());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.profile_snapshot().unwrap().durable);
    let path = app
        .profile_snapshot()
        .unwrap()
        .roots
        .unwrap()
        .state
        .join("runtime-overrides.toml");
    let capture = app.capture_interaction_edits().unwrap();
    let prepared = app
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[create("saved-exact")],
        )
        .unwrap();
    let before = app.configuration_snapshot();
    app.request_configuration_write(prepared.into_prepared())
        .unwrap();
    assert!(app.configuration_write_pending());
    assert_eq!(app.configuration_snapshot(), before);
    let deadline = Instant::now() + Duration::from_secs(5);
    let receipt = loop {
        if let Some(result) = app.poll_configuration_write() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(receipt.publication_error.is_none());
    assert_eq!(app.configuration_snapshot(), before);
    let saved = ferric_browser_config::load_runtime_overrides(&path).unwrap();
    assert_eq!(saved.site_rules[0].id, "saved-exact");
    app.publish_configuration_update(receipt.prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        app.configuration_layers().runtime.site_rules,
        saved.site_rules
    );
    assert!(app.configuration().site_rules.is_empty());
    let reload = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    app.publish_configuration_update(reload, || Ok(())).unwrap();
    assert_eq!(app.configuration().site_rules[0].id, "saved-exact");
    exercise_durable_draft_apply(&mut app, &path);
    drop(app);
    std::fs::remove_dir_all(base).unwrap();
}

fn exercise_durable_draft_apply(app: &mut BrowserApplication, path: &std::path::Path) {
    use ferric_browser_application::SitePolicyApplyOutcome;
    for activate in [false, true] {
        let before_priority = app.configuration().site_rules[0].priority;
        let opened = app
            .open_site_policy_draft(&draft_edit("saved-exact"))
            .unwrap();
        let mut proposed = opened.rules[0].proposed.clone().unwrap();
        proposed.priority = if activate { 43 } else { 42 };
        let reviewed = app
            .update_site_policy_draft(&opened.id, opened.revision, vec![Some(proposed)])
            .unwrap();
        let (prepared, mut observer) = app
            .begin_site_policy_draft_apply(&opened.id, reviewed.revision, true)
            .unwrap();
        assert!(observer.persist());
        app.request_configuration_write(prepared.into_prepared())
            .unwrap();
        observer.admitted();
        assert!(app.discard_site_policy_draft(&opened.id).is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        let receipt = loop {
            if let Some(result) = app.poll_configuration_write() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        let durability_uncertain = matches!(
            receipt.outcome,
            ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_)
        );
        app.publish_configuration_update(receipt.prepared, || Ok(()))
            .unwrap();
        let activation = observer
            .finish(
                Some(app),
                SitePolicyApplyOutcome::Published {
                    durability_uncertain,
                },
            )
            .unwrap();
        assert_eq!(
            ferric_browser_config::load_runtime_overrides(path)
                .unwrap()
                .site_rules[0]
                .priority,
            if activate { 43 } else { 42 }
        );
        assert_eq!(app.configuration().site_rules[0].priority, before_priority);
        if activate {
            let prepared = activation.prepare_activation(app).unwrap();
            app.publish_configuration_update(prepared, || Ok(()))
                .unwrap();
            activation.finish_activation(Some(app), true);
            assert_eq!(app.configuration().site_rules[0].priority, 43);
        } else {
            let mut layers = app.configuration_layers().clone();
            layers.temporary.site_rules.push(rule("intervening"));
            app.update_configuration(layers).unwrap();
            assert!(activation.prepare_activation(app).is_err());
            activation.finish_activation(Some(app), false);
        }
        let retained = app.site_policy_draft(&opened.id).unwrap();
        let operation = retained.operation.unwrap();
        assert_eq!(operation.saved, Some(true));
        assert_eq!(operation.active, activate);
        app.discard_site_policy_draft(&opened.id).unwrap();
    }
}
