use ferric_browser_core::{
    CommandRegistry, Mode, ModeEnterError, ModeEnterOptions, complete, parse_mode_enter,
};

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn raw_forms_accept_existing_mode_aliases_only() {
    for (name, mode) in [
        ("passthrough", Mode::PassThrough),
        ("pass-through", Mode::PassThrough),
        ("passthrough-once", Mode::PassThroughOnce),
        ("pass-through-once", Mode::PassThroughOnce),
    ] {
        for raw in [false, true] {
            let mut args = arguments(&[name]);
            if raw {
                args.push("--raw".into());
            }
            assert_eq!(parse_mode_enter(&args), Ok(ModeEnterOptions { mode, raw }));
        }
    }
    for name in ["normal", "insert", "insert-locked", "caret"] {
        assert!(!parse_mode_enter(&arguments(&[name])).unwrap().raw);
        assert_eq!(
            parse_mode_enter(&arguments(&[name, "--raw"])),
            Err(ModeEnterError::RawRequiresPassthrough)
        );
    }
    for values in [
        vec!["passthrough", "--raw", "--raw"],
        vec!["passthrough", "--unknown"],
        vec!["--raw", "passthrough"],
        vec!["passthrough", "extra"],
    ] {
        assert_eq!(
            parse_mode_enter(&arguments(&values)),
            Err(ModeEnterError::InvalidArguments)
        );
    }
    assert_eq!(parse_mode_enter(&[]), Err(ModeEnterError::MissingMode));
    assert_eq!(
        parse_mode_enter(&arguments(&["hint"])),
        Err(ModeEnterError::UnknownMode)
    );
}

#[test]
fn raw_completion_follows_a_passthrough_mode_and_is_not_duplicated() {
    let registry = CommandRegistry::default_v1();
    for input in [
        "mode-enter passthrough ",
        "mode-enter passthrough-once --",
        "mode-enter pass-through ",
    ] {
        let result = complete(input, input.chars().count(), &registry, &[], 100);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.insert_text.as_str())
                .collect::<Vec<_>>(),
            ["--raw"]
        );
    }
    for input in ["mode-enter insert ", "mode-enter passthrough --raw "] {
        assert!(
            complete(input, input.chars().count(), &registry, &[], 100)
                .candidates
                .is_empty()
        );
    }
}

#[test]
fn command_alias_expansion_retains_raw_arguments() {
    let definitions = CommandRegistry::default_v1().definitions().to_vec();
    let alias = ferric_browser_core::CommandAlias::from_text(
        "raw-key",
        "mode-enter passthrough-once --raw",
    )
    .unwrap();
    let registry = CommandRegistry::new_with_aliases(definitions, vec![alias]).unwrap();
    let expanded = registry
        .expand_command(ferric_browser_core::ParsedCommand {
            name: "raw-key".into(),
            arguments: vec![],
        })
        .unwrap();
    assert_eq!(expanded.len(), 1);
    assert_eq!(
        parse_mode_enter(&expanded[0].arguments),
        Ok(ModeEnterOptions {
            mode: Mode::PassThroughOnce,
            raw: true
        })
    );
}
