//! Numeric product contracts from implementation-plan Section 3.12. These
//! assertions freeze shared definitions; they do not qualify the later DOM,
//! input, draft/capture or interchange services that must enforce them.

use ferric_browser_application as application;
use ferric_browser_config as config;
use ferric_browser_core as core;

#[test]
fn shared_limits_match_the_independent_product_contract() {
    for (name, actual, required) in [
        ("ordinary candidates", core::MAX_HINT_CANDIDATES, 5_000),
        ("DOM inspections", core::MAX_HINT_INSPECTIONS, 50_000),
        ("shared root depth", core::MAX_HINT_ROOT_DEPTH, 8),
        ("early Hint intents", core::MAX_HINT_EARLY_INTENTS, 256),
        (
            "queued native events",
            core::MAX_HINT_QUEUED_NATIVE_EVENTS,
            0,
        ),
        (
            "minimum fallback alphabet",
            core::MIN_HINT_ALPHABET_CHARS,
            2,
        ),
        (
            "maximum fallback alphabet",
            core::MAX_HINT_ALPHABET_CHARS,
            32,
        ),
        ("region facets", core::MAX_HINT_REGIONS, 256),
        ("surface facets", core::MAX_HINT_SURFACES, 256),
        (
            "drafts per profile",
            application::MAX_SITE_POLICY_DRAFTS_PER_PROFILE,
            16,
        ),
        (
            "simultaneous previews",
            application::MAX_SITE_POLICY_PREVIEWS,
            8,
        ),
        (
            "captures per window",
            application::MAX_SITE_POLICY_CAPTURES_PER_WINDOW,
            1,
        ),
        (
            "import rule operations",
            config::MAX_SITE_POLICY_IMPORT_OPERATIONS,
            256,
        ),
    ] {
        assert_eq!(actual, required, "{name}");
    }
    assert_eq!(config::MAX_SITE_POLICY_INTERCHANGE_BYTES, 2 * 1024 * 1024);
}

#[test]
fn consumer_limits_share_the_existing_outer_bounds() {
    assert_eq!(
        config::MAX_INTERACTION_ROOT_STEPS,
        core::MAX_HINT_ROOT_DEPTH
    );
    assert_eq!(
        config::MIN_INTERACTION_FALLBACK_ALPHABET_CHARS,
        core::MIN_HINT_ALPHABET_CHARS
    );
    assert_eq!(
        config::MAX_INTERACTION_FALLBACK_ALPHABET_CHARS,
        core::MAX_HINT_ALPHABET_CHARS
    );
    assert_eq!(
        config::MAX_SITE_POLICY_IMPORT_OPERATIONS,
        config::MAX_SITE_RULES
    );
    assert_eq!(
        config::MAX_SITE_POLICY_INTERCHANGE_BYTES,
        config::MAX_CONFIGURATION_BYTES
    );
}
