//! Test/fuzz-only driver for the production decoder. Fixed browser inputs are
//! never reconstructed from the fuzzed stamp or annotation. No live DOM/broker
//! authority, native input or independent publication service is provided.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use ferric_browser_config::{
    Config, ConfigurationSources, InteractionFamily, InteractionField, InteractionPolicyBuilder,
    InteractionPolicyRevision, RuntimeOverrides, SitePolicySourceRevisions, SiteRule,
    select_site_policy_rules,
};
use ferric_browser_core::{DocumentId, HintCandidate, HintTarget, ProfileId, TabId, WindowId};
use serde_json::{Value, json};

use super::{
    AnnotationDocumentRoles, AnnotationFocusedReceiver, AnnotationGenerations, AnnotationRequest,
    CheckedAnnotationFacts, CheckedAnnotations, MAX_INTERACTION_ANNOTATION_BYTES,
    decode_annotation_facts, decode_annotations,
};

const OWNER: &str = "frame-22222222-2222-4222-8222-222222222222";
const ANCHOR: &str = include_str!("../../../../fuzz/corpus/interaction_proof/valid-anchor");
const COLLECTION: &str = include_str!("../../../../fuzz/corpus/interaction_proof/valid-collection");
const FOCUSED: &str = include_str!("../../../../fuzz/corpus/interaction_proof/valid-focused-frame");
const SHADOW: &str = include_str!("../../../../fuzz/corpus/interaction_proof/valid-shadow-slot");
// Independent structural raw seeds share the same browser-owned contexts.
// The actual renderer golden is exercised separately by tests/discovery.rs.
const OBSERVED_ANCHOR: &str =
    include_str!("../../../../fuzz/corpus/interaction_proof/valid-observed-anchor");
const OBSERVED_COLLECTION: &str =
    include_str!("../../../../fuzz/corpus/interaction_proof/valid-observed-collection");
const OBSERVED_UNAVAILABLE: &str =
    include_str!("../../../../fuzz/corpus/interaction_proof/valid-observed-unavailable");
const CANDIDATES: &str =
    include_str!("../../../../fuzz/corpus/interaction_proof/ordinary-candidates");

#[derive(Clone, Copy)]
enum Case {
    Anchor,
    Collection,
    Focused,
    Shadow,
}

impl Case {
    fn seed(self) -> &'static str {
        match self {
            Self::Anchor => ANCHOR,
            Self::Collection => COLLECTION,
            Self::Focused => FOCUSED,
            Self::Shadow => SHADOW,
        }
    }

    fn config(self) -> Config {
        let patch = if matches!(self, Self::Collection) {
            json!({"collections":[{"id":"items","families":["links"],"codes":["1","2"],"container_self":true,"item_selector":".card","primary_selector":"a","identity_attributes":["data-id"],"order":"dom"}]})
        } else {
            let (id, root) = match self {
                Self::Anchor => ("top", Value::Null),
                Self::Focused => (
                    "focused",
                    json!({"start":"focused-frame","receiver_origin":"https://frame.example.test","steps":[]}),
                ),
                Self::Shadow => (
                    "shadow",
                    json!({"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"#shell"},{"kind":"assigned-slot","host_selector":"slot","assigned_selector":".card"}]}),
                ),
                Self::Collection => unreachable!(),
            };
            let mut entry = json!({"id":id,"code":"m","families":["links"],"target_selector":"a","predicates":{"hint_name":{"equals":"Mac"},"role":"link"}});
            if !root.is_null() {
                entry["root"] = root;
            }
            json!({"anchors":[entry]})
        };
        Config {
            site_rules: vec![SiteRule {
                id: "fuzz-base".into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                set: BTreeMap::new(),
                interaction: Some(serde_json::from_value(patch).expect("fixed typed fixture")),
            }],
            ..Config::default()
        }
    }
}

struct Fixture {
    request: AnnotationRequest,
    candidates: Vec<HintCandidate>,
}

impl Fixture {
    fn capture(case: Case, configuration: &Config, revision: u64) -> Self {
        let authored = ConfigurationSources::default();
        let generated = RuntimeOverrides::default();
        let temporary = RuntimeOverrides::default();
        let selected = select_site_policy_rules(
            configuration,
            &authored,
            &generated,
            &temporary,
            SitePolicySourceRevisions::default(),
            "https://example.test/page",
        )
        .expect("fixed fixture resolution");
        let policy = InteractionPolicyBuilder::default()
            .finish_selected_rules(
                &selected,
                InteractionPolicyRevision {
                    profile: ProfileId::from_display("profileid-1").unwrap(),
                    profile_revision: u64::MAX,
                    configuration_revision: revision,
                    policy_revision: 3,
                },
            )
            .expect("fixed fixture policy");
        let focused = matches!(case, Case::Focused);
        let mut ordinary: Value = serde_json::from_str(CANDIDATES).unwrap();
        if focused {
            ordinary[0]["frame_owner"] = json!(OWNER);
        }
        let candidates = crate::parse_hint_payload(&ordinary.to_string())
            .expect("production ordinary decoder admits fixture");
        let request = AnnotationRequest::capture(
            policy,
            WindowId::from_display("windowid-1").unwrap(),
            HintTarget {
                tab: TabId::from_display("tabid-2").unwrap(),
                document: DocumentId::from_display("documentid-3").unwrap(),
                generation: u64::MAX,
            },
            InteractionFamily::Links,
            AnnotationGenerations {
                request_id: u64::MAX,
                ..Default::default()
            },
            Some("scope-fixture"),
            AnnotationDocumentRoles::capture(
                [],
                focused.then(|| AnnotationFocusedReceiver {
                    owner: OWNER.into(),
                    origin: "https://frame.example.test".into(),
                    depth: 1,
                    composed_depth: 1,
                }),
            )
            .expect("fixed native receiver roles"),
        )
        .expect("fixed browser-owned request");
        Self {
            request,
            candidates,
        }
    }
}

fn fixtures() -> &'static [Fixture] {
    static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        [Case::Anchor, Case::Collection, Case::Focused, Case::Shadow]
            .into_iter()
            .map(|case| {
                let fixture = Fixture::capture(case, &case.config(), 1);
                let checked =
                    decode_annotations(case.seed(), &fixture.request, &fixture.candidates)
                        .expect("fixed positive corpus seed");
                check_result(&checked, &fixture);
                let raw_seeds: &[&str] = match case {
                    Case::Anchor => &[OBSERVED_ANCHOR, OBSERVED_UNAVAILABLE],
                    Case::Collection => &[OBSERVED_COLLECTION],
                    Case::Focused | Case::Shadow => &[],
                };
                for raw in raw_seeds {
                    let checked = decode_annotations(raw, &fixture.request, &fixture.candidates)
                        .expect("fixed raw observation corpus seed");
                    check_result(&checked, &fixture);
                }
                fixture
            })
            .collect()
    })
}

fn check_result(checked: &CheckedAnnotations, fixture: &Fixture) {
    let retained = checked.policy_snapshot();
    let expected = &fixture.request.policy;
    assert_eq!(retained.revision(), expected.revision());
    assert_eq!(retained.enabled(), expected.enabled());
    assert_eq!(retained.labels(), expected.labels());
    assert_eq!(retained.ordinary_labels(), expected.ordinary_labels());
    assert_eq!(retained.anchors(), expected.anchors());
    assert_eq!(retained.collections(), expected.collections());
    assert_eq!(retained.hint_page_keys(), expected.hint_page_keys());
    assert_eq!(retained.page_receiver(), expected.page_receiver());
    assert_eq!(retained.remaps(), expected.remaps());
    assert_eq!(retained.keys(), expected.keys());
    for field in [InteractionField::Anchors, InteractionField::Collections] {
        assert_eq!(retained.origin(field), expected.origin(field));
    }
    let mut referenced = BTreeSet::new();
    for outcome in checked.wire.outcomes.iter() {
        match &outcome.result {
            super::wire::ResultFacts::AnchorComplete { matches, .. } => {
                referenced.extend(matches.iter().map(|witness| witness.candidate.index));
            }
            super::wire::ResultFacts::CollectionComplete { items, .. } => {
                referenced.extend(items.iter().map(|item| item.primary.candidate.index));
            }
            super::wire::ResultFacts::Unavailable { .. }
            | super::wire::ResultFacts::SearchComplete { .. } => {}
            super::wire::ResultFacts::AnchorObserved { .. }
            | super::wire::ResultFacts::CollectionObserved { .. } => {
                panic!("checked output must not retain raw observations");
            }
        }
    }
    assert_eq!(
        checked.candidates.keys().copied().collect::<BTreeSet<_>>(),
        referenced,
        "retained snapshots must match referenced indices"
    );
    for index in referenced {
        assert_eq!(
            checked.candidate_snapshot(index),
            Some(&fixture.candidates[usize::from(index)])
        );
    }
    let encoded = serde_json::to_string(checked).expect("checked annotation serializes");
    assert_eq!(serde_json::to_string(checked).unwrap(), encoded);
    // Canonical number spellings can be longer than raw input. The cap is on
    // raw UTF-8 bytes; do not raise it to enforce a false round-trip invariant.
    if encoded.len() <= MAX_INTERACTION_ANNOTATION_BYTES {
        let round_trip = decode_annotations(&encoded, &fixture.request, &fixture.candidates)
            .expect("bounded checked round trip");
        assert_eq!(serde_json::to_string(&round_trip).unwrap(), encoded);
    }
}

/// Exercises raw annotation bytes against four independently captured browser
/// contexts. Returns the number of accepted contexts for corpus-path tests.
///
/// # Panics
/// Panics if the production decoder violates checked-result invariants. Fuzz
/// targets deliberately do not catch such panics or publish any result.
pub fn exercise_annotations(data: &[u8]) -> usize {
    let Ok(raw) = std::str::from_utf8(data) else {
        return 0;
    };
    let mut accepted = 0;
    for fixture in fixtures() {
        let before = fixture.candidates.clone();
        let stamp = fixture.request.stamp_json().unwrap();
        let immediate = decode_annotations(raw, &fixture.request, &fixture.candidates);
        let staged = decode_annotation_facts(raw, &fixture.request, fixture.candidates.clone())
            .and_then(CheckedAnnotationFacts::normalize);
        match (immediate, staged) {
            (Ok(checked), Ok(staged)) => {
                accepted += 1;
                check_result(&checked, fixture);
                check_result(&staged, fixture);
                assert_eq!(
                    serde_json::to_value(&checked).unwrap(),
                    serde_json::to_value(&staged).unwrap()
                );
                assert_eq!(
                    checked.association_documents_json().unwrap(),
                    staged.association_documents_json().unwrap()
                );
                assert_eq!(
                    checked.association_focused_documents_json().unwrap(),
                    staged.association_focused_documents_json().unwrap()
                );
                assert_eq!(staged.original_annotation(), raw);
            }
            (Err(immediate), Err(staged)) => assert_eq!(immediate, staged),
            _ => panic!("owned staging and borrowed decoding disagree"),
        }
        assert_eq!(fixture.candidates, before);
        assert_eq!(fixture.request.stamp_json().unwrap(), stamp);
    }
    accepted
}

#[cfg(test)]
mod tests;
