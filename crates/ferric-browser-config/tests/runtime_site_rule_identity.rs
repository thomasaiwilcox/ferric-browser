use ferric_browser_config::{InteractionPolicyPatch, RuntimeOverrides, SiteRule};

const PATTERN: &str = "https://example.test/*";

fn canonical() -> RuntimeOverrides {
    let mut layer = RuntimeOverrides::default();
    layer
        .set_site_literal(PATTERN, "content.zoom", "1.25")
        .unwrap();
    layer
}

fn custom() -> SiteRule {
    let mut rule = canonical().site_rules.remove(0);
    rule.id = "custom-imported".into();
    rule.interaction = Some(InteractionPolicyPatch {
        enabled: Some(false),
        ..Default::default()
    });
    rule
}

#[test]
fn pattern_set_creates_canonical_rule_without_editing_custom_rule() {
    let custom = custom();
    let expected_id = canonical().site_rules.remove(0).id;
    let mut layer = RuntimeOverrides {
        site_rules: vec![custom.clone()],
        ..Default::default()
    };
    layer
        .set_site_literal(PATTERN, "content.zoom", "1.5")
        .unwrap();
    assert_eq!(layer.site_rules.len(), 2);
    assert_eq!(layer.site_rules[0], custom);
    assert_eq!(layer.site_rules[1].id, expected_id);
    assert_eq!(
        layer.site_rules[1].set["content.zoom"].as_float(),
        Some(1.5)
    );
}

#[test]
fn pattern_unset_reports_absence_when_only_custom_rule_exists() {
    let mut layer = RuntimeOverrides {
        site_rules: vec![custom()],
        ..Default::default()
    };
    let before = layer.clone();
    assert!(!layer.unset_site_setting(PATTERN, "content.zoom").unwrap());
    assert_eq!(layer, before);
}

#[test]
fn pattern_set_selects_canonical_identity_even_after_same_pattern_custom_rule() {
    let custom = custom();
    let mut layer = canonical();
    let canonical_id = layer.site_rules[0].id.clone();
    layer.site_rules.insert(0, custom.clone());
    layer
        .set_site_literal(PATTERN, "content.zoom", "1.5")
        .unwrap();
    assert_eq!(layer.site_rules.len(), 2);
    assert_eq!(layer.site_rules[0], custom);
    assert_eq!(layer.site_rules[1].id, canonical_id);
    assert_eq!(
        layer.site_rules[1].set["content.zoom"].as_float(),
        Some(1.5)
    );
}

#[test]
fn pattern_unset_selects_canonical_identity_and_preserves_interaction() {
    let custom = custom();
    let mut layer = canonical();
    layer.site_rules[0].interaction = custom.interaction.clone();
    let mut expected_canonical = layer.site_rules[0].clone();
    expected_canonical.set.clear();
    layer.site_rules.insert(0, custom.clone());
    assert!(layer.unset_site_setting(PATTERN, "content.zoom").unwrap());
    assert_eq!(layer.site_rules, [custom, expected_canonical]);
    let before = layer.clone();
    assert!(!layer.unset_site_setting(PATTERN, "content.zoom").unwrap());
    assert_eq!(layer, before);
}

#[test]
fn canonical_id_pattern_mismatch_rejects_set_and_unset_atomically() {
    for unset in [false, true] {
        let mut layer = canonical();
        layer.site_rules[0].pattern = "https://elsewhere.test/*".into();
        layer.site_rules.push(custom());
        let before = layer.clone();
        let error = if unset {
            layer
                .unset_site_setting(PATTERN, "content.zoom")
                .unwrap_err()
        } else {
            layer
                .set_site_literal(PATTERN, "content.zoom", "1.5")
                .unwrap_err()
        };
        assert!(error.to_string().contains("pattern"));
        assert_eq!(layer, before);
    }
}

#[test]
fn canonical_creation_obeys_rule_capacity_without_borrowing_a_custom_identity() {
    for count in [255, 256] {
        let mut layer = RuntimeOverrides::default();
        for index in 0..count {
            let mut rule = custom();
            rule.id = format!("custom-{index}");
            layer.site_rules.push(rule);
        }
        let before = layer.clone();
        let result = layer.set_site_literal(PATTERN, "content.zoom", "1.5");
        if count == 255 {
            result.unwrap();
            assert_eq!(layer.site_rules.len(), 256);
            assert_eq!(&layer.site_rules[..count], before.site_rules);
            // Updating the existing canonical rule needs no additional slot.
            layer
                .set_site_literal(PATTERN, "content.zoom", "1.75")
                .unwrap();
            assert_eq!(layer.site_rules.len(), 256);
            assert_eq!(&layer.site_rules[..count], before.site_rules);
        } else {
            assert!(result.is_err());
            assert_eq!(layer, before);
        }
    }
}

fn rejects_duplicate_canonical_unset(second_pattern: &str) {
    let mut layer = canonical();
    let mut duplicate = layer.site_rules[0].clone();
    duplicate.pattern = second_pattern.into();
    layer.site_rules.push(duplicate);
    let before = layer.clone();
    assert!(layer.unset_site_setting(PATTERN, "content.zoom").is_err());
    assert_eq!(layer, before);
}

#[test]
fn duplicate_canonical_rules_cannot_be_repaired_by_guessing_which_one_to_unset() {
    rejects_duplicate_canonical_unset(PATTERN);
}

#[test]
fn duplicate_canonical_id_with_another_pattern_cannot_survive_a_guessed_unset() {
    rejects_duplicate_canonical_unset("https://elsewhere.test/*");
}

#[test]
fn unset_validation_failure_preserves_the_entire_original_layer() {
    let mut layer = canonical();
    let mut invalid_unrelated = custom();
    invalid_unrelated.id.clear();
    layer.site_rules.push(invalid_unrelated);
    let before = layer.clone();
    assert!(layer.unset_site_setting(PATTERN, "content.zoom").is_err());
    assert_eq!(layer, before);
}
