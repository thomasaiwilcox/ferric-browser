//! Test-first P00 contracts for the additive v3 Interaction schema (P02).
//! These failed on the pinned baseline and exercise production schema integration.
use ferric_browser_config::{Config, validate};

fn parse_rule(patch: &str) -> Config {
    let source = format!(
        "schema_version = 3\n[[site_rules]]\nid = 'interaction-fixture'\n\
         pattern = 'https://example.test/*'\n\
         [site_rules.set]\n'content.zoom' = 1.25\n{patch}"
    );
    let config: Config = toml::from_str(&source).expect("additive Interaction schema must parse");
    validate(&config).expect("authored Interaction values must validate");
    config
}

#[test]
fn t_p02_sparse_interaction_preserves_absence_clear_and_unrelated_settings() {
    for patch in [
        "[site_rules.interaction]\n",
        "[site_rules.interaction]\nenabled = false\n",
        "[site_rules.interaction.labels]\nmin_chars = 2\n",
        "[site_rules.interaction]\nanchors = []\ncollections = []\nremaps = []\nhint_page_keys = []\n",
    ] {
        let config = parse_rule(patch);
        let encoded = toml::Value::try_from(&config).expect("serialize sparse source");
        let expected: toml::Value = toml::from_str(patch).expect("fixture TOML");
        // The standalone patch table supplies the expected sparse section;
        // Config stores that section on the first site_rules array entry.
        let rule = &encoded["site_rules"][0];
        assert_eq!(rule["interaction"], expected["site_rules"]["interaction"]);
        assert_eq!(rule["set"]["content.zoom"].as_float(), Some(1.25));
        assert_eq!(encoded["schema_version"].as_integer(), Some(3));
        let decoded: Config = encoded.try_into().expect("round trip");
        assert_eq!(decoded, config);
    }
    let old = parse_rule("");
    let encoded = toml::Value::try_from(&old).expect("old schema serializes");
    assert!(encoded["site_rules"][0].get("interaction").is_none());
}

#[test]
fn t_p01_old_v3_omits_interaction_on_round_trip() {
    let config = parse_rule("");
    let encoded = toml::to_string(&config).expect("serialize old v3 config");
    let value: toml::Value = toml::from_str(&encoded).expect("serialized TOML");
    assert!(value["site_rules"][0].get("interaction").is_none());
    assert_eq!(toml::from_str::<Config>(&encoded).unwrap(), config);
}

#[test]
fn t_p07_interaction_rejects_unknown_fields_even_when_disabled() {
    parse_rule("[site_rules.interaction]\nenabled=false\n");
    let source = "schema_version = 3\n[[site_rules]]\nid='fixture'\n\
                  pattern='https://example.test/*'\n\
                  [site_rules.interaction]\nenabled=false\nunknown_behavior=true\n";
    assert!(toml::from_str::<Config>(source).is_err());
}

#[test]
fn t_p09_unsetting_last_ordinary_setting_keeps_interaction_data() {
    let config = parse_rule("[site_rules.interaction.labels]\nmin_chars=2\n");
    let mut overrides = ferric_browser_config::RuntimeOverrides::default();
    overrides
        .set_site_literal("https://example.test/*", "content.zoom", "1.25")
        .unwrap();
    overrides.site_rules[0].interaction = config.site_rules[0].interaction.clone();
    assert!(
        overrides
            .unset_site_setting("https://example.test/*", "content.zoom")
            .unwrap()
    );
    assert_eq!(overrides.site_rules.len(), 1);
    assert!(overrides.site_rules[0].set.is_empty());
    assert!(overrides.site_rules[0].interaction.is_some());
}

#[test]
fn explicit_null_is_not_optional_field_absence() {
    use ferric_browser_config::*;
    use serde_json::{Value, json};
    fn rejects<T: serde::de::DeserializeOwned>(base: &Value, fields: &[&str]) {
        assert!(serde_json::from_value::<T>((*base).clone()).is_ok());
        for field in fields {
            let mut candidate = (*base).clone();
            candidate[field] = Value::Null;
            assert!(
                serde_json::from_value::<T>(candidate).is_err(),
                "accepted null field {field}"
            );
        }
    }
    rejects::<InteractionPolicyPatch>(
        &json!({}),
        &[
            "enabled",
            "labels",
            "anchors",
            "collections",
            "hint_page_keys",
            "page_receiver",
            "remaps",
        ],
    );
    rejects::<InteractionLabelsPatch>(
        &json!({}),
        &[
            "alphabet",
            "min_chars",
            "fallback_prefix",
            "excluded_chars",
            "auto_follow",
        ],
    );
    rejects::<InteractionAnchor>(
        &json!({"id":"a","code":"m","families":["links"],"target_selector":"a"}),
        &["root", "scope_selector", "target_selector", "predicates"],
    );
    rejects::<InteractionCollection>(
        &json!({"id":"c","families":["links"],"codes":["1"],"container_selector":"#c","item_selector":"article","primary_selector":"a","order":"dom"}),
        &[
            "root",
            "container_selector",
            "container_self",
            "primary_root_steps",
            "primary_selector",
            "primary_self",
            "item_attributes",
            "primary_predicates",
            "identity_attributes",
            "row_tolerance_px",
        ],
    );
    rejects::<InteractionRoot>(&json!({"start":"top-document"}), &["receiver_origin"]);
    rejects::<InteractionRootStep>(
        &json!({"kind":"open-shadow","host_selector":"x-host"}),
        &["host_selector", "self", "assigned_selector"],
    );
    rejects::<InteractionPredicates>(
        &json!({}),
        &[
            "hint_name",
            "href",
            "role",
            "attributes",
            "width_px",
            "height_px",
            "viewport_x",
            "viewport_y",
        ],
    );
    rejects::<InteractionNameMatch>(&json!({"equals":"Mac"}), &["equals", "contains", "case"]);
    rejects::<InteractionHrefMatch>(&json!({"path":"/*"}), &["origin", "path"]);
    rejects::<InteractionAttributeTest>(
        &json!({"name":"data-id","present":true}),
        &["equals", "prefix", "present"],
    );
    rejects::<SiteRule>(
        &json!({"id":"fixture","pattern":"https://example.test/*"}),
        &["interaction"],
    );
}

#[test]
fn implementation_plan_examples_parse_validate_and_round_trip() {
    // Exact full-configuration examples from plan Sections 8.1–8.3; the
    // documented root-field fragment is also tested inside its collection.
    for (name, source) in [
        (
            "navigation",
            include_str!("fixtures/site-policy/navigation.toml"),
        ),
        (
            "suggestions",
            include_str!("fixtures/site-policy/suggestions.toml"),
        ),
        (
            "shadow-grid",
            include_str!("fixtures/site-policy/shadow-grid.toml"),
        ),
        (
            "frame-shadow-grid",
            include_str!("fixtures/site-policy/frame-shadow-grid.toml"),
        ),
    ] {
        let config: Config = toml::from_str(source)
            .unwrap_or_else(|error| panic!("plan example {name} must parse: {error}"));
        validate(&config)
            .unwrap_or_else(|error| panic!("plan example {name} must validate: {error}"));
        let encoded = toml::to_string(&config).expect("example serializes");
        assert_eq!(
            toml::from_str::<Config>(&encoded).unwrap(),
            config,
            "{name}"
        );
    }
}
