use super::*;
use crate::{
    EffectiveInteractionPolicy, InteractionPolicyBuilder, InteractionPolicyPatch,
    InteractionPolicyRevision, InteractionRemap, PolicySourceIdentity, SiteRule,
};
use std::collections::BTreeMap;

fn patch(from: &str, to: &str, contexts: &[InteractionRemapContext]) -> InteractionPolicyPatch {
    InteractionPolicyPatch {
        remaps: Some(vec![InteractionRemap {
            id: "map".into(),
            from: from.into(),
            to: to.into(),
            contexts: contexts.into(),
        }]),
        ..Default::default()
    }
}
fn requirement(from: &str, to: &str) -> NativeRemapCapabilityRequirement {
    patch(from, to, &[InteractionRemapContext::Passthrough])
        .compile_authored_keys()
        .unwrap()
        .capability_requirements()
        .next()
        .unwrap()
        .clone()
}

#[test]
fn every_printable_ascii_and_planned_named_destination_has_requirement_metadata() {
    use PageKeyDestinationClass as C;
    for byte in b' '..=b'~' {
        let expected = match byte {
            b'a'..=b'z' => C::AsciiLower,
            b'A'..=b'Z' => C::AsciiUpper,
            b'0'..=b'9' => C::Digits,
            _ => C::PunctuationSpace,
        };
        let requirement = requirement("p", &char::from(byte).to_string());
        assert_eq!(requirement.destination_class(), expected);
        assert_eq!(requirement.source().as_str(), "p");
        assert_eq!(requirement.context(), InteractionRemapContext::Passthrough);
        assert_eq!(
            serde_json::to_value(&requirement).unwrap()["qualification"],
            "required"
        );
    }
    for (name, expected) in [
        ("Space", C::PunctuationSpace),
        ("Enter", C::EnterTabBacktab),
        ("Tab", C::EnterTabBacktab),
        ("Backtab", C::EnterTabBacktab),
        ("Backspace", C::EditingNavigation),
        ("Delete", C::EditingNavigation),
        ("Insert", C::EditingNavigation),
        ("Escape", C::EditingNavigation),
        ("Left", C::EditingNavigation),
        ("Right", C::EditingNavigation),
        ("Up", C::EditingNavigation),
        ("Down", C::EditingNavigation),
        ("Home", C::EditingNavigation),
        ("End", C::EditingNavigation),
        ("PgUp", C::EditingNavigation),
        ("PgDown", C::EditingNavigation),
        ("Back", C::BackForward),
        ("Forward", C::BackForward),
    ] {
        assert_eq!(
            requirement("p", name).destination_class(),
            expected,
            "{name}"
        );
    }
    for number in 1..=12 {
        assert_eq!(
            requirement("p", &format!("F{number}")).destination_class(),
            C::FunctionKeys
        );
    }
}

#[test]
fn aliases_are_canonical_but_modifiers_and_gesture_pairs_stay_distinct() {
    for (alias, canonical) in [
        ("+", "Plus"),
        ("Return", "Enter"),
        ("PageUp", "PgUp"),
        ("PageDown", "PgDown"),
        ("Shift+p", "P"),
        ("Ctrl+P", "Ctrl+Shift+p"),
        ("Ctrl+Shift+Tab", "Ctrl+Backtab"),
        ("Shift+Tab", "Backtab"),
    ] {
        assert_eq!(requirement("p", alias), requirement("p", canonical));
    }
    assert_eq!(requirement("Shift+p", "k"), requirement("P", "k"));
    for token in [
        "Ctrl+k",
        "Alt+F1",
        "Meta+Left",
        "Shift+Enter",
        "Ctrl+P",
        "Ctrl+^",
        "Ctrl+Plus",
        "Shift+Space",
        "Ctrl+Backtab",
    ] {
        let value = requirement("p", token);
        assert_eq!(
            value.destination_class(),
            PageKeyDestinationClass::ModifierChords
        );
        assert!(!value.destination().modifiers().is_empty());
    }
    assert_ne!(requirement("p", "k"), requirement("p", "j"));
    assert_ne!(requirement("p", "k"), requirement("q", "k"));
    assert_ne!(requirement("p", "Ctrl+k"), requirement("p", "Ctrl+Shift+k"));
    assert_eq!(
        requirement("p", "Shift+Tab").destination_class(),
        PageKeyDestinationClass::EnterTabBacktab
    );
    assert_eq!(
        requirement("p", "Shift+p").destination_class(),
        PageKeyDestinationClass::AsciiUpper
    );
}

#[test]
fn contexts_are_explicit_and_metadata_never_grants_source_admission() {
    use InteractionRemapContext as C;
    let contexts = [
        C::PassthroughOnce,
        C::Passthrough,
        C::NormalPageKey,
        C::HintPageKey,
    ];
    let compiled = patch("p", "k", &contexts).compile_authored_keys().unwrap();
    let source = LogicalKeyToken::parse("p").unwrap();
    assert_eq!(compiled.capability_requirements().count(), 4);
    assert!(compiled.hint_page_keys().is_empty());
    assert!(compiled.hint_label_source_characters().is_empty());
    for context in contexts {
        let remap = compiled.remap(context, &source).unwrap();
        assert_eq!(remap.id(), "map");
        assert_eq!(remap.capability_requirement().context(), context);
        assert_eq!(remap.destination().as_str(), "k");
        assert_eq!(
            remap.capability_requirement().receiver_scope(),
            NativeReceiverScopeRequirement::TopDocument
        );
    }
    let once = patch("p", "k", &[C::PassthroughOnce])
        .compile_authored_keys()
        .unwrap();
    assert!(once.remap(C::Passthrough, &source).is_none());
    assert!(once.remap(C::HintPageKey, &source).is_none());
    assert_ne!(
        compiled
            .remap(C::Passthrough, &source)
            .unwrap()
            .capability_requirement(),
        compiled
            .remap(C::PassthroughOnce, &source)
            .unwrap()
            .capability_requirement()
    );
    assert!(patch("p", "k", &[]).compile_authored_keys().is_err());
    assert!(
        patch("p", "k", &[C::Passthrough, C::Passthrough])
            .compile_authored_keys()
            .is_err()
    );
    let source_unicode = requirement("é", "k");
    assert_eq!(source_unicode.source().as_str(), "é");
    for destination in [
        "é",
        "Ctrl+é",
        "😀",
        "Ctrl+😀",
        "F13",
        "Ctrl",
        "k j",
        "Ctrl+Escape",
        "Shift+Ctrl+Escape",
    ] {
        assert!(
            patch("p", destination, &[C::Passthrough])
                .compile_authored_keys()
                .is_err(),
            "{destination}"
        );
    }
    for source in ["Ctrl+Escape", "Shift+Ctrl+Escape"] {
        assert!(
            patch(source, "k", &contexts)
                .compile_authored_keys()
                .is_err()
        );
    }
    assert!(
        patch("Space", "k", &[C::HintPageKey])
            .compile_authored_keys()
            .is_err()
    );
    assert!(
        patch("p", "Space", &[C::HintPageKey])
            .compile_authored_keys()
            .is_ok()
    );
    assert!(
        patch("p", "Ctrl+Tab", &[C::HintPageKey])
            .compile_authored_keys()
            .is_ok()
    );
}

fn site_rule(id: &str, patch: InteractionPolicyPatch) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(patch),
    }
}

#[test]
fn cross_entry_alias_overlap_rejects_atomically_and_disjoint_contexts_are_legal() {
    use InteractionRemapContext as C;
    let contexts = [
        C::PassthroughOnce,
        C::Passthrough,
        C::NormalPageKey,
        C::HintPageKey,
    ];
    let source = LogicalKeyToken::parse("Ctrl+P").unwrap();
    for first in contexts {
        for second in contexts {
            let mut value = patch("Ctrl+P", "k", &[first]);
            value.remaps.as_mut().unwrap().push(InteractionRemap {
                id: "second".into(),
                from: "Ctrl+Shift+p".into(),
                to: "j".into(),
                contexts: vec![second],
            });
            let result = value.compile_authored_keys();
            if first == second {
                assert!(result.unwrap_err().contains("overlaps"));
                let mut builder = InteractionPolicyBuilder::default();
                builder
                    .apply_selected_rule(
                        &site_rule("good", patch("p", "k", &[C::Passthrough])),
                        PolicySourceIdentity::Generated,
                        1,
                        0,
                    )
                    .unwrap();
                assert!(
                    builder
                        .apply_selected_rule(
                            &site_rule("bad", value),
                            PolicySourceIdentity::Generated,
                            2,
                            1
                        )
                        .is_err()
                );
                let policy = builder
                    .finish(InteractionPolicyRevision {
                        profile: ferric_browser_core::ProfileId::from_display("profileid-1")
                            .unwrap(),
                        profile_revision: 1,
                        configuration_revision: 1,
                        policy_revision: 1,
                    })
                    .unwrap();
                assert_eq!(policy.remaps().len(), 1);
                assert_eq!(policy.remaps()[0].from, "p");
                assert_eq!(policy.keys().capability_requirements().count(), 1);
                assert_eq!(
                    policy
                        .keys()
                        .capability_requirements()
                        .next()
                        .unwrap()
                        .source()
                        .as_str(),
                    "p"
                );
            } else {
                let compiled = result.unwrap();
                assert_eq!(compiled.capability_requirements().count(), 2);
                assert_eq!(
                    compiled
                        .remap(first, &source)
                        .unwrap()
                        .destination()
                        .as_str(),
                    "k"
                );
                assert_eq!(
                    compiled
                        .remap(second, &source)
                        .unwrap()
                        .destination()
                        .as_str(),
                    "j"
                );
            }
        }
    }
}

fn policy(receiver: InteractionPageReceiver) -> EffectiveInteractionPolicy {
    let mut builder = InteractionPolicyBuilder::default();
    for (index, patch) in [
        InteractionPolicyPatch {
            page_receiver: Some(receiver),
            ..Default::default()
        },
        patch("p", "k", &[InteractionRemapContext::Passthrough]),
    ]
    .into_iter()
    .enumerate()
    {
        builder
            .apply_selected_rule(
                &SiteRule {
                    id: format!("rule-{index}"),
                    pattern: "https://example.test/*".into(),
                    priority: 0,
                    set: BTreeMap::new(),
                    interaction: Some(patch),
                },
                PolicySourceIdentity::Generated,
                1,
                index,
            )
            .unwrap();
    }
    builder
        .finish(InteractionPolicyRevision {
            profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
            profile_revision: 1,
            configuration_revision: 1,
            policy_revision: 1,
        })
        .unwrap()
}

#[test]
fn inherited_receiver_scope_is_frozen_while_origins_remain_admission_data() {
    for receiver in [
        InteractionPageReceiver::TopDocument {},
        InteractionPageReceiver::FocusedFrame {
            origins: vec!["https://frame.test".into()],
        },
        InteractionPageReceiver::TopOrFocusedFrame {
            origins: vec!["https://frame.test".into()],
        },
    ] {
        let expected = match &receiver {
            InteractionPageReceiver::TopDocument {} => NativeReceiverScopeRequirement::TopDocument,
            InteractionPageReceiver::FocusedFrame { .. } => {
                NativeReceiverScopeRequirement::FocusedFrame
            }
            InteractionPageReceiver::TopOrFocusedFrame { .. } => {
                NativeReceiverScopeRequirement::TopOrFocusedFrame
            }
        };
        let policy = policy(receiver.clone());
        assert_eq!(policy.page_receiver(), &receiver);
        let value = policy.keys().capability_requirements().next().unwrap();
        assert_eq!(value.receiver_scope(), expected);
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(
            json["receiver_scope"],
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(json["qualification"], "required");
        assert_eq!(json.as_object().unwrap().len(), 6);
        assert!(json.get("origins").is_none());
    }
    let different_origin = policy(InteractionPageReceiver::FocusedFrame {
        origins: vec!["https://another.test".into()],
    });
    let original = policy(InteractionPageReceiver::FocusedFrame {
        origins: vec!["https://frame.test".into()],
    });
    assert_ne!(different_origin.page_receiver(), original.page_receiver());
    assert_eq!(
        different_origin.keys().capability_requirements().next(),
        original.keys().capability_requirements().next()
    );
    for invalid in [
        InteractionPageReceiver::FocusedFrame { origins: vec![] },
        InteractionPageReceiver::FocusedFrame {
            origins: vec!["https://frame.test/".into()],
        },
        InteractionPageReceiver::TopOrFocusedFrame {
            origins: vec!["null".into()],
        },
    ] {
        let mut value = patch("p", "k", &[InteractionRemapContext::Passthrough]);
        value.page_receiver = Some(invalid);
        assert!(value.compile_authored_keys().is_err());
    }
}

#[test]
fn requirement_inventory_is_bounded_and_reciprocal_mappings_remain_one_hop() {
    use InteractionRemapContext as C;
    let contexts = vec![
        C::PassthroughOnce,
        C::Passthrough,
        C::NormalPageKey,
        C::HintPageKey,
    ];
    let mut value = InteractionPolicyPatch {
        remaps: Some(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"
                .chars()
                .enumerate()
                .map(|(index, source)| InteractionRemap {
                    id: format!("r{index}"),
                    from: source.to_string(),
                    to: "k".into(),
                    contexts: contexts.clone(),
                })
                .collect(),
        ),
        ..Default::default()
    };
    let compiled = value.compile_authored_keys().unwrap();
    assert_eq!(compiled.capability_requirements().count(), 128);
    assert!(compiled.hint_page_keys().is_empty());
    for requirement in compiled.capability_requirements() {
        assert!(serde_json::to_vec(requirement).unwrap().len() < 512);
    }
    value.remaps.as_mut().unwrap().push(InteractionRemap {
        id: "overflow".into(),
        from: "6".into(),
        to: "k".into(),
        contexts,
    });
    assert!(value.compile_authored_keys().is_err());
    let mut reciprocal = patch("p", "k", &[C::Passthrough]);
    reciprocal.remaps.as_mut().unwrap().push(InteractionRemap {
        id: "reverse".into(),
        from: "k".into(),
        to: "p".into(),
        contexts: vec![C::Passthrough],
    });
    let compiled = reciprocal.compile_authored_keys().unwrap();
    for (source, destination) in [("p", "k"), ("k", "p")] {
        let entry = compiled
            .remap(C::Passthrough, &LogicalKeyToken::parse(source).unwrap())
            .unwrap();
        assert_eq!(entry.destination().as_str(), destination);
        assert_eq!(
            entry.capability_requirement().destination().as_str(),
            destination
        );
    }
}
