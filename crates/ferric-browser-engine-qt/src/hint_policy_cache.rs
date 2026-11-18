//! Prepared bridge values derived from the application's one policy capture.
//! Physical input reads only identities and implicitly shared Qt strings here.
use cxx_qt_lib::QString;
use ferric_browser_application::{
    BrowserApplication, CompiledSitePolicySnapshot, SitePolicyMatchContext,
};
use ferric_browser_core::WindowId;
use std::{collections::BTreeMap, sync::Arc};

const FAMILIES: [&str; 11] = [
    "all",
    "links",
    "inputs",
    "buttons",
    "images",
    "media",
    "scrollables",
    "regions",
    "surfaces",
    "actions",
    "focus",
];

#[derive(Clone)]
pub(super) struct CachedHintSelectorPolicy {
    pub(super) revision: u64,
    pub(super) revision_hex: String,
    pub(super) serialized: QString,
    pub(super) provenance: Arc<[String]>,
}

pub(super) struct CachedHintPolicies {
    snapshot: Arc<CompiledSitePolicySnapshot>,
    context: SitePolicyMatchContext,
    url: QString,
    owner_revision: u64,
    families: BTreeMap<&'static str, CachedHintSelectorPolicy>,
}

impl CachedHintPolicies {
    pub(super) fn preview_family(&self, family: &str) -> Option<CachedHintSelectorPolicy> {
        self.families.get(family).cloned()
    }
    pub(super) fn prepare(
        snapshot: Arc<CompiledSitePolicySnapshot>,
        context: SitePolicyMatchContext,
        url: QString,
        owner_revision: u64,
    ) -> Result<Self, String> {
        let selectors = snapshot.discovery().selectors()?;
        let policies = serde_json::to_string(selectors)
            .map_err(|error| format!("selector policy could not be serialized: {error}"))?;
        let mut families = BTreeMap::new();
        for family in FAMILIES {
            // The common semantic revision accounts for normalized match context,
            // values and provenance. Query/fragment changes add no new matcher.
            let identity = serde_json::to_vec(&(
                owner_revision,
                snapshot.revision().policy_revision,
                family,
                &policies,
            ))
            .map_err(|error| format!("selector identity could not be serialized: {error}"))?;
            let revision = identity
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
                });
            let revision_hex = format!("{revision:016x}");
            let serialized = QString::from(format!(
                r#"{{"family":"{family}","configuration_revision":{owner_revision},"selector_revision":"{revision_hex}","policies":{policies}}}"#
            ));
            families.insert(
                family,
                CachedHintSelectorPolicy {
                    revision,
                    revision_hex,
                    serialized,
                    provenance: ferric_browser_config::HintSelectorFamily::parse(family)
                        .and_then(|family| selectors.get(&family))
                        .map_or_else(
                            || Arc::from([]),
                            |entry| Arc::from(entry.provenance.clone()),
                        ),
                },
            );
        }
        Ok(Self {
            snapshot,
            context,
            url,
            owner_revision,
            families,
        })
    }

    pub(super) fn matches(
        &self,
        snapshot: &Arc<CompiledSitePolicySnapshot>,
        url: &QString,
        owner_revision: u64,
    ) -> bool {
        Arc::ptr_eq(&self.snapshot, snapshot)
            && self.url == *url
            && self.owner_revision == owner_revision
    }

    pub(super) fn get(
        &self,
        app: &BrowserApplication,
        window: WindowId,
        url: &QString,
        owner_revision: u64,
        family: &str,
    ) -> Result<CachedHintSelectorPolicy, String> {
        self.capture(app, window, url, owner_revision, family)
            .map(|(_, selectors)| selectors)
    }

    pub(super) fn capture(
        &self,
        app: &BrowserApplication,
        window: WindowId,
        url: &QString,
        owner_revision: u64,
        family: &str,
    ) -> Result<(Arc<CompiledSitePolicySnapshot>, CachedHintSelectorPolicy), String> {
        let current = app
            .cached_site_policy_snapshot(window, &self.context)
            .ok_or("site policy refresh is pending or its owner is unavailable")?;
        if !self.matches(current, url, owner_revision) {
            return Err("site policy capture is no longer current".into());
        }
        self.families
            .get(family)
            .cloned()
            .map(|selectors| (Arc::clone(&self.snapshot), selectors))
            .ok_or_else(|| format!("unsupported hint selector family: {family}"))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use ferric_browser_application::{ConfigurationLayers, ProfileOpenRequest, ProfileStorage};
    use ferric_browser_config::{Config, ContextsConfig, HintSelectorFamily, RuntimeOverrides};
    use ferric_browser_core::{PrivacyKind, ValidatedUrl};
    use std::time::{Duration, Instant};

    const URL: &str = "https://example.test/page";

    pub(crate) fn application() -> (BrowserApplication, WindowId) {
        let config = Config::default();
        let (mut app, window, _) =
            BrowserApplication::bootstrap(config.clone(), PrivacyKind::Ephemeral, "default")
                .unwrap();
        app.request_profile_open(ProfileOpenRequest {
            name: "default".into(),
            label: "default".into(),
            privacy: PrivacyKind::Ephemeral,
            storage: ProfileStorage::Transient,
            base_configuration: config,
            base_authored: Arc::default(),
            initial_profile_overrides: RuntimeOverrides::default(),
            command_line_overrides: RuntimeOverrides::default(),
            config_path: None,
            script_catalog_root: None,
            contexts: ContextsConfig::default(),
            context_configuration_error: None,
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = app.poll_profile_open() {
                result.unwrap();
                break;
            }
            assert!(
                Instant::now() < deadline,
                "transient test profile did not open"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        (app, window)
    }

    fn prepare(
        app: &mut BrowserApplication,
        window: WindowId,
        url: &str,
        owner: u64,
    ) -> CachedHintPolicies {
        let snapshot = app.refresh_site_policy_snapshot(window, url).unwrap();
        let context = SitePolicyMatchContext::for_url(&ValidatedUrl::parse(url).unwrap());
        CachedHintPolicies::prepare(snapshot, context, QString::from(url), owner).unwrap()
    }

    #[test]
    fn prepared_family_reads_never_compile_and_require_current_owner_and_capture() {
        let (mut app, window) = application();
        let url = QString::from(URL);
        let cache = prepare(&mut app, window, URL, 17);
        let count = app.interaction_compilation_count();
        let expected_policies =
            serde_json::to_value(cache.snapshot.discovery().selectors().unwrap()).unwrap();
        for family in FAMILIES {
            let first = cache.get(&app, window, &url, 17, family).unwrap();
            let parsed: serde_json::Value =
                serde_json::from_str(&first.serialized.to_string()).unwrap();
            assert_eq!(parsed["family"], family);
            assert_eq!(parsed["policies"], expected_policies);
            assert_eq!(parsed["configuration_revision"], 17);
            assert_eq!(parsed["selector_revision"], first.revision_hex);
            assert_eq!(
                u64::from_str_radix(&first.revision_hex, 16).unwrap(),
                first.revision
            );
            for _ in 0..20 {
                assert_eq!(
                    cache
                        .get(&app, window, &url, 17, family)
                        .unwrap()
                        .serialized,
                    first.serialized
                );
            }
        }
        assert_eq!(app.interaction_compilation_count(), count);
        assert!(cache.get(&app, window, &url, 18, "all").is_err());
        assert!(
            cache
                .get(
                    &app,
                    window,
                    &QString::from("https://other.test/"),
                    17,
                    "all"
                )
                .is_err()
        );
        assert!(cache.get(&app, window, &url, 17, "unknown").is_err());
        app.invalidate_interaction_snapshot();
        assert!(cache.get(&app, window, &url, 17, "all").is_err());
        let refreshed = prepare(&mut app, window, URL, 17);
        assert!(refreshed.get(&app, window, &url, 17, "all").is_ok());
        app.replace_configuration(Config::default()).unwrap();
        assert!(refreshed.get(&app, window, &url, 17, "all").is_err());
    }

    #[test]
    fn rejected_actual_document_closes_prepared_bridge_values() {
        let (mut app, window) = application();
        let cache = prepare(&mut app, window, URL, 5);
        let tab = app.windows()[&window].active_tab.unwrap();
        let observation = app
            .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
            .unwrap();
        assert!(app.reject_observed_interaction_context(&observation));
        assert!(
            cache
                .get(&app, window, &QString::from(URL), 5, "all")
                .is_err()
        );
        assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
    }

    #[test]
    fn global_only_discovery_is_prepared_and_selector_conflicts_are_explicit() {
        let (mut app, window) = application();
        let global = prepare(&mut app, window, "about:blank", 9);
        assert!(
            global
                .get(&app, window, &QString::from("about:blank"), 9, "buttons")
                .is_ok()
        );
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.base.hints.selectors.insert(
            HintSelectorFamily::Buttons,
            toml::from_str("mode='extend'\nselectors=['.base']").unwrap(),
        );
        for index in 0..2 {
            let mut rule: ferric_browser_config::SiteRule = toml::from_str(&format!(
                "id='rule{index}'\npattern='https://example.test/*'\npriority={index}"
            ))
            .unwrap();
            rule.set.insert(
                "hints.selectors.buttons".into(),
                toml::Value::Table(toml::Table::from_iter([
                    ("mode".into(), "extend".into()),
                    (
                        "selectors".into(),
                        toml::Value::Array(
                            (0..16)
                                .map(|item| format!(".rule{index}_{item}").into())
                                .collect(),
                        ),
                    ),
                ])),
            );
            layers.base.site_rules.push(rule);
        }
        let prepared = app.prepare_configuration_reload(layers).unwrap();
        app.publish_configuration_update(prepared, || Ok(()))
            .unwrap();
        let snapshot = app.refresh_site_policy_snapshot(window, URL).unwrap();
        assert!(snapshot.discovery().selectors().is_err());
        assert!(snapshot.discovery().proxy_targets().is_ok());
        let context = SitePolicyMatchContext::for_url(&ValidatedUrl::parse(URL).unwrap());
        assert!(CachedHintPolicies::prepare(snapshot, context, QString::from(URL), 9).is_err());
        assert!(
            global
                .get(&app, window, &QString::from("about:blank"), 9, "buttons")
                .is_err()
        );
    }
}
