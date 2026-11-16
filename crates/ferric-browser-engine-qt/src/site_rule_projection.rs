//! Independent projections of the selected application's site settings.

use ferric_browser_config::{Config, matching_site_rules};

pub(super) struct SiteRuleProjection {
    pub javascript: Option<bool>,
    pub images: Option<bool>,
    pub force_dark: Option<bool>,
    pub autoplay: Option<String>,
    pub zoom: Option<f64>,
}

pub(super) fn resolve(effective: &Config, url: &str) -> SiteRuleProjection {
    let mut projection = SiteRuleProjection {
        javascript: None,
        images: None,
        force_dark: None,
        autoplay: None,
        zoom: None,
    };
    // A failed input consumer cannot erase independent ordinary site settings.
    for rule in matching_site_rules(effective, url) {
        for (key, value) in &rule.set {
            match key.as_str() {
                "content.javascript" => projection.javascript = value.as_bool(),
                "content.images" => projection.images = value.as_bool(),
                "content.force_dark" => projection.force_dark = value.as_bool(),
                "content.autoplay" => projection.autoplay = value.as_str().map(ToOwned::to_owned),
                "content.zoom" => {
                    projection.zoom = value.as_float().or_else(|| {
                        value
                            .as_integer()
                            .and_then(|value| i32::try_from(value).ok())
                            .map(f64::from)
                    });
                }
                _ => {}
            }
        }
    }
    projection
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_policy_runtime::{MixedPolicyState, resolve_for_layers};
    use ferric_browser_application::ConfigurationLayers;
    use ferric_browser_config::SiteRule;
    use std::collections::BTreeMap;

    #[test]
    fn historical_mixed_failure_keeps_ordinary_site_settings() {
        let mut layers = ConfigurationLayers::new(Config::default());
        // Explicit historical/fault injection, never public invalid admission.
        layers.base.site_rules.push(SiteRule {
            id: "historical".into(),
            pattern: "https://example.test/*".into(),
            priority: 0,
            interaction: None,
            set: BTreeMap::from([
                ("input.shortcut_policy".into(), toml::Value::Boolean(false)),
                ("content.zoom".into(), toml::Value::Float(1.25)),
                ("content.javascript".into(), toml::Value::Boolean(false)),
            ]),
        });
        let mixed = MixedPolicyState::from_result(resolve_for_layers(
            &layers.base,
            &layers.profile,
            &layers.runtime,
            &layers.command_line,
            &layers.temporary,
            &layers.base,
            "https://example.test/",
        ));
        assert_eq!(mixed.status(), "unavailable");
        assert!(mixed.policy().is_none());
        let matched = resolve(&layers.base, "https://example.test/");
        assert_eq!(matched.zoom, Some(1.25));
        assert_eq!(matched.javascript, Some(false));
        let next = resolve(&layers.base, "https://other.test/");
        assert_eq!(next.zoom, None);
        assert_eq!(next.javascript, None);
    }
}
