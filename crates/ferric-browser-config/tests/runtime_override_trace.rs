use ferric_browser_config::{
    Config, HintSelectorFamily, HintSelectorMode, HintSelectorPolicy, RuntimeOverrides,
    apply_runtime_overrides, apply_runtime_overrides_traced,
};

#[test]
fn canonical_assignment_trace_keeps_composed_selectors_and_literal_binding_unsets() {
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: vec!["#base".into()],
        },
    );
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#extra']}",
        )
        .unwrap();
    overrides.set_binding("normal", ".", "").unwrap();
    let traced = apply_runtime_overrides_traced(&base, &overrides).unwrap();
    assert_eq!(
        traced.configuration,
        apply_runtime_overrides(&base, &overrides).unwrap()
    );
    assert_eq!(traced.assignments.len(), 2);
    assert_eq!(traced.assignments[0].input["mode"].as_str(), Some("extend"));
    assert_eq!(
        traced.assignments[0].assigned["mode"].as_str(),
        Some("replace")
    );
    assert_eq!(
        traced.assignments[0].assigned["selectors"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let unset = &traced.assignments[1];
    assert_eq!(unset.path, ["bindings", "normal", "."]);
    assert_eq!(unset.input.as_str(), Some(""));
    assert_eq!(unset.assigned.as_str(), Some("unbound"));
    assert_eq!(traced.configuration.bindings["normal"]["."], "unbound");
}

#[test]
fn rejected_layer_has_no_partial_trace_or_modified_base() {
    let base = Config::default();
    let before = base.clone();
    let mut overrides = RuntimeOverrides::default();
    overrides
        .settings
        .insert("content.images".into(), toml::Value::Boolean(false));
    overrides
        .settings
        .insert("content.zoom".into(), toml::Value::Integer(99));
    assert!(apply_runtime_overrides_traced(&base, &overrides).is_err());
    assert!(apply_runtime_overrides(&base, &overrides).is_err());
    assert_eq!(base, before);
}
