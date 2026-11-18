//! Browser-owned evidence of an accepted `WebEngine` syntax callback. These
//! receipts confer no write/publication permission and no live-document proof.
use std::sync::Arc;

use ferric_browser_application::{
    BrowserApplication, ConfigurationLayers, ConfigurationRequestCapture, ProfileOpenCandidate,
    ProfileOpenValidationCapture,
};

use crate::selector_validation::{
    SelectorValidationInventory, SelectorValidationItem, selector_validation_inventory,
    selector_validation_items,
};

pub(super) struct SelectorValidationCapture {
    context: Arc<()>,
    owner: SelectorValidationOwner,
    generation: u64,
    inventory: SelectorValidationInventory,
}

enum SelectorValidationOwner {
    Active(Box<ConfigurationRequestCapture>),
    Profile(ProfileOpenValidationCapture),
}

pub(super) struct SelectorValidationReceipt(SelectorValidationCapture);

/// The fingerprint remains a scheduling cache, never a validation credential.
pub(super) struct SelectorValidationPublication {
    pub(super) fingerprint: String,
    pub(super) receipt: Option<Arc<SelectorValidationReceipt>>,
}

#[derive(Default)]
pub(super) struct SelectorValidationReceipts {
    // Stable browser-owned syntax-parser implementation context for this UI
    // lifetime. Navigation/recreation of its inert validation document does
    // not change Qt's parser. Another UI has another allocation; the separate
    // owner binding also prevents reuse across profiles in this UI.
    context: Arc<()>,
    active: Option<Arc<SelectorValidationReceipt>>,
    desired: Option<Arc<SelectorValidationReceipt>>,
}

impl SelectorValidationReceipts {
    pub(super) fn capture(
        &self,
        state: &BrowserApplication,
        layers: &ConfigurationLayers,
        generation: u64,
        items: &[SelectorValidationItem],
    ) -> Option<SelectorValidationCapture> {
        if items.is_empty() || items != selector_validation_items(layers) {
            return None;
        }
        Some(SelectorValidationCapture {
            context: Arc::clone(&self.context),
            owner: SelectorValidationOwner::Active(Box::new(
                state.capture_configuration_request()?,
            )),
            generation,
            inventory: selector_validation_inventory(layers),
        })
    }

    pub(super) fn capture_profile(
        &self,
        state: &BrowserApplication,
        candidate: &ProfileOpenCandidate,
        generation: u64,
        items: &[SelectorValidationItem],
    ) -> Option<SelectorValidationCapture> {
        if items.is_empty()
            || items != selector_validation_items(candidate.layers())
            || !state.profile_candidate_is_current(candidate)
        {
            return None;
        }
        Some(SelectorValidationCapture {
            context: Arc::clone(&self.context),
            owner: SelectorValidationOwner::Profile(candidate.validation_capture()),
            generation,
            inventory: selector_validation_inventory(candidate.layers()),
        })
    }

    /// Called exclusively after the callback's owner, operation, generation,
    /// revision, timeout and parser-success checks reach Accept. Check/Prepare
    /// deliberately have no capture and cannot mint application evidence.
    pub(super) fn accepted(
        &self,
        capture: Option<SelectorValidationCapture>,
        generation: u64,
    ) -> Option<Arc<SelectorValidationReceipt>> {
        let capture = capture?;
        (capture.generation == generation && Arc::ptr_eq(&capture.context, &self.context))
            .then(|| Arc::new(SelectorValidationReceipt(capture)))
    }

    fn matches(
        &self,
        receipt: &SelectorValidationReceipt,
        state: &BrowserApplication,
        inventory: &SelectorValidationInventory,
    ) -> bool {
        Arc::ptr_eq(&receipt.0.context, &self.context)
            && match &receipt.0.owner {
                SelectorValidationOwner::Active(owner) => {
                    state.configuration_request_owner_is_current(owner)
                }
                SelectorValidationOwner::Profile(owner) => {
                    state.profile_open_validation_is_current(owner)
                }
            }
            && receipt.0.inventory == *inventory
    }

    pub(super) fn matching(
        &self,
        state: &BrowserApplication,
        layers: &ConfigurationLayers,
    ) -> Option<Arc<SelectorValidationReceipt>> {
        let inventory = selector_validation_inventory(layers);
        self.active
            .iter()
            .chain(&self.desired)
            .find(|receipt| self.matches(receipt, state, &inventory))
            .cloned()
    }

    /// Invoke only after canonical publication succeeds. Runtime updates may
    /// advance desired CSS while active consumers retain older reload sources.
    pub(super) fn published(
        &mut self,
        state: &BrowserApplication,
        receipt: Option<Arc<SelectorValidationReceipt>>,
    ) {
        let sources = state.configuration_policy_sources();
        let active = selector_validation_inventory(sources.reload_layers());
        let desired = selector_validation_inventory(sources.desired_layers());
        let find = |inventory: &SelectorValidationInventory| {
            receipt
                .iter()
                .chain(&self.active)
                .chain(&self.desired)
                .find(|receipt| self.matches(receipt, state, inventory))
                .cloned()
        };
        let next_active = find(&active);
        let next_desired = find(&desired);
        self.active = next_active;
        self.desired = next_desired;
    }

    pub(super) fn proves(
        &self,
        receipt: Option<&Arc<SelectorValidationReceipt>>,
        state: &BrowserApplication,
        layers: &ConfigurationLayers,
        selectors: &[String],
    ) -> bool {
        !selectors.is_empty()
            && receipt.is_some_and(|receipt| {
                self.matches(receipt, state, &selector_validation_inventory(layers))
                    && selectors.iter().all(|selector| {
                        receipt
                            .0
                            .inventory
                            .items
                            .iter()
                            .any(|item| item.selector == *selector)
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::{
        Config, ContextsConfig, HintSelectorFamily, HintSelectorMode, HintSelectorPolicy,
        RuntimeOverrides,
    };

    fn profile_candidate(
        state: &mut BrowserApplication,
        layers: ConfigurationLayers,
    ) -> ProfileOpenCandidate {
        state
            .request_profile_open_for_validation(ferric_browser_application::ProfileOpenRequest {
                name: "receipt-profile".into(),
                label: "Receipt profile".into(),
                privacy: ferric_browser_core::PrivacyKind::Private,
                storage: ferric_browser_application::ProfileStorage::Transient,
                base_configuration: layers.base,
                base_authored: layers.base_authored,
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: RuntimeOverrides::default(),
                config_path: None,
                script_catalog_root: None,
                contexts: ContextsConfig::default(),
                context_configuration_error: None,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(result) = state.poll_profile_candidate() {
                return result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    fn commit_profile(state: &mut BrowserApplication, candidate: ProfileOpenCandidate) {
        state.admit_profile_candidate(candidate).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(result) = state.poll_profile_open() {
                result.unwrap();
                return;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn profile_receipt_joins_only_its_exact_installed_owner() {
        let (mut state, _, _) = BrowserApplication::bootstrap(
            Config::default(),
            ferric_browser_core::PrivacyKind::Private,
            "receipt-profile",
        )
        .unwrap();
        let layers = layers("a[href]");
        let candidate = profile_candidate(&mut state, layers.clone());
        let mut receipts = SelectorValidationReceipts::default();
        let items = selector_validation_items(&layers);
        let foreign = BrowserApplication::new(Config::default());
        assert!(
            receipts
                .capture_profile(&foreign, &candidate, 9, &items)
                .is_none()
        );
        assert!(receipts.capture(&state, &layers, 9, &items).is_none());
        let receipt = receipts
            .accepted(receipts.capture_profile(&state, &candidate, 9, &items), 9)
            .unwrap();
        assert!(!receipts.proves(Some(&receipt), &state, &layers, &["a[href]".into()]));
        receipts.published(&state, Some(receipt.clone()));
        assert!(receipts.matching(&state, &layers).is_none());
        commit_profile(&mut state, candidate);
        receipts.published(&state, Some(receipt));
        assert!(receipts.matching(&state, &layers).is_some());
        state.close_profile();
        assert!(receipts.matching(&state, &layers).is_none());
    }

    #[test]
    fn equal_profile_replacement_cannot_rebind_old_candidate_css_evidence() {
        let (mut state, _, _) = BrowserApplication::bootstrap(
            Config::default(),
            ferric_browser_core::PrivacyKind::Private,
            "receipt-profile",
        )
        .unwrap();
        let layers = layers("a[href]");
        let candidate = profile_candidate(&mut state, layers.clone());
        let mut receipts = SelectorValidationReceipts::default();
        let items = selector_validation_items(&layers);
        let obsolete_receipt = receipts
            .accepted(receipts.capture_profile(&state, &candidate, 9, &items), 9)
            .unwrap();
        let candidate = state
            .prepare_profile_candidate_temporary(candidate, RuntimeOverrides::default())
            .unwrap();
        assert_eq!(candidate.layers(), &layers);
        commit_profile(&mut state, candidate);
        receipts.published(&state, Some(obsolete_receipt));
        assert!(receipts.matching(&state, &layers).is_none());
    }

    fn layers(selector: &str) -> ConfigurationLayers {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.base.hints.selectors.insert(
            HintSelectorFamily::Links,
            HintSelectorPolicy {
                mode: HintSelectorMode::Replace,
                selectors: vec![selector.into()],
            },
        );
        layers
    }

    fn accept(
        receipts: &SelectorValidationReceipts,
        state: &BrowserApplication,
        layers: &ConfigurationLayers,
    ) -> Arc<SelectorValidationReceipt> {
        receipts
            .accepted(
                receipts.capture(state, layers, 9, &selector_validation_items(layers)),
                9,
            )
            .unwrap()
    }

    #[test]
    fn only_exact_nonempty_requests_in_the_same_parser_context_can_mint() {
        let state = BrowserApplication::new(Config::default());
        let receipts = SelectorValidationReceipts::default();
        let layers = layers("a[href]");
        assert!(receipts.capture(&state, &layers, 9, &[]).is_none());
        assert!(
            receipts
                .capture(
                    &state,
                    &layers,
                    9,
                    &selector_validation_items(&self::layers("button"))
                )
                .is_none()
        );
        assert!(receipts.accepted(None, 9).is_none());
        let capture = receipts.capture(&state, &layers, 9, &selector_validation_items(&layers));
        assert!(receipts.accepted(capture, 10).is_none());
        let capture = receipts.capture(&state, &layers, 9, &selector_validation_items(&layers));
        assert!(
            SelectorValidationReceipts::default()
                .accepted(capture, 9)
                .is_none()
        );
    }

    #[test]
    fn accepting_a_candidate_does_not_install_or_leak_it_to_active() {
        let mut state = BrowserApplication::new(Config::default());
        let layers = layers("a[href]");
        let mut receipts = SelectorValidationReceipts::default();
        let receipt = accept(&receipts, &state, &layers);
        assert!(receipts.matching(&state, &layers).is_none());
        assert!(receipts.proves(Some(&receipt), &state, &layers, &["a[href]".into()]));
        assert!(!receipts.proves(
            Some(&receipt),
            &state,
            &layers,
            &["a[href]".into(), "button".into()]
        ));
        assert!(!receipts.proves(Some(&receipt), &state, &layers, &[]));
        // A receipt for uninstalled layers cannot be installed through some
        // unrelated successful publication (or a validation-only check).
        receipts.published(&state, Some(receipt.clone()));
        assert!(receipts.matching(&state, &layers).is_none());
        state.activate_configuration(layers.clone()).unwrap();
        receipts.published(&state, Some(receipt));
        assert!(receipts.matching(&state, &layers).is_some());
    }

    #[test]
    fn repeated_runtime_edits_preserve_old_reload_proof_independently() {
        let mut state = BrowserApplication::new(Config::default());
        let old = layers("a[href]");
        state.activate_configuration(old.clone()).unwrap();
        let mut receipts = SelectorValidationReceipts::default();
        let receipt = accept(&receipts, &state, &old);
        receipts.published(&state, Some(receipt));
        let desired = layers("button");
        let receipt = accept(&receipts, &state, &desired);
        state.update_configuration(desired.clone()).unwrap();
        receipts.published(&state, Some(receipt));
        assert_eq!(state.configuration_policy_sources().reload_layers(), &old);
        assert!(receipts.matching(&state, &old).is_some());
        assert!(receipts.matching(&state, &desired).is_some());
        // A failed/unaccepted third proposal cannot displace either proof.
        let failed = layers("input");
        drop(receipts.capture(&state, &failed, 10, &selector_validation_items(&failed)));
        assert!(receipts.matching(&state, &failed).is_none());
        let mut unrelated = desired.clone();
        unrelated
            .runtime
            .set_literal("content.images", "false")
            .unwrap();
        state.update_configuration(unrelated.clone()).unwrap();
        receipts.published(&state, None);
        assert!(receipts.matching(&state, &old).is_some());
        assert!(receipts.matching(&state, &unrelated).is_some());
        state.activate_configuration(unrelated.clone()).unwrap();
        receipts.published(&state, None);
        assert!(receipts.matching(&state, &unrelated).is_some());
        assert!(receipts.matching(&state, &old).is_none());
    }

    #[test]
    fn same_literals_in_another_layer_or_owner_do_not_inherit_evidence() {
        let mut state = BrowserApplication::new(Config::default());
        let layers = layers("a[href]");
        state.activate_configuration(layers.clone()).unwrap();
        let mut receipts = SelectorValidationReceipts::default();
        let receipt = accept(&receipts, &state, &layers);
        receipts.published(&state, Some(receipt.clone()));
        let mut moved = ConfigurationLayers::new(Config::default());
        moved
            .runtime
            .set_literal(
                "hints.selectors.links",
                "{mode='replace',selectors=['a[href]']}",
            )
            .unwrap();
        assert!(receipts.matching(&state, &moved).is_none());
        let mut foreign = BrowserApplication::new(Config::default());
        foreign.activate_configuration(layers.clone()).unwrap();
        assert_eq!(
            foreign.configuration_revision(),
            state.configuration_revision()
        );
        assert!(!receipts.proves(Some(&receipt), &foreign, &layers, &["a[href]".into()]));
        receipts.published(&foreign, Some(receipt));
        assert!(receipts.matching(&foreign, &layers).is_none());
    }

    #[test]
    fn duplicate_occurrences_and_authored_paths_are_part_of_the_inventory() {
        let state = BrowserApplication::new(Config::default());
        let layers = layers("a[href]");
        let receipts = SelectorValidationReceipts::default();
        let receipt = accept(&receipts, &state, &layers);
        let mut duplicate = layers.clone();
        duplicate
            .temporary
            .set_literal(
                "hints.selectors.links",
                "{mode='replace',selectors=['a[href]']}",
            )
            .unwrap();
        assert_eq!(
            selector_validation_items(&duplicate),
            selector_validation_items(&layers)
        );
        assert!(!receipts.proves(Some(&receipt), &state, &duplicate, &["a[href]".into()]));
        let dir =
            std::env::temp_dir().join(format!("ferric-css-inventory-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw =
            "schema_version=3\n[hints.selectors.links]\nmode='replace'\nselectors=['a[href]']\n";
        for name in ["first.toml", "second.toml"] {
            std::fs::write(dir.join(name), raw).unwrap();
        }
        let load = |name| {
            let loaded = ferric_browser_config::load(dir.join(name)).unwrap();
            let mut layers = ConfigurationLayers::new(loaded.config);
            layers.base_authored = loaded.authored;
            layers
        };
        let first = load("first.toml");
        let second = load("second.toml");
        assert_eq!(
            selector_validation_items(&first),
            selector_validation_items(&second)
        );
        let receipt = accept(&receipts, &state, &first);
        assert!(!receipts.proves(Some(&receipt), &state, &second, &["a[href]".into()]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
