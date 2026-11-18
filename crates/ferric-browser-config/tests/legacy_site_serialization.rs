//! Expected bytes were emitted by the pinned pre-Interaction configuration
//! crate, not by this test's implementation. See fixtures/site-policy/legacy-v3.
use ferric_browser_config::{Config, RuntimeOverrides, validate};

#[test]
fn old_v3_authored_and_default_config_serialization_is_byte_identical() {
    let config: Config =
        toml::from_str(include_str!("fixtures/site-policy/legacy-v3/input.toml")).unwrap();
    validate(&config).unwrap();
    assert_eq!(
        toml::to_string(&config).unwrap(),
        include_str!("fixtures/site-policy/legacy-v3/config.toml")
    );
    assert_eq!(
        toml::to_string(&Config::default()).unwrap(),
        include_str!("fixtures/site-policy/legacy-v3/defaults.toml")
    );
}

#[test]
fn old_v3_generated_runtime_serialization_is_byte_identical() {
    let mut overrides = RuntimeOverrides::default();
    overrides.set_literal("hints.min_chars", "2").unwrap();
    overrides
        .set_site_literal("https://example.test/*", "content.zoom", "1.25")
        .unwrap();
    let expected = include_str!("fixtures/site-policy/legacy-v3/runtime.toml");
    assert_eq!(toml::to_string(&overrides).unwrap(), expected);
    assert_eq!(
        toml::from_str::<RuntimeOverrides>(expected).unwrap(),
        overrides
    );
}
