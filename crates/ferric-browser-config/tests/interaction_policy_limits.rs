//! Independent product-boundary examples. A shape-valid token/alphabet is not
//! a qualified native gesture or a usable filtered fallback codebook.
use ferric_browser_config::{
    InteractionField, InteractionPolicyBuilder, InteractionPolicyPatch, InteractionPolicyRevision,
    PolicySourceIdentity, SiteRule, validate_interaction_entry_id, validate_site_rule,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn anchor(index: usize) -> Value {
    json!({"id":format!("a{index}"), "code":format!("a{index:02}"), "families":["links"], "target_selector":"a"})
}
fn collection(index: usize, count: usize) -> Value {
    json!({"id":format!("c{index}"), "families":["links"],
        "codes":(0..count).map(|code| format!("b{index:02}{code:02}")).collect::<Vec<_>>(),
        "container_self":true, "item_selector":".item", "primary_self":true, "order":"dom"})
}
fn patch(value: Value) -> InteractionPolicyPatch {
    serde_json::from_value(value).unwrap()
}

#[test]
fn anchor_collection_and_code_counts_have_minus_exact_plus_boundaries() {
    for enabled in [true, false] {
        for count in [63, 64, 65] {
            let anchors = (0..count).map(anchor).collect::<Vec<_>>();
            assert_eq!(
                patch(json!({"enabled":enabled,"anchors":anchors}))
                    .validate_authored()
                    .is_ok(),
                count <= 64
            );
            assert_eq!(
                patch(json!({"enabled":enabled,"collections":[collection(0, count)]}))
                    .validate_authored()
                    .is_ok(),
                count <= 64
            );
        }
        for count in [15, 16, 17] {
            let collections = (0..count)
                .map(|index| collection(index, 1))
                .collect::<Vec<_>>();
            assert_eq!(
                patch(json!({"enabled":enabled,"collections":collections}))
                    .validate_authored()
                    .is_ok(),
                count <= 16
            );
        }
    }
}

#[test]
fn key_remap_and_exclusion_counts_use_their_own_32_entry_bounds() {
    let characters = "abcdefghijklmnopqrstuvwxyz0123456789";
    for count in [31, 32, 33] {
        let tokens = characters
            .chars()
            .take(count)
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        let remaps = tokens.iter().enumerate().map(|(index, token)| json!({"id":format!("r{index}"),"from":token,"to":"k","contexts":["passthrough"]})).collect::<Vec<_>>();
        for value in [
            json!({"hint_page_keys":tokens}),
            json!({"remaps":remaps}),
            json!({"labels":{"excluded_chars":tokens}}),
        ] {
            assert_eq!(patch(value).validate_authored().is_ok(), count <= 32);
        }
    }
}

#[test]
fn id_label_prefix_and_utf8_operand_bounds_are_not_character_counts() {
    for bytes in [63, 64, 65] {
        assert_eq!(
            validate_interaction_entry_id(&"x".repeat(bytes)).is_ok(),
            bytes <= 64
        );
        let source = patch(json!({"hint_page_keys":["x".repeat(bytes)]}));
        assert_eq!(source.validate_shape_and_selectors().is_ok(), bytes <= 64);
        // These long strings are not gestures, even when their size is legal.
        assert!(source.validate_authored().is_err());
    }
    for length in [6, 7, 8] {
        assert_eq!(
            patch(json!({"labels":{"fallback_prefix":"z".repeat(length)}}))
                .validate_authored()
                .is_ok(),
            length <= 7
        );
    }
    for length in [7, 8, 9] {
        assert_eq!(
            patch(json!({"labels":{"min_chars":length}}))
                .validate_authored()
                .is_ok(),
            length <= 8
        );
        let mut entry = anchor(0);
        entry["code"] = json!("x".repeat(length));
        assert_eq!(
            patch(json!({"anchors":[entry]}))
                .validate_authored()
                .is_ok(),
            length <= 8
        );
    }
    for bytes in [255_usize, 256, 257] {
        let operand = format!(
            "{}{}",
            "é".repeat(bytes / 2),
            if bytes.is_multiple_of(2) { "" } else { "a" }
        );
        assert_eq!(operand.len(), bytes);
        for predicates in [
            json!({"hint_name":{"equals":operand}}),
            json!({"attributes":[{"name":"data-x","prefix":operand}]}),
        ] {
            let mut entry = anchor(0);
            entry["predicates"] = predicates;
            assert_eq!(
                patch(json!({"anchors":[entry]}))
                    .validate_authored()
                    .is_ok(),
                bytes <= 256
            );
        }
    }
}

#[test]
fn item_attributes_and_receiver_origins_preserve_distinct_caps() {
    for count in [3, 4, 5] {
        let mut entry = collection(0, 1);
        entry["item_attributes"] = json!(
            (0..count)
                .map(|index| json!({"name":format!("data-{index}"),"present":true}))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            patch(json!({"collections":[entry]}))
                .validate_authored()
                .is_ok(),
            count <= 4
        );
    }
    for count in [7, 8, 9] {
        let origins = (0..count)
            .map(|index| format!("https://frame{index}.example.test"))
            .collect::<Vec<_>>();
        assert_eq!(
            patch(json!({"page_receiver":{"scope":"focused-frame","origins":origins}}))
                .validate_authored()
                .is_ok(),
            count <= 8
        );
    }
}

fn apply(builder: &mut InteractionPolicyBuilder, id: &str, interaction: InteractionPolicyPatch) {
    builder
        .apply_selected_rule(
            &SiteRule {
                id: id.into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                set: BTreeMap::new(),
                interaction: Some(interaction),
            },
            PolicySourceIdentity::Generated,
            1,
            0,
        )
        .unwrap();
}
fn revision() -> InteractionPolicyRevision {
    InteractionPolicyRevision {
        profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
        profile_revision: 0,
        configuration_revision: 1,
        policy_revision: 1,
    }
}

#[test]
fn composed_reservations_reject_513_and_replacement_clear_repairs_them() {
    for total in [511, 512, 513] {
        let codes = total - 1;
        let collections = (0..8)
            .map(|index| collection(index, if index == 7 { codes - 7 * 64 } else { 64 }))
            .collect::<Vec<_>>();
        let mut builder = InteractionPolicyBuilder::default();
        apply(
            &mut builder,
            "anchors",
            patch(json!({"anchors":[anchor(0)]})),
        );
        // Each source is valid on its own; final composition owns aggregate bounds.
        apply(
            &mut builder,
            "collections",
            patch(json!({"collections":collections})),
        );
        let outcome = builder.clone().finish(revision());
        assert_eq!(outcome.is_ok(), total <= 512);
        if total > 512 {
            assert!(outcome.unwrap_err().contains("512"));
            apply(&mut builder, "clear", patch(json!({"anchors":[]})));
            let repaired = builder.finish(revision()).unwrap();
            assert_eq!(
                repaired
                    .collections()
                    .iter()
                    .map(|collection| collection.codes.len())
                    .sum::<usize>(),
                512
            );
            assert_eq!(
                repaired
                    .origin(InteractionField::Anchors)
                    .unwrap()
                    .rule()
                    .unwrap()
                    .id(),
                "clear"
            );
        }
    }
}

#[test]
fn legacy_setting_limit_precedes_unsupported_vocabulary_without_expanding_it() {
    assert_eq!(ferric_browser_config::MAX_SITE_RULE_SETTINGS, 32);
    for count in [31, 32, 33] {
        let rule = SiteRule {
            id: "limits".into(),
            pattern: "https://example.test/*".into(),
            priority: 0,
            set: (0..count)
                .map(|index| (format!("unsupported{index}"), toml::Value::Boolean(true)))
                .collect(),
            interaction: None,
        };
        let error = validate_site_rule(&rule).unwrap_err().to_string();
        assert!(error.contains(if count <= 32 {
            "unsupported setting"
        } else {
            "too many settings"
        }));
    }
}

#[test]
fn regression_entry_overflow_identifies_the_field_count_and_limit() {
    for (value, field, count, limit) in [
        (
            json!({"anchors":(0..65).map(anchor).collect::<Vec<_>>()}),
            "anchors",
            65,
            64,
        ),
        (
            json!({"collections":(0..17).map(|i| collection(i, 1)).collect::<Vec<_>>()}),
            "collections",
            17,
            16,
        ),
        (
            json!({"hint_page_keys":vec!["k"; 33]}),
            "hint_page_keys",
            33,
            32,
        ),
        (
            json!({"remaps":vec![json!({"id":"r","from":"p","to":"k","contexts":["passthrough"]}); 33]}),
            "remaps",
            33,
            32,
        ),
    ] {
        let error = patch(value).validate_authored().unwrap_err();
        for expected in [
            "authored".to_owned(),
            field.to_owned(),
            count.to_string(),
            limit.to_string(),
        ] {
            assert!(error.contains(&expected), "{error}");
        }
    }
}
