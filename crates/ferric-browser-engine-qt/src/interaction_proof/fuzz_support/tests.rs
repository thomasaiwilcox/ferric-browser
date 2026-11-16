use super::*;
use crate::interaction_proof::AnnotationError;
use ferric_browser_application::{
    BrowserApplication, ConfigurationLayers, PreparedConfigurationConsumer,
};
use ferric_browser_config::ConfigError;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const UNAVAILABLE: &str =
    include_str!("../../../../../fuzz/corpus/interaction_proof/valid-unavailable");
const NEGATIVE: [&str; 6] = [
    include_str!("../../../../../fuzz/corpus/interaction_proof/stale-request"),
    include_str!("../../../../../fuzz/corpus/interaction_proof/forbidden-command"),
    include_str!("../../../../../fuzz/corpus/interaction_proof/dangling-root"),
    include_str!("../../../../../fuzz/corpus/interaction_proof/invalid-geometry"),
    include_str!("../../../../../fuzz/corpus/interaction_proof/duplicate-version"),
    include_str!("../../../../../fuzz/corpus/interaction_proof/truncated"),
];

#[test]
fn positive_annotation_corpus_reaches_each_real_decoder_context() {
    for raw in [
        ANCHOR,
        COLLECTION,
        FOCUSED,
        SHADOW,
        UNAVAILABLE,
        OBSERVED_ANCHOR,
        OBSERVED_COLLECTION,
        OBSERVED_UNAVAILABLE,
    ] {
        assert_eq!(exercise_annotations(raw.as_bytes()), 1);
    }
    assert_eq!(exercise_annotations(CANDIDATES.as_bytes()), 0);
    for raw in NEGATIVE {
        assert_eq!(exercise_annotations(raw.as_bytes()), 0);
    }
}

#[test]
fn every_incomplete_json_prefix_rejects_without_changing_browser_inputs() {
    for raw in [
        ANCHOR,
        COLLECTION,
        FOCUSED,
        SHADOW,
        OBSERVED_ANCHOR,
        OBSERVED_COLLECTION,
        OBSERVED_UNAVAILABLE,
    ] {
        let raw = raw.trim_end();
        for length in 0..raw.len() {
            assert_eq!(
                exercise_annotations(&raw.as_bytes()[..length]),
                0,
                "accepted incomplete prefix at {length}"
            );
        }
    }
}

#[test]
fn independently_mutated_stamps_references_and_geometry_never_change_context() {
    let fixture = &fixtures()[0];
    let baseline: Value = serde_json::from_str(ANCHOR).unwrap();
    let before = fixture.candidates.clone();
    for field in baseline["stamp"].as_object().unwrap().keys() {
        let mut changed = baseline.clone();
        changed["stamp"][field] = Value::Null;
        assert!(
            decode_annotations(&changed.to_string(), &fixture.request, &fixture.candidates)
                .is_err()
        );
    }
    for (pointer, value) in [
        ("/documents/0/id", json!(0)),
        ("/documents/0/epoch", json!("changed")),
        ("/roots/0/resolved/document", json!(999)),
        ("/outcomes/0/rule/id", json!("unknown")),
        (
            "/outcomes/0/result/anchor-complete/matches/0/candidate/element_id",
            json!(11),
        ),
        (
            "/outcomes/0/result/anchor-complete/matches/0/projected_primary_fragment_css/width",
            json!(-1),
        ),
        (
            "/outcomes/0/result/anchor-complete/matches/0/name",
            json!({"truncated":{}}),
        ),
    ] {
        let mut changed = baseline.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            decode_annotations(&changed.to_string(), &fixture.request, &fixture.candidates)
                .is_err()
        );
    }
    assert_eq!(fixture.candidates, before);
    assert!(decode_annotations(ANCHOR, &fixture.request, &fixture.candidates).is_ok());
}

#[test]
fn generated_role_work_exhaustion_and_raw_byte_exhaustion_are_closed_errors() {
    let fixture = &fixtures()[0];
    let mut changed: Value = serde_json::from_str(ANCHOR).unwrap();
    changed["outcomes"][0]["result"]["anchor-complete"]["matches"][0]["role"] =
        json!({"complete-present":{"value":"x ".repeat(100_001)}});
    assert_eq!(
        decode_annotations(&changed.to_string(), &fixture.request, &fixture.candidates)
            .unwrap_err(),
        AnnotationError::WorkLimit
    );
    let oversized = " ".repeat(MAX_INTERACTION_ANNOTATION_BYTES + 1);
    assert_eq!(
        decode_annotations(&oversized, &fixture.request, &fixture.candidates).unwrap_err(),
        AnnotationError::TooLarge
    );
}

fn prepare(
    app: &BrowserApplication,
    raw: &str,
) -> Result<PreparedConfigurationConsumer<CheckedAnnotations>, ConfigError> {
    app.prepare_configuration_reload(ConfigurationLayers::new(Case::Anchor.config()))?
        .prepare_consumer(|effective, _, revision| {
            // Compile from the actual prepared candidate and proposed revision.
            // This fixture has a single base source and shared default settings;
            // it does not substitute for the full P03 global/site compiler.
            let fixture = Fixture::capture(Case::Anchor, effective, revision);
            decode_annotations(raw, &fixture.request, &fixture.candidates)
                .map_err(|error| ConfigError::Validation(error.to_string()))
        })
}

#[test]
fn annotation_parsing_and_publication_share_the_real_prepared_transaction() {
    let mut app = BrowserApplication::new(Config::default());
    let before = app.configuration_snapshot();
    let persisted = Cell::new(0);
    for raw in NEGATIVE.into_iter().chain([&ANCHOR[..ANCHOR.len() / 2]]) {
        assert!(prepare(&app, raw).is_err());
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(persisted.get(), 0);
    }
    let prepared = prepare(&app, ANCHOR).unwrap();
    let (configuration, annotations) = app
        .publish_configuration_consumer(prepared, || {
            persisted.set(persisted.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(persisted.get(), 1);
    assert_eq!(configuration.revision, 1);
    assert_eq!(
        annotations
            .policy_snapshot()
            .revision()
            .configuration_revision,
        configuration.revision
    );
    assert_eq!(annotations.candidate_snapshot(0).unwrap().text, "Mac");
}

#[test]
fn preparation_panic_or_stale_annotation_never_persists_and_does_not_poison_owner() {
    let mut app = BrowserApplication::new(Config::default());
    let before = app.configuration_snapshot();
    let prepared = app
        .prepare_configuration_reload(ConfigurationLayers::new(Case::Anchor.config()))
        .unwrap();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = prepared.prepare_consumer::<CheckedAnnotations>(|_, _, _| {
            panic!("injected compiler panic before persistence")
        });
    }));
    assert!(panic.is_err());
    assert_eq!(app.configuration_snapshot(), before);
    // The same owner can publish after the injected preparation panic.
    let prepared = prepare(&app, ANCHOR).unwrap();
    assert!(
        app.publish_configuration_consumer(prepared, || Ok(()))
            .is_ok()
    );

    let mut stale_owner = BrowserApplication::new(Config::default());
    let prepared = prepare(&stale_owner, ANCHOR).unwrap();
    stale_owner
        .update_configuration(ConfigurationLayers::new(Config::default()))
        .unwrap();
    let intervening = stale_owner.configuration_snapshot();
    assert!(
        stale_owner
            .publish_configuration_consumer(prepared, || panic!("stale consumer must not persist"))
            .is_err()
    );
    assert_eq!(stale_owner.configuration_snapshot(), intervening);
}

#[test]
#[should_panic(expected = "retained snapshots must match referenced indices")]
fn fuzz_invariant_detects_an_injected_missing_snapshot() {
    let fixture = &fixtures()[0];
    let mut checked = decode_annotations(ANCHOR, &fixture.request, &fixture.candidates).unwrap();
    checked.candidates.clear();
    check_result(&checked, fixture);
}
