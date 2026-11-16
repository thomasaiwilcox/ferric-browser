use ferric_browser_config::{
    Config, ConfigError, ConfigurationApplyBoundary, ConfigurationSources, PendingSettingChange,
    RuntimeOverrides, apply_config_changes, pending_config_changes,
    resolve_runtime_override_layers,
};
use std::sync::Arc;

mod discovery_compilation;
pub use discovery_compilation::{
    EffectiveHintProxyTargets, HintProxyCompositionTrace, HintProxyContribution,
    InspectedHintProxyTargets, InspectedHintSelectors,
};
mod inspection;
mod interaction_edits;
mod saved_runtime_edits;
pub use saved_runtime_edits::SavedRuntimeEditCapture;
mod interaction_validation;
pub use interaction_edits::{
    InteractionEditCapture, InteractionRuleChange, InteractionRuleEdit, InteractionRuleSnapshot,
    PreparedInteractionEdits,
};
pub(crate) use interaction_validation::{
    CapturedInteractionContexts, InteractionValidationProfile,
};
mod policy_compilation;
pub use inspection::{
    ConfigurationInspectionCandidate, ConfigurationInspectionCandidateStage,
    ConfigurationInspectionInputs, InspectedInteractionContribution, InspectedInteractionValue,
    InspectedSettingContribution, InspectedSettingValue, InspectionBoundary, InspectionOrigin,
    InspectionProfile, InspectionValidationStage, InteractionInspection,
    InteractionInspectionExplanation, InteractionInspectionRequest, InteractionInspectionValues,
    SettingActivationBoundary, SettingExplanation, SettingInspection, SettingInspectionPending,
    SettingInspectionRequest, SettingInspectionValidation,
};
pub use policy_compilation::InspectedInteractionPolicy;

/// All precedence-bearing configuration inputs owned by the application.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationLayers {
    pub base: Config,
    /// Raw include inputs travel with the effective base in every prepared
    /// update. They are used for validation/provenance, never rule matching.
    pub base_authored: std::sync::Arc<ConfigurationSources>,
    pub profile: RuntimeOverrides,
    pub runtime: RuntimeOverrides,
    pub command_line: RuntimeOverrides,
    pub temporary: RuntimeOverrides,
}

impl ConfigurationLayers {
    #[must_use]
    pub fn new(base: Config) -> Self {
        Self {
            base,
            base_authored: std::sync::Arc::default(),
            profile: RuntimeOverrides::default(),
            runtime: RuntimeOverrides::default(),
            command_line: RuntimeOverrides::default(),
            temporary: RuntimeOverrides::default(),
        }
    }

    /// Resolves all layers using the configuration crate's canonical
    /// precedence rules.
    ///
    /// # Errors
    ///
    /// Returns a schema or validation error when a layer is invalid.
    pub fn resolve(&self) -> Result<Config, ConfigError> {
        resolve_runtime_override_layers(
            &self.base,
            &self.profile,
            &self.runtime,
            &self.command_line,
            &self.temporary,
        )
    }
}

/// Immutable configuration projection for presentation adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationSnapshot {
    pub effective: Config,
    pub revision: u64,
    pub pending_changes: Vec<PendingSettingChange>,
}

/// Immutable source inputs captured at the existing configuration boundaries.
/// Policy compilers take live label/proxy globals from desired inputs and
/// reload-scoped rules/selectors from reload inputs. Both retain their own
/// authored catalogue. Startup inputs retain origins of values that a reload
/// cannot activate. None of these whole graphs is an active configuration:
/// startup/restart values still require the separately supplied effective view.
/// Revisions are local to the owning application/profile configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationPolicySources {
    desired: Arc<ConfigurationLayers>,
    reload: Arc<ConfigurationLayers>,
    startup: Arc<ConfigurationLayers>,
    revision: u64,
    reload_revision: u64,
    startup_revision: u64,
}

impl ConfigurationPolicySources {
    /// Mandatory catalogue/static checks, without installing an effective
    /// consumer or asserting contextual/engine/live validation success.
    pub(crate) fn validate_raw_sources(&self) -> Result<(), ConfigError> {
        let boundaries = [
            (&self.desired, self.revision),
            (&self.reload, self.reload_revision),
            (&self.startup, self.startup_revision),
        ];
        for (index, (layers, revision)) in boundaries.iter().enumerate() {
            if boundaries[..index]
                .iter()
                .any(|(earlier, _)| Arc::ptr_eq(earlier, layers))
            {
                continue;
            }
            layers.base_authored.validate_base_settings(&layers.base)?;
            policy_compilation::checked(layers, *revision)?;
        }
        Ok(())
    }

    /// Prospective receipt-free shared Runtime substitution for another owner.
    /// Other source and activation boundaries remain that owner's own inputs.
    pub(crate) fn with_shared_runtime(
        &self,
        runtime: &RuntimeOverrides,
    ) -> Result<Self, ConfigError> {
        let mut desired = self.desired.as_ref().clone();
        desired.runtime = runtime.clone();
        desired.resolve()?;
        let mut candidate = self.clone();
        candidate.desired = Arc::new(desired);
        candidate.validate_raw_sources()?;
        Ok(candidate)
    }

    /// Projects a prospective configuration reload from these exact desired
    /// layers while retaining the startup boundary. This pure capture grants
    /// no validation receipt, activation or publication permission.
    #[must_use]
    pub fn prospective_reload(&self) -> Self {
        Self {
            desired: Arc::clone(&self.desired),
            reload: Arc::clone(&self.desired),
            startup: Arc::clone(&self.startup),
            revision: self.revision,
            reload_revision: self.revision,
            startup_revision: self.startup_revision,
        }
    }

    #[must_use]
    pub fn desired_layers(&self) -> &ConfigurationLayers {
        &self.desired
    }

    #[must_use]
    pub fn reload_layers(&self) -> &ConfigurationLayers {
        &self.reload
    }

    #[must_use]
    pub fn startup_layers(&self) -> &ConfigurationLayers {
        &self.startup
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn reload_revision(&self) -> u64 {
        self.reload_revision
    }

    #[must_use]
    pub const fn startup_revision(&self) -> u64 {
        self.startup_revision
    }
}

/// A validated update that has not changed the active application state.
#[derive(Debug)]
pub struct PreparedConfigurationUpdate {
    pub(crate) owner: ConfigurationOwner,
    pub(crate) expected_revision: u64,
    pub(crate) profile_revision: u64,
    pub(crate) state: ConfigurationState,
    pub(crate) preparation_identity: Arc<()>,
    pub(crate) admission: ConfigurationAdmission,
}

/// Admission history of this exact immutable preparation, never a validation
/// receipt. Receiving committed shared data performs no additional disk write.
#[derive(Debug)]
pub(crate) enum ConfigurationAdmission {
    Fresh,
    RuntimeWrite,
    ContextReload,
    CommittedPeer(std::sync::Weak<crate::profile_configuration::SharedConfiguration>),
}

impl ConfigurationAdmission {
    pub(crate) fn requires_context_preflight(&self) -> bool {
        matches!(self, Self::Fresh)
    }

    pub(crate) fn ensure_fresh(&self) -> Result<(), ConfigError> {
        if self.requires_context_preflight() {
            Ok(())
        } else {
            Err(ConfigError::validation(format_args!(
                "This preparation already belongs to admitted configuration work"
            )))
        }
    }
}

/// Local identity captured before deferring configuration preparation. This
/// performs no source composition, shared-profile locking or I/O and grants
/// neither persistence nor publication authority.
#[derive(Clone, Debug)]
pub struct ConfigurationRequestCapture {
    owner: ConfigurationOwner,
    revision: u64,
}

impl crate::BrowserApplication {
    #[must_use]
    pub fn capture_configuration_request(&self) -> Option<ConfigurationRequestCapture> {
        let owner = ConfigurationOwner::capture(self);
        owner.matches(self).then(|| ConfigurationRequestCapture {
            owner,
            revision: self.configuration_revision(),
        })
    }

    /// Checks the original local owner before preparation or callback effects.
    /// Ordinary prepared-update checks still arbitrate shared-profile changes
    /// and write admission later, outside the input path.
    #[must_use]
    pub fn configuration_request_is_current(&self, capture: &ConfigurationRequestCapture) -> bool {
        capture.owner.matches(self) && capture.revision == self.configuration_revision()
    }

    /// Checks lifetime for the remaining steps of an already admitted local
    /// command chain. Earlier steps may legitimately change its local revision;
    /// initial admission and prepared publication still check exact revisions.
    #[must_use]
    pub fn configuration_request_owner_is_current(
        &self,
        capture: &ConfigurationRequestCapture,
    ) -> bool {
        capture.owner.matches(self)
    }
}

/// A live application identity plus its captured profile lifetime. The marker
/// is compared by allocation identity, never by its unit pointee's equality.
#[derive(Clone, Debug)]
pub(crate) struct ConfigurationOwner {
    marker: Arc<()>,
    profile: Option<crate::ProfileSnapshot>,
    instance: Option<crate::ProfileInstanceId>,
}

impl ConfigurationOwner {
    pub(crate) fn same_capture(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.marker, &other.marker)
            && self.profile == other.profile
            && self.instance == other.instance
    }

    pub(crate) fn capture(app: &crate::BrowserApplication) -> Self {
        Self {
            marker: Arc::clone(&app.configuration_owner),
            profile: app.profile_snapshot(),
            instance: app.profile_instance_id(),
        }
    }

    pub(crate) fn matches(&self, app: &crate::BrowserApplication) -> bool {
        self.belongs_to(app)
            && app.profile_open.is_none()
            && app.runtime.view().shutdown() == ferric_browser_core::ShutdownState::Running
    }

    /// Lifetime identity, independent of temporary readiness (for example an
    /// admitted profile-open attempt which may fail and leave this owner live).
    pub(crate) fn belongs_to(&self, app: &crate::BrowserApplication) -> bool {
        Arc::ptr_eq(&self.marker, &app.configuration_owner)
            && self.profile == app.profile_snapshot()
            && self.instance == app.profile_instance_id()
    }
}

impl PreparedConfigurationUpdate {
    /// This exact handle receives an already committed shared runtime. It
    /// retains its own owner's other layers and may expose closed contextual
    /// conflicts, but grants no engine proof or permission to write the file.
    #[must_use]
    pub fn is_committed_peer_update(&self) -> bool {
        matches!(self.admission, ConfigurationAdmission::CommittedPeer(_))
    }

    #[must_use]
    pub fn snapshot(&self) -> ConfigurationSnapshot {
        self.state.snapshot()
    }

    /// Immutable inputs for validation and preview of this exact candidate.
    /// Reading them grants no persistence or publication authority.
    #[must_use]
    pub fn policy_sources(&self) -> ConfigurationPolicySources {
        self.state.policy_sources()
    }

    /// Compiles an owned typed consumer snapshot from this exact prepared
    /// candidate, before persistence. The compiler sees both the active-after-
    /// boundary projection and the retained source layers; it must respect
    /// pending settings when choosing inputs. It must not publish anything.
    ///
    /// # Errors
    /// Returns compilation/validation errors without publishing configuration
    /// or consumer state. No consumer output is exposed by the prepared handle.
    pub fn prepare_consumer<T>(
        self,
        compile: impl FnOnce(&Config, &ConfigurationLayers, u64) -> Result<T, ConfigError>,
    ) -> Result<PreparedConfigurationConsumer<T>, ConfigError> {
        let consumer = compile(
            self.state.effective(),
            self.state.layers(),
            self.state.revision(),
        )?;
        Ok(PreparedConfigurationConsumer {
            configuration: self,
            consumer,
        })
    }

    /// Compiles a typed policy consumer from this exact prepared projection
    /// and its retained source boundaries, before persistence/publication.
    /// Source inputs expose provenance; they grant no runtime action authority.
    /// The compiler must validate desired raw inputs and choose each active
    /// policy field from its declared boundary.
    ///
    /// # Errors
    /// Returns compilation errors without publishing configuration or output.
    pub fn prepare_policy_consumer<T>(
        self,
        compile: impl FnOnce(&Config, &ConfigurationPolicySources) -> Result<T, ConfigError>,
    ) -> Result<PreparedConfigurationConsumer<T>, ConfigError> {
        let sources = self.state.policy_sources();
        let consumer = compile(self.state.effective(), &sources)?;
        Ok(PreparedConfigurationConsumer {
            configuration: self,
            consumer,
        })
    }
}

/// A typed compilation attached to the existing configuration transaction.
/// Fields stay private; only successful application publication releases the
/// owned consumer snapshot. This is not a second writer or publication queue.
pub struct PreparedConfigurationConsumer<T> {
    pub(crate) configuration: PreparedConfigurationUpdate,
    pub(crate) consumer: T,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConfigurationState {
    layers: Arc<ConfigurationLayers>,
    reload_layers: Arc<ConfigurationLayers>,
    reload_revision: u64,
    startup_layers: Arc<ConfigurationLayers>,
    startup_revision: u64,
    effective: Config,
    revision: u64,
    pending_changes: Vec<PendingSettingChange>,
}

impl ConfigurationState {
    pub(crate) fn new(effective: Config) -> Self {
        let layers = Arc::new(ConfigurationLayers::new(effective.clone()));
        Self {
            reload_layers: Arc::clone(&layers),
            startup_layers: Arc::clone(&layers),
            layers,
            reload_revision: 0,
            startup_revision: 0,
            effective,
            revision: 0,
            pending_changes: Vec::new(),
        }
    }

    pub(crate) fn layers(&self) -> &ConfigurationLayers {
        &self.layers
    }

    pub(crate) fn policy_sources(&self) -> ConfigurationPolicySources {
        ConfigurationPolicySources {
            desired: Arc::clone(&self.layers),
            reload: Arc::clone(&self.reload_layers),
            startup: Arc::clone(&self.startup_layers),
            revision: self.revision,
            reload_revision: self.reload_revision,
            startup_revision: self.startup_revision,
        }
    }

    pub(crate) const fn effective(&self) -> &Config {
        &self.effective
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn snapshot(&self) -> ConfigurationSnapshot {
        ConfigurationSnapshot {
            effective: self.effective.clone(),
            revision: self.revision,
            pending_changes: self.pending_changes.clone(),
        }
    }

    pub(crate) fn replace_effective(
        &mut self,
        effective: Config,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        ferric_browser_config::validate(&effective)?;
        let snapshot = self.commit(
            ConfigurationLayers::new(effective.clone()),
            effective,
            Vec::new(),
            ConfigurationApplyBoundary::Reload,
        )?;
        self.capture_startup_sources();
        Ok(snapshot)
    }

    pub(crate) fn activate_layers(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let effective = layers.resolve()?;
        let snapshot = self.commit(
            layers,
            effective,
            Vec::new(),
            ConfigurationApplyBoundary::Reload,
        )?;
        self.capture_startup_sources();
        Ok(snapshot)
    }

    fn capture_startup_sources(&mut self) {
        self.startup_layers = Arc::clone(&self.layers);
        self.startup_revision = self.revision;
    }

    pub(crate) fn update_layers_at(
        &mut self,
        layers: ConfigurationLayers,
        boundary: ConfigurationApplyBoundary,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let candidate = layers.resolve()?;
        let current = toml::Value::try_from(&self.effective).map_err(|error| {
            ConfigError::Validation(format!("could not encode active configuration: {error}"))
        })?;
        let candidate_value = toml::Value::try_from(&candidate).map_err(|error| {
            ConfigError::Validation(format!("could not encode candidate configuration: {error}"))
        })?;
        let effective_value = apply_config_changes(&current, &candidate_value, boundary);
        let pending = pending_config_changes(&effective_value, &candidate_value);
        let effective = effective_value.try_into::<Config>().map_err(|error| {
            ConfigError::Validation(format!(
                "could not decode active configuration after applying live changes: {error}"
            ))
        })?;
        ferric_browser_config::validate(&effective)?;
        self.commit(layers, effective, pending, boundary)
    }

    fn commit(
        &mut self,
        layers: ConfigurationLayers,
        effective: Config,
        pending_changes: Vec<PendingSettingChange>,
        boundary: ConfigurationApplyBoundary,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| ConfigError::Validation("configuration revision is exhausted".into()))?;
        let layers = Arc::new(layers);
        if boundary == ConfigurationApplyBoundary::Reload {
            self.reload_layers = Arc::clone(&layers);
            self.reload_revision = revision;
        }
        self.layers = layers;
        self.effective = effective;
        self.pending_changes = pending_changes;
        self.revision = revision;
        Ok(self.snapshot())
    }
}

#[cfg(test)]
mod consumer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BrowserApplication;
    use ferric_browser_config::{
        PageGroupingPolicy, PhonePasskeyPolicy, TabsPresentation, setting_metadata,
    };

    #[test]
    fn request_capture_rejects_foreign_and_changed_local_configuration() {
        let mut owner = BrowserApplication::new(Config::default());
        let foreign = BrowserApplication::new(Config::default());
        let capture = owner.capture_configuration_request().unwrap();
        assert!(owner.configuration_request_is_current(&capture));
        assert!(!foreign.configuration_request_is_current(&capture));
        let mut layers = owner.configuration_layers().clone();
        layers
            .temporary
            .set_literal("content.zoom", "1.25")
            .unwrap();
        owner.update_configuration(layers).unwrap();
        assert!(!owner.configuration_request_is_current(&capture));
        assert!(owner.configuration_request_owner_is_current(&capture));
        assert!(
            owner.configuration_request_is_current(&owner.capture_configuration_request().unwrap())
        );
    }

    #[test]
    fn request_capture_rejects_profile_activation_even_with_unchanged_revision() {
        let mut owner = BrowserApplication::new(Config::default());
        let capture = owner.capture_configuration_request().unwrap();
        let revision = owner.configuration_revision();
        owner
            .activate_profile(crate::profile::ProfileActivation {
                name: "replacement".into(),
                id: None,
                privacy: ferric_browser_core::PrivacyKind::Normal,
                roots: None,
                lock: None,
                contexts: None,
                storage_path: None,
            })
            .unwrap();
        assert_eq!(owner.configuration_revision(), revision);
        assert!(!owner.configuration_request_is_current(&capture));
        assert!(!owner.configuration_request_owner_is_current(&capture));
    }

    #[test]
    fn authored_sources_publish_and_reject_with_their_effective_base() {
        let loaded = ferric_browser_config::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
        ))
        .unwrap();
        let authored = loaded.authored;
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = authored.clone();
        let mut state = ConfigurationState::new(Config::default());
        state.activate_layers(layers.clone()).unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &state.layers().base_authored,
            &authored
        ));
        assert!(state.effective().site_rules.is_empty());
        assert_eq!(
            state.layers().base_authored.documents()[0]
                .site_rules()
                .unwrap()
                .len(),
            1
        );

        layers.runtime.set_literal("content.zoom", "1.25").unwrap();
        state
            .update_layers_at(layers.clone(), ConfigurationApplyBoundary::RuntimeUpdate)
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &state.layers().base_authored,
            &authored
        ));
        let before = state.clone();
        layers.base.ui.font_size_pt = 5.0;
        layers.base_authored = std::sync::Arc::default();
        assert!(
            state
                .update_layers_at(layers, ConfigurationApplyBoundary::Reload)
                .is_err()
        );
        assert_eq!(state, before);
        assert!(std::sync::Arc::ptr_eq(
            &state.layers().base_authored,
            &authored
        ));
        state.replace_effective(Config::default()).unwrap();
        assert!(state.layers().base_authored.documents().is_empty());
        assert_eq!(state.effective(), &Config::default());
    }

    #[test]
    fn feature_settings_declare_their_scope_and_apply_boundary() {
        let presentation = setting_metadata("tabs.presentation").expect("presentation metadata");
        assert_eq!(presentation.supported_scopes, &["global"]);
        assert_eq!(presentation.apply_time, "restart");

        let phone = setting_metadata("passkeys.phone").expect("phone metadata");
        assert_eq!(phone.supported_scopes, &["global", "profile"]);
        assert_eq!(phone.apply_time, "navigation");

        let grouping = setting_metadata("hyprland.page_grouping").expect("page-grouping metadata");
        assert_eq!(grouping.supported_scopes, &["global"]);
        assert_eq!(grouping.apply_time, "live");
    }

    #[test]
    fn invalid_combined_feature_update_leaves_active_configuration_unchanged() {
        let mut app = BrowserApplication::new(Config::default());
        let before = app.configuration_snapshot();
        let mut candidate = Config::default();
        candidate.tabs.presentation = TabsPresentation::Windows;
        candidate.passkeys.phone = PhonePasskeyPolicy::On;
        candidate.hyprland.page_grouping = PageGroupingPolicy::Auto;
        candidate.ui.font_size_pt = 5.0;

        assert!(
            app.update_configuration(ConfigurationLayers::new(candidate))
                .is_err(),
            "an invalid combined configuration update must be rejected"
        );

        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_revision(), before.revision);
        assert_eq!(
            app.configuration().tabs.presentation,
            TabsPresentation::Internal
        );
        assert_eq!(app.configuration().passkeys.phone, PhonePasskeyPolicy::Off);
        assert_eq!(
            app.configuration().hyprland.page_grouping,
            PageGroupingPolicy::Off
        );
    }
}
