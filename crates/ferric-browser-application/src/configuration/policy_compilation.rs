//! Source-boundary orchestration over the configuration crate's one resolver.
//! These immutable values still require codebook, engine and live qualification.
use super::{ConfigurationLayers, ConfigurationPolicySources};
use ferric_browser_config::{
    AuthoredHints, ConfigError, EffectiveInteractionPolicy, InteractionCompositionTrace,
    InteractionPolicyBuilder, InteractionPolicyRevision, PolicySourceIdentity,
    ResolvedSitePolicyRules, SelectedSitePolicyRules, SitePolicySelectionTrace,
    SitePolicySourceRevisions, resolve_site_policy_rules,
};

/// Effective Interaction values and opt-in observations of their canonical
/// composition. Static/effective validation grants no engine or live proof.
#[derive(Clone, Debug)]
pub struct InspectedInteractionPolicy {
    pub policy: EffectiveInteractionPolicy,
    pub trace: InteractionCompositionTrace,
    pub selection: SitePolicySelectionTrace,
}

pub(super) struct CheckedSources<'a> {
    pub(super) rules: ResolvedSitePolicyRules<'a, 'a>,
    shared: Vec<(PolicySourceIdentity, AuthoredHints)>,
}

/// One checked source graph shared by a batch of open contexts. This remains
/// borrowed, uninstalled compilation state; it cannot authorize publication.
pub(super) struct InteractionSourceCompiler<'a> {
    sources: &'a ConfigurationPolicySources,
    desired: CheckedSources<'a>,
    reload: Option<CheckedSources<'a>>,
    overlays: [AuthoredHints; 4],
}

impl ConfigurationPolicySources {
    /// Compiles a pure effective Interaction value for a browser-owned match
    /// context. The caller supplies its captured profile identity/revisions;
    /// source revisions come from this immutable configuration capture.
    /// Matching uses only reload-scoped rules, while explicit live shared
    /// label leaves interleave at their existing source boundaries.
    ///
    /// # Errors
    /// Rejects stale revisions, inconsistent catalogues, unsupported sources,
    /// invalid raw inputs or final composed conflicts. No partial value escapes.
    pub fn compile_interaction(
        &self,
        profile_name: &str,
        revision: InteractionPolicyRevision,
        url: &str,
    ) -> Result<EffectiveInteractionPolicy, ConfigError> {
        self.check_interaction_revision(revision)?;
        InteractionSourceCompiler::new(self, false)?.compile(profile_name, revision, url)
    }

    /// Observes the same source composition as `compile_interaction`. Profile
    /// identity/revisions must come from the same captured application owner.
    ///
    /// # Errors
    /// Rejects invalid sources, mismatched catalogues and effective conflicts.
    /// A rejected combination supplies no partial effective policy or trace.
    pub fn inspect_interaction(
        &self,
        profile_name: &str,
        revision: InteractionPolicyRevision,
        url: &str,
    ) -> Result<InspectedInteractionPolicy, ConfigError> {
        self.check_interaction_revision(revision)?;
        let compiler = InteractionSourceCompiler::new(self, true)?;
        let (builder, selection) = compiler.builder(profile_name, url, true)?;
        let (policy, trace) = builder
            .finish_with_trace(revision)
            .map_err(|message| invalid(&message))?;
        Ok(InspectedInteractionPolicy {
            policy,
            trace,
            selection: selection.ok_or_else(|| invalid("Interaction selection is unavailable"))?,
        })
    }

    fn check_interaction_revision(
        &self,
        revision: InteractionPolicyRevision,
    ) -> Result<(), ConfigError> {
        if revision.configuration_revision != self.revision() {
            return Err(ConfigError::validation(format_args!(
                "Interaction compilation configuration revision does not match its source capture"
            )));
        }
        Ok(())
    }
}

impl<'a> InteractionSourceCompiler<'a> {
    /// Equality of exact canonical composition inputs, excluding revision
    /// stamps. Used only to retain an already closed conflict during unrelated
    /// edits or a pending repair; never to grant an effective policy or proof.
    pub(super) fn same_inputs_for(&self, other: &Self, url: &str) -> bool {
        if self.desired.shared != other.desired.shared || self.overlays != other.overlays {
            return false;
        }
        let selected = self
            .reload
            .as_ref()
            .unwrap_or(&self.desired)
            .rules
            .select(url);
        let previous = other
            .reload
            .as_ref()
            .unwrap_or(&other.desired)
            .rules
            .select(url);
        let inputs = |rule: &ferric_browser_config::SelectedSitePolicyRule<'_>| {
            // Compare below without cloning raw policies or retaining failed
            // partial values. Ordinary set is conservatively included too.
            rule.rule().interaction.is_some()
        };
        let mut selected = selected.selected().iter().filter(|rule| inputs(rule));
        let mut previous = previous.selected().iter().filter(|rule| inputs(rule));
        loop {
            match (selected.next(), previous.next()) {
                (None, None) => return true,
                (Some(left), Some(right))
                    if left.rule() == right.rule()
                        && left.origin().source() == right.origin().source()
                        && left.origin().rule() == right.origin().rule() => {}
                _ => return false,
            }
        }
    }

    pub(super) fn has_applicable_interaction(&self, url: &str) -> bool {
        self.reload
            .as_ref()
            .unwrap_or(&self.desired)
            .rules
            .select(url)
            .selected()
            .iter()
            .any(|rule| rule.rule().interaction.is_some())
    }

    pub(super) fn new(
        sources: &'a ConfigurationPolicySources,
        validate_catalogue: bool,
    ) -> Result<Self, ConfigError> {
        let desired = checked(sources.desired_layers(), sources.revision())?;
        let reload = if std::sync::Arc::ptr_eq(&sources.desired, &sources.reload)
            && sources.revision == sources.reload_revision
        {
            None
        } else {
            Some(checked(sources.reload_layers(), sources.reload_revision())?)
        };
        if validate_catalogue {
            sources
                .desired_layers()
                .base_authored
                .validate_base_settings(&sources.desired_layers().base)?;
            if reload.is_some() {
                sources
                    .reload_layers()
                    .base_authored
                    .validate_base_settings(&sources.reload_layers().base)?;
            }
        }
        let layers = sources.desired_layers();
        let overlays = [
            layers.profile.shared_hint_labels()?,
            layers.runtime.shared_hint_labels()?,
            layers.command_line.shared_hint_labels()?,
            layers.temporary.shared_hint_labels()?,
        ];
        Ok(Self {
            sources,
            desired,
            reload,
            overlays,
        })
    }

    pub(super) fn compile(
        &self,
        profile_name: &str,
        revision: InteractionPolicyRevision,
        url: &str,
    ) -> Result<EffectiveInteractionPolicy, ConfigError> {
        self.sources.check_interaction_revision(revision)?;
        let (builder, _) = self.builder(profile_name, url, false)?;
        builder
            .finish(revision)
            .map_err(|message| invalid(&message))
    }

    fn builder(
        &self,
        profile_name: &str,
        url: &str,
        inspect: bool,
    ) -> Result<(InteractionPolicyBuilder, Option<SitePolicySelectionTrace>), ConfigError> {
        let sources = self.sources;
        let desired = &self.desired;
        let reload = self.reload.as_ref().unwrap_or(desired);
        let (selected, selection, mut builder) = if inspect {
            let (selected, selection) = reload.rules.select_with_trace(url);
            (
                selected,
                Some(selection),
                InteractionPolicyBuilder::traced(),
            )
        } else {
            (
                reload.rules.select(url),
                None,
                InteractionPolicyBuilder::default(),
            )
        };
        for (source, hints) in &desired.shared {
            builder
                .apply_shared_labels(hints, source.clone(), sources.revision())
                .map_err(|message| invalid(&message))?;
        }
        builder
            .apply_shared_labels(
                &self.overlays[0],
                PolicySourceIdentity::Profile {
                    name: profile_name.into(),
                },
                sources.revision(),
            )
            .map_err(|message| invalid(&message))?;
        apply_rules(&mut builder, &selected, |source| {
            matches!(
                source,
                PolicySourceIdentity::InMemory | PolicySourceIdentity::Authored { .. }
            )
        })?;
        apply_rules(&mut builder, &selected, |source| {
            matches!(source, PolicySourceIdentity::Generated)
        })?;
        builder
            .apply_shared_labels(
                &self.overlays[1],
                PolicySourceIdentity::Generated,
                sources.revision(),
            )
            .map_err(|message| invalid(&message))?;
        builder
            .apply_shared_labels(
                &self.overlays[2],
                PolicySourceIdentity::CommandLine,
                sources.revision(),
            )
            .map_err(|message| invalid(&message))?;
        apply_rules(&mut builder, &selected, |source| {
            matches!(source, PolicySourceIdentity::Temporary)
        })?;
        builder
            .apply_shared_labels(
                &self.overlays[3],
                PolicySourceIdentity::Temporary,
                sources.revision(),
            )
            .map_err(|message| invalid(&message))?;
        Ok((builder, selection))
    }
}

pub(super) fn checked(
    layers: &ConfigurationLayers,
    revision: u64,
) -> Result<CheckedSources<'_>, ConfigError> {
    if !layers.profile.site_rules.is_empty() || !layers.command_line.site_rules.is_empty() {
        return Err(ConfigError::validation(format_args!(
            "profile and command-line sources do not provide site rules"
        )));
    }
    let shared = layers.base_authored.shared_label_sources(&layers.base)?;
    // Validate typed globals through their existing composition path. Sparse
    // authored presence is read separately, never inferred from this result.
    layers.resolve()?;
    let rules = resolve_site_policy_rules(
        &layers.base,
        &layers.base_authored,
        &layers.runtime,
        &layers.temporary,
        SitePolicySourceRevisions {
            authored: revision,
            generated: revision,
            temporary: revision,
        },
    )?;
    Ok(CheckedSources { rules, shared })
}

fn apply_rules(
    builder: &mut InteractionPolicyBuilder,
    selected: &SelectedSitePolicyRules<'_>,
    accepts: impl Fn(&PolicySourceIdentity) -> bool,
) -> Result<(), ConfigError> {
    for rule in selected
        .selected()
        .iter()
        .filter(|rule| accepts(rule.origin().source()))
    {
        builder
            .apply_resolved_rule(rule)
            .map_err(|message| invalid(&message))?;
    }
    Ok(())
}

fn invalid(message: &str) -> ConfigError {
    ConfigError::validation(format_args!("{message}"))
}
