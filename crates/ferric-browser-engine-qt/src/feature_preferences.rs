//! Typed operational-preference projection for QML.
//!
//! These scalar settings control presentation integration only.  Keeping them
//! as explicit properties prevents QML from reparsing the complete
//! configuration graph for routine UI decisions.

use ferric_browser_config::{
    Config, HotTabPolicy, PageGroupingPolicy, PhonePasskeyPolicy, TabsPresentation,
};

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
    pub tabs_presentation_requested: String,
    pub tabs_presentation_effective: String,
    pub tabs_presentation_status: String,
    pub tabs_presentation_reason: String,
    pub hot_tab_requested: String,
    pub hot_tab_status: String,
    pub hot_tab_reason: String,
    pub phone_passkeys_requested: String,
    pub phone_passkeys_status: String,
    pub phone_passkeys_reason: String,
    pub page_grouping_requested: String,
    pub page_grouping_status: String,
    pub page_grouping_reason: String,
}

#[must_use]
pub(super) fn project(effective: &Config, requested: &Config) -> FeaturePreferences {
    FeaturePreferences {
        switcher_max_results: i32::try_from(effective.switcher.max_results.clamp(10, 1_000))
            .unwrap_or(1_000),
        downloads_ask_destination: effective.downloads.ask_destination,
        desktop_notifications_enabled: effective.desktop.notifications,
        desktop_media_keys_enabled: effective.desktop.media_keys,
        push_service_enabled: effective.privacy.push_service,
        spellcheck_enabled: effective.spellcheck.enabled,
        spellcheck_languages: effective.spellcheck.languages.clone(),
        blocking_list_ids: effective.blocking.lists.clone(),
        blocking_update_interval_hours: i32::try_from(
            effective.blocking.update_interval_hours.clamp(1, 168),
        )
        .unwrap_or(168),
        link_cleaning_update_source: effective
            .links
            .cleaning
            .update_source
            .clone()
            .unwrap_or_default(),
        link_cleaning_update_sha256: effective
            .links
            .cleaning
            .update_sha256
            .clone()
            .unwrap_or_default(),
        tabs_presentation_requested: tabs_presentation_name(&requested.tabs.presentation).into(),
        tabs_presentation_effective: tabs_presentation_name(&effective.tabs.presentation).into(),
        tabs_presentation_status: presentation_status(effective, requested).into(),
        tabs_presentation_reason: presentation_reason(effective, requested).into(),
        hot_tab_requested: hot_tab_name(&requested.performance.hot_tab).into(),
        hot_tab_status: if requested.performance.hot_tab == HotTabPolicy::Off {
            "disabled"
        } else {
            "unqualified"
        }
        .into(),
        hot_tab_reason: if requested.performance.hot_tab == HotTabPolicy::Off {
            "disabled by setting"
        } else {
            "no qualified hot-tab consumer is registered"
        }
        .into(),
        phone_passkeys_requested: phone_passkeys_name(&requested.passkeys.phone).into(),
        phone_passkeys_status: if requested.passkeys.phone == PhonePasskeyPolicy::Off {
            "disabled"
        } else {
            "unqualified"
        }
        .into(),
        phone_passkeys_reason: if requested.passkeys.phone == PhonePasskeyPolicy::Off {
            "disabled by setting; native authentication remains in control"
        } else {
            "no qualified phone-auth consumer is registered"
        }
        .into(),
        page_grouping_requested: page_grouping_name(&requested.hyprland.page_grouping).into(),
        page_grouping_status: if requested.hyprland.page_grouping == PageGroupingPolicy::Off {
            "disabled"
        } else {
            "unqualified"
        }
        .into(),
        page_grouping_reason: if requested.hyprland.page_grouping == PageGroupingPolicy::Off {
            "disabled by setting"
        } else {
            "no qualified Hyprland page-grouping consumer is registered"
        }
        .into(),
    }
}

fn tabs_presentation_name(value: &TabsPresentation) -> &'static str {
    match value {
        TabsPresentation::Internal => "internal",
        TabsPresentation::Windows => "windows",
    }
}

fn hot_tab_name(value: &HotTabPolicy) -> &'static str {
    match value {
        HotTabPolicy::Off => "off",
        HotTabPolicy::Auto => "auto",
        HotTabPolicy::On => "on",
    }
}

fn phone_passkeys_name(value: &PhonePasskeyPolicy) -> &'static str {
    match value {
        PhonePasskeyPolicy::Off => "off",
        PhonePasskeyPolicy::On => "on",
    }
}

fn page_grouping_name(value: &PageGroupingPolicy) -> &'static str {
    match value {
        PageGroupingPolicy::Off => "off",
        PageGroupingPolicy::Auto => "auto",
    }
}

fn presentation_status(effective: &Config, requested: &Config) -> &'static str {
    if effective.tabs.presentation != requested.tabs.presentation {
        "pending-restart"
    } else if effective.tabs.presentation == TabsPresentation::Windows {
        "unqualified"
    } else {
        "active"
    }
}

fn presentation_reason(effective: &Config, requested: &Config) -> &'static str {
    if effective.tabs.presentation != requested.tabs.presentation {
        "the requested presentation applies after browser restart"
    } else if effective.tabs.presentation == TabsPresentation::Windows {
        "native page-window presentation has no accepted capability proof"
    } else {
        "internal tabs are the active presentation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::{BrowserApplication, ConfigurationLayers};

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
            project(&config, &config),
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
                tabs_presentation_requested: "internal".into(),
                tabs_presentation_effective: "internal".into(),
                tabs_presentation_status: "active".into(),
                tabs_presentation_reason: "internal tabs are the active presentation".into(),
                hot_tab_requested: "off".into(),
                hot_tab_status: "disabled".into(),
                hot_tab_reason: "disabled by setting".into(),
                phone_passkeys_requested: "off".into(),
                phone_passkeys_status: "disabled".into(),
                phone_passkeys_reason:
                    "disabled by setting; native authentication remains in control".into(),
                page_grouping_requested: "off".into(),
                page_grouping_status: "disabled".into(),
                page_grouping_reason: "disabled by setting".into(),
            }
        );
    }

    #[test]
    fn presentation_request_remains_pending_without_changing_active_routing() {
        let mut app = BrowserApplication::new(Config::default());
        let mut requested = Config::default();
        requested.tabs.presentation = TabsPresentation::Windows;

        let prepared = app
            .prepare_configuration_update(ConfigurationLayers::new(requested.clone()))
            .expect("presentation request prepares");
        let pending = prepared.snapshot();
        assert_eq!(
            pending.effective.tabs.presentation,
            TabsPresentation::Internal
        );
        assert!(pending.pending_changes.iter().any(|change| {
            change.key == "tabs.presentation"
                && change.apply_time == "restart"
                && change.current == toml::Value::String("internal".into())
                && change.pending == toml::Value::String("windows".into())
        }));

        let preferences = project(&pending.effective, &requested);
        assert_eq!(preferences.tabs_presentation_requested, "windows");
        assert_eq!(preferences.tabs_presentation_effective, "internal");
        assert_eq!(preferences.tabs_presentation_status, "pending-restart");
        assert_eq!(
            preferences.tabs_presentation_reason,
            "the requested presentation applies after browser restart"
        );
        assert_eq!(
            crate::settings_presentation::configuration_status(
                "Settings saved",
                &pending.pending_changes,
            ),
            "Settings saved; some changes need a browser restart"
        );
        assert_eq!(preferences.hot_tab_status, "disabled");
        assert_eq!(preferences.phone_passkeys_status, "disabled");
        assert_eq!(preferences.page_grouping_status, "disabled");

        let published = app
            .publish_configuration_update(prepared, || Ok(()))
            .expect("pending request publishes without changing active routing");
        assert_eq!(
            published.effective.tabs.presentation,
            TabsPresentation::Internal
        );
        assert_eq!(
            app.configuration().tabs.presentation,
            TabsPresentation::Internal
        );
    }
}
