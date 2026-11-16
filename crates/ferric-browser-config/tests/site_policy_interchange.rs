use ferric_browser_config::{
    MAX_SITE_POLICY_INTERCHANGE_BYTES, SitePolicyExchange, SitePolicyExchangeOperation,
};

const HEADER: &str = "format='ferric-site-interaction'\nformat_version=1\n";
const UPSERT: &str = "[[rules]]\noperation='upsert-interaction'\nid='navigation'\npattern='https://example.test/*'\npriority=2\n[rules.interaction]\nenabled=true\n[[rules.interaction.anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'\n";
const UNSET: &str =
    "[[rules]]\noperation='unset-interaction'\nid='remove'\npattern='https://example.test/*'\n";

#[test]
fn shipped_local_examples_are_valid_constrained_round_trips() {
    for source in [
        include_str!("../../../examples/site-policies/navigation.toml"),
        include_str!("../../../examples/site-policies/navigation-two-character.toml"),
        include_str!("../../../examples/site-policies/visible-list.toml"),
        include_str!("../../../examples/site-policies/grid-sidebar.toml"),
        include_str!("../../../examples/site-policies/shadow-grid.toml"),
        include_str!("../../../examples/site-policies/player-remap.toml"),
    ] {
        let parsed = SitePolicyExchange::parse(source.as_bytes()).unwrap();
        assert!(parsed.unsupported_capabilities().is_empty());
        assert_eq!(
            SitePolicyExchange::parse(parsed.encode().unwrap().as_bytes()).unwrap(),
            parsed
        );
        assert!(parsed.rules().iter().all(|rule| match rule {
            SitePolicyExchangeOperation::UpsertInteraction { pattern, .. }
            | SitePolicyExchangeOperation::UnsetInteraction { pattern, .. } =>
                pattern.contains(".example.test/"),
        }));
    }
}

#[test]
fn everyday_site_presets_cover_the_default_actions_hint_family() {
    use ferric_browser_config::{InteractionFamily, InteractionRemapContext};

    for source in [
        include_str!("../../../examples/site-policies/presets/youtube-browse.toml"),
        include_str!("../../../examples/site-policies/presets/youtube-watch.toml"),
        include_str!("../../../examples/site-policies/presets/reddit-reading.toml"),
    ] {
        let parsed = SitePolicyExchange::parse(source.as_bytes()).unwrap();
        assert!(parsed.unsupported_capabilities().is_empty());
        assert_eq!(
            SitePolicyExchange::parse(parsed.encode().unwrap().as_bytes()).unwrap(),
            parsed
        );
        for rule in parsed.rules() {
            let SitePolicyExchangeOperation::UpsertInteraction {
                id, interaction, ..
            } = rule
            else {
                panic!("starter presets must upsert policies")
            };
            if id == "youtube-watch" {
                assert!(
                    interaction
                        .hint_page_keys
                        .as_ref()
                        .unwrap()
                        .iter()
                        .any(|key| key == "p")
                );
                let player = interaction
                    .remaps
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|remap| remap.from == "p")
                    .unwrap();
                assert_eq!(player.to, "k");
                assert!(
                    player
                        .contexts
                        .contains(&InteractionRemapContext::NormalPageKey)
                );
                assert!(
                    player
                        .contexts
                        .contains(&InteractionRemapContext::HintPageKey)
                );
            }
            for anchor in interaction.anchors.iter().flatten() {
                assert!(anchor.families.contains(&InteractionFamily::Actions));
            }
            for collection in interaction.collections.iter().flatten() {
                assert!(collection.families.contains(&InteractionFamily::Actions));
            }
        }
    }
}

#[test]
fn video_grid_and_player_examples_leave_hint_page_keys_available() {
    let grid = SitePolicyExchange::parse(include_bytes!(
        "../../../examples/site-policies/grid-sidebar.toml"
    ))
    .unwrap();
    let player = SitePolicyExchange::parse(include_bytes!(
        "../../../examples/site-policies/player-remap.toml"
    ))
    .unwrap();
    let SitePolicyExchangeOperation::UpsertInteraction {
        interaction: grid, ..
    } = &grid.rules()[0]
    else {
        panic!("grid example must upsert")
    };
    let SitePolicyExchangeOperation::UpsertInteraction {
        interaction: player,
        ..
    } = &player.rules()[0]
    else {
        panic!("player example must upsert")
    };
    for collection in grid.collections.as_ref().unwrap() {
        for code in &collection.codes {
            for source in player.hint_page_keys.as_ref().unwrap() {
                assert!(
                    !code.contains(source.as_str()),
                    "{code} consumes page source {source}"
                );
            }
        }
    }
}

#[test]
fn production_parser_round_trips_mixed_operations_and_derives_omitted_requirements() {
    let parsed = SitePolicyExchange::parse(format!("{HEADER}{UPSERT}{UNSET}").as_bytes()).unwrap();
    assert_eq!(parsed.rules().len(), 2);
    assert_eq!(parsed.rules()[1].id(), "remove");
    assert_eq!(
        parsed.required_capabilities(),
        vec!["interaction-policy-v1", "semantic-anchors-v1"]
    );
    assert!(parsed.unsupported_capabilities().is_empty());
    let encoded = parsed.encode().unwrap();
    assert_eq!(
        SitePolicyExchange::parse(encoded.as_bytes()).unwrap(),
        parsed
    );
    let raw: toml::Value = toml::from_str(&encoded).unwrap();
    let unset = &raw["rules"][1];
    assert!(unset.get("interaction").is_none() && unset.get("priority").is_none());
}

#[test]
fn closed_format_rejects_configuration_authority_and_ambiguous_operations() {
    for text in [
        format!("{HEADER}include=['elsewhere.toml']\n{UPSERT}"),
        format!("{HEADER}permissions=[]\n{UPSERT}"),
        format!("{HEADER}metadata={{anything='value'}}\n{UPSERT}"),
        format!(
            "{HEADER}{}",
            UPSERT.replace("priority=2", "priority=2\nset={'content.zoom'=2}")
        ),
        format!(
            "{HEADER}{}",
            UNSET.replace(
                "operation='unset-interaction'",
                "operation='unset-interaction'\npriority=1"
            )
        ),
        format!("{HEADER}{UNSET}[rules.interaction]\nenabled=false\n"),
        format!("{HEADER}{}", UPSERT.replace("priority=2\n", "")),
        format!(
            "{HEADER}{}",
            UPSERT.replace("upsert-interaction", "delete-rule")
        ),
        format!(
            "{HEADER}{UPSERT}{}",
            UNSET.replace("id='remove'", "id='navigation'")
        ),
        format!(
            "{HEADER}{}",
            UPSERT.replace(
                "target_selector='#main'",
                "target_selector='#main'\nscript='alert(1)'"
            )
        ),
        format!(
            "{HEADER}{}",
            UPSERT.replace("https://example.test/*", "file:///etc/*")
        ),
        format!(
            "{}{}",
            HEADER.replace("format_version=1", "format_version=2"),
            UPSERT
        ),
    ] {
        assert!(
            SitePolicyExchange::parse(text.as_bytes()).is_err(),
            "accepted {text}"
        );
    }
}

#[test]
fn unavailable_requirements_are_preserved_for_explicit_draft_diagnostics() {
    let text = format!("{HEADER}required_capabilities=['future-v2']\n{UPSERT}");
    let document = SitePolicyExchange::parse(text.as_bytes()).unwrap();
    assert_eq!(document.unsupported_capabilities(), vec!["future-v2"]);
    let round_trip = SitePolicyExchange::parse(document.encode().unwrap().as_bytes()).unwrap();
    assert_eq!(document, round_trip);
    assert!(
        document
            .required_capabilities()
            .contains(&"semantic-anchors-v1".into())
    );
    let duplicate = format!(
        "{HEADER}required_capabilities=['interaction-policy-v1','interaction-policy-v1']\n{UNSET}"
    );
    assert!(SitePolicyExchange::parse(duplicate.as_bytes()).is_err());
}

#[test]
fn byte_operation_and_metadata_limits_reject_before_import() {
    let bytes = vec![b' '; usize::try_from(MAX_SITE_POLICY_INTERCHANGE_BYTES).unwrap() + 1];
    assert!(
        SitePolicyExchange::parse(&bytes)
            .unwrap_err()
            .contains("2 MiB")
    );
    assert!(SitePolicyExchange::parse(&[255]).is_err());
    assert!(SitePolicyExchange::parse(HEADER.as_bytes()).is_err());
    let rules = (0..256)
        .map(|index| SitePolicyExchangeOperation::UnsetInteraction {
            id: format!("rule-{index}"),
            pattern: "https://example.test/*".into(),
        })
        .collect::<Vec<_>>();
    assert!(SitePolicyExchange::new(rules.clone()).is_ok());
    let mut too_many = rules;
    too_many.push(SitePolicyExchangeOperation::UnsetInteraction {
        id: "last".into(),
        pattern: "https://example.test/*".into(),
    });
    assert!(SitePolicyExchange::new(too_many).is_err());
    let requirements = (0..16)
        .map(|index| format!("'future-{index}'"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        SitePolicyExchange::parse(
            format!("{HEADER}required_capabilities=[{requirements}]\n{UPSERT}").as_bytes()
        )
        .is_err()
    );
}
