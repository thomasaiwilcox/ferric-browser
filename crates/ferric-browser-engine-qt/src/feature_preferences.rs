//! Typed operational-preference projection for QML.
//!
//! These scalar settings control presentation integration only.  Keeping them
//! as explicit properties prevents QML from reparsing the complete
//! configuration graph for routine UI decisions.

use ferric_browser_config::Config;

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct FeaturePreferences {
    pub switcher_max_results: i32,
    pub downloads_ask_destination: bool,
    pub desktop_notifications_enabled: bool,
    pub desktop_media_keys_enabled: bool,
    pub push_service_enabled: bool,
    pub spellcheck_enabled: bool,
    pub spellcheck_languages: Vec<String>,
    pub blocking_list_ids: Vec<String>,
    pub blocking_update_interval_hours: i32,
    pub link_cleaning_update_source: String,
    pub link_cleaning_update_sha256: String,
}

#[must_use]
pub(super) fn project(config: &Config) -> FeaturePreferences {
    FeaturePreferences {
        switcher_max_results: i32::try_from(config.switcher.max_results.clamp(10, 1_000))
            .unwrap_or(1_000),
        downloads_ask_destination: config.downloads.ask_destination,
        desktop_notifications_enabled: config.desktop.notifications,
        desktop_media_keys_enabled: config.desktop.media_keys,
        push_service_enabled: config.privacy.push_service,
        spellcheck_enabled: config.spellcheck.enabled,
        spellcheck_languages: config.spellcheck.languages.clone(),
        blocking_list_ids: config.blocking.lists.clone(),
        blocking_update_interval_hours: i32::try_from(
            config.blocking.update_interval_hours.clamp(1, 168),
        )
        .unwrap_or(168),
        link_cleaning_update_source: config
            .links
            .cleaning
            .update_source
            .clone()
            .unwrap_or_default(),
        link_cleaning_update_sha256: config
            .links
            .cleaning
            .update_sha256
            .clone()
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_bounds_ui_counts_and_preserves_validated_feature_flags() {
        let mut config = Config::default();
        config.switcher.max_results = u32::MAX;
        config.blocking.update_interval_hours = u32::MAX;
        config.downloads.ask_destination = false;
        config.desktop.notifications = false;
        config.desktop.media_keys = false;
        config.privacy.push_service = true;
        config.spellcheck.enabled = false;
        config.spellcheck.languages = vec!["en-GB".into(), "de".into()];
        config.blocking.lists = vec!["easylist".into()];
        config.links.cleaning.update_source = Some("https://example.test/rules.json".into());
        config.links.cleaning.update_sha256 = Some("abc123".into());

        assert_eq!(
            project(&config),
            FeaturePreferences {
                switcher_max_results: 1_000,
                downloads_ask_destination: false,
                desktop_notifications_enabled: false,
                desktop_media_keys_enabled: false,
                push_service_enabled: true,
                spellcheck_enabled: false,
                spellcheck_languages: vec!["en-GB".into(), "de".into()],
                blocking_list_ids: vec!["easylist".into()],
                blocking_update_interval_hours: 168,
                link_cleaning_update_source: "https://example.test/rules.json".into(),
                link_cleaning_update_sha256: "abc123".into(),
            }
        );
    }
}
