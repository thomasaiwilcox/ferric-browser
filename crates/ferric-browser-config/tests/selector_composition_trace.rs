use ferric_browser_config::{
    HintSelectorContributionOutcome, HintSelectorFamily, HintSelectorInputField, HintSelectorMode,
    HintSelectorPolicy, HintSelectorPolicyBuilder, HintSelectorSeedOrigins, PolicyFieldOrigin,
    PolicySourceIdentity,
};
use std::{collections::BTreeMap, sync::Arc};

fn origin(source: PolicySourceIdentity) -> Arc<PolicyFieldOrigin> {
    Arc::new(PolicyFieldOrigin::shared(source, 7).unwrap())
}

fn settings(mode: &str, selectors: &[&str]) -> BTreeMap<String, toml::Value> {
    BTreeMap::from([(
        "hints.selectors.buttons".into(),
        toml::Value::Table(toml::Table::from_iter([
            ("mode".into(), toml::Value::String(mode.into())),
            (
                "selectors".into(),
                toml::Value::Array(
                    selectors
                        .iter()
                        .map(|value| toml::Value::String((*value).into()))
                        .collect(),
                ),
            ),
        ])),
    )])
}

fn seeded(
    selectors: Vec<String>,
) -> (
    BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
    BTreeMap<HintSelectorFamily, HintSelectorSeedOrigins>,
) {
    (
        BTreeMap::from([(
            HintSelectorFamily::Buttons,
            HintSelectorPolicy {
                mode: HintSelectorMode::Replace,
                selectors,
            },
        )]),
        BTreeMap::from([(
            HintSelectorFamily::Buttons,
            HintSelectorSeedOrigins {
                mode: origin(PolicySourceIdentity::Authored { document_id: 0 }),
                selectors: Some(origin(PolicySourceIdentity::Authored { document_id: 1 })),
            },
        )]),
    )
}

#[test]
fn extend_retains_mode_and_duplicate_origins_while_replace_retires_them() {
    let (base, seeds) = seeded(vec!["#base".into()]);
    let profile = origin(PolicySourceIdentity::Profile {
        name: "default".into(),
    });
    let mut traced = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    let mut plain = HintSelectorPolicyBuilder::from_base(&base, "base").unwrap();
    let input = settings("extend", &["#base", "#new"]);
    traced
        .apply_settings_with_origin(&input, "profile", &profile)
        .unwrap();
    plain.apply_settings(&input, "profile").unwrap();
    let (policies, trace) = traced.finish_with_trace().unwrap();
    assert_eq!(policies, plain.finish().unwrap());
    let fields = &trace[&HintSelectorFamily::Buttons].fields;
    assert_eq!(
        fields.mode.as_ref().unwrap().source,
        seeds[&HintSelectorFamily::Buttons].mode
    );
    assert_eq!(
        fields.selectors[0].source,
        seeds[&HintSelectorFamily::Buttons]
            .selectors
            .clone()
            .unwrap()
    );
    assert_eq!(fields.selectors[1].source, profile);
    assert_eq!(
        fields.selectors[1].input,
        HintSelectorInputField::Selector(1)
    );
    let duplicate = &trace[&HintSelectorFamily::Buttons].contributions[0].duplicates[0];
    assert_eq!((duplicate.input_index, duplicate.retained_index), (0, 0));
    assert_eq!(
        duplicate.retained_origin.source,
        seeds[&HintSelectorFamily::Buttons]
            .selectors
            .clone()
            .unwrap()
    );

    let mut cleared = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    cleared
        .apply_settings_with_origin(&input, "profile", &profile)
        .unwrap();
    let temporary = origin(PolicySourceIdentity::Temporary);
    let empty: BTreeMap<String, toml::Value> = BTreeMap::from([(
        "hints.selectors.buttons".into(),
        toml::from_str("mode='replace'").unwrap(),
    )]);
    cleared
        .apply_settings_with_origin(&empty, "temporary", &temporary)
        .unwrap();
    let (policies, trace) = cleared.finish_with_trace().unwrap();
    assert!(
        policies[&HintSelectorFamily::Buttons]
            .policy
            .as_ref()
            .unwrap()
            .selectors
            .is_empty()
    );
    let fields = &trace[&HintSelectorFamily::Buttons].fields;
    assert!(fields.selectors.is_empty());
    assert_eq!(fields.mode.as_ref().unwrap().source, temporary);
    assert_eq!(
        fields.empty_list.as_ref().unwrap().input,
        HintSelectorInputField::DefaultSelectors
    );
    assert_eq!(fields.empty_list.as_ref().unwrap().source, temporary);
    assert_eq!(trace[&HintSelectorFamily::Buttons].contributions.len(), 2);
    assert_eq!(
        trace[&HintSelectorFamily::Buttons].contributions[0].duplicates[0]
            .retained_origin
            .source,
        seeds[&HintSelectorFamily::Buttons]
            .selectors
            .clone()
            .unwrap()
    );
}

#[test]
fn empty_extend_does_not_claim_inherited_empty_array_or_mode() {
    let (base, seeds) = seeded(Vec::new());
    let mut builder = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    builder
        .apply_settings_with_origin(
            &settings("extend", &[]),
            "temporary",
            &origin(PolicySourceIdentity::Temporary),
        )
        .unwrap();
    let (policies, trace) = builder.finish_with_trace().unwrap();
    assert_eq!(
        policies[&HintSelectorFamily::Buttons]
            .policy
            .as_ref()
            .unwrap()
            .mode,
        HintSelectorMode::Replace
    );
    let trace = &trace[&HintSelectorFamily::Buttons];
    assert_eq!(trace.seed, trace.fields);
    assert_eq!(
        trace.fields.empty_list.as_ref().unwrap().input,
        HintSelectorInputField::Selectors
    );
    assert_eq!(
        trace.contributions[0].outcome,
        HintSelectorContributionOutcome::Applied
    );
}

#[test]
fn overflow_is_unavailable_and_replacement_recovers_without_false_origins() {
    let (base, mut seeds) = seeded((0..32).map(|index| format!("#base-{index}")).collect());
    let seed = origin(PolicySourceIdentity::InMemory);
    seeds.insert(
        HintSelectorFamily::Buttons,
        HintSelectorSeedOrigins {
            mode: seed.clone(),
            selectors: Some(seed),
        },
    );
    let input = settings("extend", &["#overflow"]);
    let mut rejected = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    rejected
        .apply_settings_with_origin(
            &input,
            "generated",
            &origin(PolicySourceIdentity::Generated),
        )
        .unwrap();
    assert!(rejected.finish_with_trace().is_err());
    let mut repaired = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    repaired
        .apply_settings_with_origin(
            &input,
            "generated",
            &origin(PolicySourceIdentity::Generated),
        )
        .unwrap();
    repaired
        .apply_settings_with_origin(
            &input,
            "command-line",
            &origin(PolicySourceIdentity::CommandLine),
        )
        .unwrap();
    let temporary = origin(PolicySourceIdentity::Temporary);
    repaired
        .apply_settings_with_origin(&settings("replace", &["#repair"]), "temporary", &temporary)
        .unwrap();
    let (policies, trace) = repaired.finish_with_trace().unwrap();
    assert_eq!(
        policies[&HintSelectorFamily::Buttons]
            .policy
            .as_ref()
            .unwrap()
            .selectors,
        ["#repair"]
    );
    let trace = &trace[&HintSelectorFamily::Buttons];
    assert!(matches!(
        trace.contributions[0].outcome,
        HintSelectorContributionOutcome::EffectiveOverflow(_)
    ));
    assert_eq!(
        trace.contributions[1].outcome,
        HintSelectorContributionOutcome::BlockedByEarlierOverflow
    );
    assert_eq!(trace.fields.selectors.len(), 1);
    assert_eq!(trace.fields.selectors[0].source, temporary);
}

#[test]
fn unconfigured_empty_setting_records_explicit_and_defaulted_lists_separately() {
    for explicit in [false, true] {
        let mut builder =
            HintSelectorPolicyBuilder::from_base_traced(&BTreeMap::new(), "base", &BTreeMap::new())
                .unwrap();
        let mut input = settings("extend", &[]);
        if !explicit {
            input
                .get_mut("hints.selectors.buttons")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .remove("selectors");
        }
        let temporary = origin(PolicySourceIdentity::Temporary);
        builder
            .apply_settings_with_origin(&input, "temporary", &temporary)
            .unwrap();
        let (_, trace) = builder.finish_with_trace().unwrap();
        let fields = &trace[&HintSelectorFamily::Buttons].fields;
        assert_eq!(fields.mode.as_ref().unwrap().source, temporary);
        assert_eq!(
            fields.empty_list.as_ref().unwrap().input,
            if explicit {
                HintSelectorInputField::Selectors
            } else {
                HintSelectorInputField::DefaultSelectors
            }
        );
    }
}

#[test]
fn aggregate_byte_overflow_never_returns_a_trace_with_partial_winners() {
    let mut builder =
        HintSelectorPolicyBuilder::from_base_traced(&BTreeMap::new(), "base", &BTreeMap::new())
            .unwrap();
    let mut plain = HintSelectorPolicyBuilder::from_base(&BTreeMap::new(), "base").unwrap();
    for family in [
        HintSelectorFamily::Buttons,
        HintSelectorFamily::Images,
        HintSelectorFamily::Media,
    ] {
        let selectors = (0..16)
            .map(|index| format!("#{}-{index}-{}", family.as_str(), "x".repeat(950)))
            .collect::<Vec<_>>();
        let policy = HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors,
        };
        let input = BTreeMap::from([(
            format!("hints.selectors.{}", family.as_str()),
            toml::Value::try_from(policy).unwrap(),
        )]);
        builder
            .apply_settings_with_origin(
                &input,
                "temporary",
                &origin(PolicySourceIdentity::Temporary),
            )
            .unwrap();
        plain.apply_settings(&input, "temporary").unwrap();
    }
    assert!(builder.finish_with_trace().is_err());
    assert!(plain.finish().is_err());
}

#[test]
fn raw_rejection_is_atomic_for_fields_and_trace_and_originless_updates_reject() {
    let (base, seeds) = seeded(vec!["#original".into()]);
    let expected = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds)
        .unwrap()
        .finish_with_trace()
        .unwrap();
    let mut builder = HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).unwrap();
    let mut input = settings("replace", &["#new"]);
    input.insert(
        "hints.selectors.media".into(),
        toml::from_str("mode='invalid'").unwrap(),
    );
    assert!(
        builder
            .apply_settings_with_origin(
                &input,
                "temporary",
                &origin(PolicySourceIdentity::Temporary)
            )
            .is_err()
    );
    assert!(
        builder
            .apply_settings(&settings("extend", &["#untracked"]), "missing-origin")
            .is_err()
    );
    assert_eq!(builder.finish_with_trace().unwrap(), expected);
}

#[test]
fn incomplete_or_wrong_layer_base_origins_reject() {
    let (base, mut seeds) = seeded(vec!["#original".into()]);
    assert!(HintSelectorPolicyBuilder::from_base_traced(&base, "base", &BTreeMap::new()).is_err());
    seeds
        .get_mut(&HintSelectorFamily::Buttons)
        .unwrap()
        .selectors = None;
    assert!(HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).is_err());
    seeds
        .get_mut(&HintSelectorFamily::Buttons)
        .unwrap()
        .selectors = Some(origin(PolicySourceIdentity::Temporary));
    assert!(HintSelectorPolicyBuilder::from_base_traced(&base, "base", &seeds).is_err());
}
