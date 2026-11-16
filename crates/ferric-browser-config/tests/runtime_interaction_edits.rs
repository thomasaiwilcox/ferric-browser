use ferric_browser_config::{InteractionPolicyPatch, RuntimeOverrides};

const PATTERN: &str = "https://example.test/*";

fn patch(source: &str) -> InteractionPolicyPatch {
    toml::from_str(source).unwrap()
}

fn fixture() -> RuntimeOverrides {
    let mut layer = RuntimeOverrides::default();
    layer
        .set_site_literal(PATTERN, "content.zoom", "1.25")
        .unwrap();
    layer.site_rules[0].id = "target".into();
    layer.site_rules[0].priority = -7;
    layer.site_rules[0].interaction = Some(patch(
        "enabled=false\nhint_page_keys=['p']\n[labels]\nmin_chars=3\nfallback_prefix='z'",
    ));
    let mut other = layer.site_rules[0].clone();
    other.id = "other".into();
    layer.site_rules.push(other);
    layer
}

#[test]
fn replace_is_a_complete_section_edit_preserving_the_rule_and_same_pattern_neighbors() {
    let mut layer = fixture();
    let replacement = patch("[labels]\nmin_chars=2");
    let mut expected = layer.clone();
    expected.site_rules[0].interaction = Some(replacement.clone());
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, replacement.clone())
            .unwrap()
    );
    assert_eq!(layer, expected);
    assert!(
        !layer
            .replace_site_interaction("target", PATTERN, replacement)
            .unwrap()
    );
    assert_eq!(layer, expected);
}

#[test]
fn explicit_empty_section_and_explicit_clears_remain_distinct() {
    let mut layer = fixture();
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, patch(""))
            .unwrap()
    );
    assert_eq!(
        layer.site_rules[0].interaction,
        Some(InteractionPolicyPatch::default())
    );
    let clear = patch("enabled=false\nanchors=[]\ncollections=[]\nhint_page_keys=[]\nremaps=[]");
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, clear.clone())
            .unwrap()
    );
    let round_trip: RuntimeOverrides = toml::from_str(&toml::to_string(&layer).unwrap()).unwrap();
    assert_eq!(round_trip.site_rules[0].interaction, Some(clear));
}

#[test]
fn unset_preserves_existing_settings_and_only_prunes_a_noncontributing_rule() {
    let mut layer = fixture();
    let mut expected = layer.clone();
    expected.site_rules[0].interaction = None;
    assert!(layer.unset_site_interaction("target", PATTERN).unwrap());
    assert_eq!(layer, expected);
    assert!(!layer.unset_site_interaction("target", PATTERN).unwrap());
    assert_eq!(layer, expected);

    let mut layer = fixture();
    layer.site_rules[0].set.clear();
    let other = layer.site_rules[1].clone();
    assert!(layer.unset_site_interaction("target", PATTERN).unwrap());
    assert_eq!(layer.site_rules, [other]);
}

#[test]
fn create_adds_a_distinct_id_without_borrowing_a_matching_pattern() {
    let mut layer = fixture();
    let before = layer.clone();
    let interaction = patch("enabled=false\nanchors=[]");
    layer
        .create_site_interaction("new", PATTERN, 12, interaction.clone())
        .unwrap();
    assert_eq!(&layer.site_rules[..2], before.site_rules);
    let created = &layer.site_rules[2];
    assert_eq!(created.id, "new");
    assert_eq!(created.pattern, PATTERN);
    assert_eq!(created.priority, 12);
    assert!(created.set.is_empty());
    assert_eq!(created.interaction, Some(interaction));
}

#[test]
fn missing_id_has_no_same_pattern_fallback() {
    let mut layer = fixture();
    let before = layer.clone();
    assert!(
        layer
            .replace_site_interaction("absent", PATTERN, patch(""))
            .is_err()
    );
    assert!(!layer.unset_site_interaction("absent", PATTERN).unwrap());
    assert_eq!(layer, before);
}

#[test]
fn pattern_mismatch_rejects_even_when_the_section_is_absent() {
    for absent in [false, true] {
        let mut layer = fixture();
        if absent {
            layer.site_rules[0].interaction = None;
        }
        let before = layer.clone();
        let other_pattern = "https://elsewhere.test/*";
        assert!(
            layer
                .replace_site_interaction("target", other_pattern, patch(""))
                .is_err()
        );
        assert!(
            layer
                .unset_site_interaction("target", other_pattern)
                .is_err()
        );
        assert!(
            layer
                .create_site_interaction("target", other_pattern, 0, patch(""))
                .is_err()
        );
        assert_eq!(layer, before);
    }
}

#[test]
fn collisions_and_duplicate_ids_never_choose_or_overwrite_an_existing_rule() {
    let mut layer = fixture();
    let before = layer.clone();
    assert!(
        layer
            .create_site_interaction("target", PATTERN, 0, patch(""))
            .is_err()
    );
    assert_eq!(layer, before);
    layer.site_rules[1].id = "target".into();
    let before = layer.clone();
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, patch(""))
            .is_err()
    );
    assert!(layer.unset_site_interaction("target", PATTERN).is_err());
    assert!(
        layer
            .create_site_interaction("target", PATTERN, 0, patch(""))
            .is_err()
    );
    assert_eq!(layer, before);
}

#[test]
fn invalid_replacements_and_unrelated_invalid_rules_reject_atomically() {
    let mut layer = fixture();
    let before = layer.clone();
    let invalid = patch("[labels]\nmin_chars=9");
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, invalid.clone())
            .is_err()
    );
    assert!(
        layer
            .create_site_interaction("new", PATTERN, 0, invalid)
            .is_err()
    );
    assert_eq!(layer, before);

    // Rejection after changing the candidate must not change the original.
    layer.site_rules[1].id.clear();
    let before = layer.clone();
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, patch(""))
            .is_err()
    );
    assert!(layer.unset_site_interaction("target", PATTERN).is_err());
    assert!(
        layer
            .create_site_interaction("new", PATTERN, 0, patch(""))
            .is_err()
    );
    assert_eq!(layer, before);
}

#[test]
fn create_obeys_the_existing_256_rule_bound_while_replace_needs_no_new_slot() {
    let mut layer = fixture();
    let template = layer.site_rules[1].clone();
    for index in 2..256 {
        let mut rule = template.clone();
        rule.id = format!("rule-{index}");
        layer.site_rules.push(rule);
    }
    let before = layer.clone();
    assert!(
        layer
            .create_site_interaction("new", PATTERN, 0, patch(""))
            .is_err()
    );
    assert_eq!(layer, before);
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, patch(""))
            .unwrap()
    );
    assert_eq!(layer.site_rules.len(), 256);
    assert_eq!(&layer.site_rules[1..], &before.site_rules[1..]);
}

#[test]
fn unchanged_replacement_still_rejects_an_invalid_unrelated_rule() {
    let mut layer = fixture();
    let same = layer.site_rules[0].interaction.clone().unwrap();
    layer.site_rules[1].id.clear();
    let before = layer.clone();
    assert!(
        layer
            .replace_site_interaction("target", PATTERN, same)
            .is_err()
    );
    assert_eq!(layer, before);
}

#[test]
fn invalid_target_ids_and_patterns_reject_before_reporting_absence() {
    let mut layer = fixture();
    let before = layer.clone();
    for id in [String::new(), "Uppercase".into(), "a".repeat(33)] {
        assert!(layer.unset_site_interaction(&id, PATTERN).is_err());
        assert!(
            layer
                .replace_site_interaction(&id, PATTERN, patch(""))
                .is_err()
        );
        assert!(
            layer
                .create_site_interaction(&id, PATTERN, 0, patch(""))
                .is_err()
        );
        assert_eq!(layer, before);
    }
    for pattern in [
        String::new(),
        "file:///tmp/config".into(),
        format!("https://example.test/{}", "a".repeat(513)),
    ] {
        assert!(layer.unset_site_interaction("absent", &pattern).is_err());
        assert!(
            layer
                .replace_site_interaction("absent", &pattern, patch(""))
                .is_err()
        );
        assert!(
            layer
                .create_site_interaction("absent", &pattern, 0, patch(""))
                .is_err()
        );
        assert_eq!(layer, before);
    }
}

#[test]
fn new_rule_ids_keep_the_existing_outer_slug_boundary() {
    let mut layer = fixture();
    for length in [1, 31, 32, 33] {
        let id = "a".repeat(length);
        let before = layer.clone();
        let result = layer.create_site_interaction(&id, PATTERN, 0, patch(""));
        if length <= 32 {
            result.unwrap();
            assert_eq!(layer.site_rules.last().unwrap().id, id);
        } else {
            assert!(result.is_err());
            assert_eq!(layer, before);
        }
    }
}
