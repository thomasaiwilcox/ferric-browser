//! Exact Interaction edits over one immutable application/profile capture.
//! These are preparation APIs, not a second writer or an engine/live validation
//! receipt. The existing configuration coordinator owns every publication.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use super::{
    ConfigurationLayers, ConfigurationRequestCapture, PreparedConfigurationUpdate,
    SavedRuntimeEditCapture, policy_compilation::checked,
};
use crate::BrowserApplication;
use ferric_browser_config::{
    ConfigError, InteractionPolicyPatch, MAX_SITE_POLICY_IMPORT_OPERATIONS, PolicyFieldOrigin,
    PolicySourceIdentity, ResolvedSitePolicyRules, RuntimeOverrides, SitePolicySourceRevisions,
    SiteRule, SurvivingSitePolicyRule, resolve_site_policy_rules, validate_site_rule,
};

type RuleSnapshotPair = (Option<SiteRule>, Option<SiteRule>);

/// Captured outside physical input. Shared-profile revision is deliberately
/// separate from the cheap generic request capture: equal restored values do
/// not make an older reviewed edit current again.
#[derive(Clone)]
pub struct InteractionEditCapture {
    request: ConfigurationRequestCapture,
    saved: Option<SavedRuntimeEditCapture>,
    shared_profile_revision: u64,
    layers: Arc<ConfigurationLayers>,
}
impl fmt::Debug for InteractionEditCapture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionEditCapture")
            .field("configuration_revision", &self.configuration_revision())
            .field("shared_profile_revision", &self.shared_profile_revision)
            .finish_non_exhaustive()
    }
}
impl InteractionEditCapture {
    pub(crate) fn belongs_to(&self, app: &BrowserApplication) -> bool {
        self.request.owner.belongs_to(app)
    }

    pub(crate) fn source_rules(
        &self,
        source: &PolicySourceIdentity,
    ) -> Result<&[SiteRule], ConfigError> {
        match source {
            PolicySourceIdentity::Generated => Ok(&self.layers.runtime.site_rules),
            PolicySourceIdentity::Temporary => Ok(&self.layers.temporary.site_rules),
            PolicySourceIdentity::Authored { document_id } => self
                .layers
                .base_authored
                .documents()
                .get(
                    usize::try_from(*document_id)
                        .map_err(|_| invalid("unknown authored document"))?,
                )
                .map(|document| document.site_rules().unwrap_or_default())
                .ok_or_else(|| invalid("unknown authored document")),
            _ => Err(invalid("unsupported Site Policies source")),
        }
    }

    pub(crate) fn rule_snapshots(
        &self,
        source: &PolicySourceIdentity,
        id: &str,
    ) -> Result<(Option<SiteRule>, Option<SiteRule>), ConfigError> {
        Ok(self
            .rule_snapshots_many(source, &[id])?
            .pop()
            .expect("one requested rule snapshot"))
    }

    pub(crate) fn rule_snapshots_many(
        &self,
        source: &PolicySourceIdentity,
        ids: &[&str],
    ) -> Result<Vec<RuleSnapshotPair>, ConfigError> {
        let original = self.source_rules(source)?;
        let empty = RuntimeOverrides::default();
        let lower = if matches!(
            source,
            PolicySourceIdentity::Generated | PolicySourceIdentity::Temporary
        ) {
            let generated = if matches!(source, PolicySourceIdentity::Temporary) {
                &self.layers.runtime
            } else {
                &empty
            };
            Some(resolve_site_policy_rules(
                &self.layers.base,
                &self.layers.base_authored,
                generated,
                &empty,
                revisions(self.configuration_revision()),
            )?)
        } else {
            None
        };
        Ok(ids
            .iter()
            .map(|id| {
                (
                    original.iter().find(|rule| rule.id == *id).cloned(),
                    lower
                        .as_ref()
                        .and_then(|lower| lower.surviving_rule(id))
                        .map(|rule| rule.rule().clone()),
                )
            })
            .collect())
    }

    #[must_use]
    pub const fn configuration_revision(&self) -> u64 {
        self.request.revision
    }
    #[must_use]
    pub const fn shared_profile_revision(&self) -> u64 {
        self.shared_profile_revision
    }

    fn editable_layer(
        &self,
        destination: &PolicySourceIdentity,
    ) -> Result<&RuntimeOverrides, ConfigError> {
        if self.saved.is_some() && !matches!(destination, PolicySourceIdentity::Generated) {
            return Err(invalid(
                "a saved Interaction capture edits only the generated source",
            ));
        }
        match destination {
            PolicySourceIdentity::Generated => Ok(&self.layers.runtime),
            PolicySourceIdentity::Temporary => Ok(&self.layers.temporary),
            PolicySourceIdentity::Authored { document_id } => {
                if usize::try_from(*document_id)
                    .ok()
                    .and_then(|index| self.layers.base_authored.documents().get(index))
                    .is_none()
                {
                    return Err(invalid(
                        "the authored source is unavailable in this capture",
                    ));
                }
                Err(invalid(
                    "authored site rules are read-only; copy to generated or temporary",
                ))
            }
            _ => Err(invalid(
                "Interaction edits require the generated or temporary destination",
            )),
        }
    }
}

/// Each operation names an exact destination ID. Expected patterns are literal
/// guards, never a fallback search for another rule with the same pattern.
#[derive(Clone, Debug)]
pub enum InteractionRuleEdit {
    /// Explicit whole-rule authoring, separate from constrained Interaction
    /// imports. The captured ordinary settings must be preserved exactly;
    /// pattern/priority changes are visible in the resulting full-rule diff.
    Reviewed {
        id: String,
        expected_pattern: String,
        proposed: Option<SiteRule>,
        acknowledge_lower_replacement: bool,
    },
    Create {
        id: String,
        pattern: String,
        priority: i32,
        interaction: InteractionPolicyPatch,
        acknowledge_lower_replacement: bool,
    },
    Replace {
        id: String,
        expected_pattern: String,
        interaction: InteractionPolicyPatch,
    },
    Unset {
        id: String,
        expected_pattern: String,
    },
}
impl InteractionRuleEdit {
    fn id(&self) -> &str {
        match self {
            Self::Create { id, .. }
            | Self::Replace { id, .. }
            | Self::Unset { id, .. }
            | Self::Reviewed { id, .. } => id,
        }
    }
    fn pattern(&self) -> &str {
        match self {
            Self::Create { pattern, .. } => pattern,
            Self::Replace {
                expected_pattern, ..
            }
            | Self::Unset {
                expected_pattern, ..
            }
            | Self::Reviewed {
                expected_pattern, ..
            } => expected_pattern,
        }
    }
    fn validate_identity(&self) -> Result<(), ConfigError> {
        // Reuse the canonical rule-envelope validator, including the existing
        // slug and HTTP(S) pattern contract. No partial operation is applied.
        validate_site_rule(&SiteRule {
            id: self.id().into(),
            pattern: self.pattern().into(),
            priority: 0,
            set: BTreeMap::new(),
            interaction: None,
        })
    }
}

/// A local full-rule review snapshot. Interchange export must explicitly omit
/// ordinary settings; this type intentionally has no blanket serialization.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractionRuleSnapshot {
    pub rule: SiteRule,
    pub origin: PolicyFieldOrigin,
}
impl InteractionRuleSnapshot {
    fn observed(rule: SurvivingSitePolicyRule<'_>) -> Self {
        Self {
            rule: rule.rule().clone(),
            origin: rule.origin().clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionRuleChange {
    pub id: String,
    pub original: Option<SiteRule>,
    pub proposed: Option<SiteRule>,
    pub seed: Option<InteractionRuleSnapshot>,
    /// Winner in the prospective desired layers, not an active-after-runtime
    /// consumer. This also shows higher shadows and lower rules revealed by
    /// pruning an otherwise empty destination rule.
    pub desired_winner: Option<InteractionRuleSnapshot>,
}

pub struct PreparedInteractionEdits {
    configuration: PreparedConfigurationUpdate,
    destination: PolicySourceIdentity,
    changes: Vec<InteractionRuleChange>,
}
impl fmt::Debug for PreparedInteractionEdits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedInteractionEdits")
            .field("destination", &self.destination)
            .field("change_count", &self.changes.len())
            .finish_non_exhaustive()
    }
}
impl PreparedInteractionEdits {
    #[must_use]
    pub const fn prepared(&self) -> &PreparedConfigurationUpdate {
        &self.configuration
    }
    #[must_use]
    pub fn into_prepared(self) -> PreparedConfigurationUpdate {
        self.configuration
    }
    #[must_use]
    pub const fn destination(&self) -> &PolicySourceIdentity {
        &self.destination
    }
    #[must_use]
    pub fn changes(&self) -> &[InteractionRuleChange] {
        &self.changes
    }
    #[must_use]
    pub fn changed(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.original != change.proposed)
    }
}

impl BrowserApplication {
    /// Captures original desired layers and both revision domains without
    /// reserving, writing or publishing. Run outside the physical input stack.
    ///
    /// # Errors
    /// Rejects missing/busy/retired profiles, unsynchronized shared settings,
    /// inconsistent source catalogues and invalid raw inputs.
    pub fn capture_interaction_edits(&self) -> Result<InteractionEditCapture, ConfigError> {
        self.ensure_configuration_writable()?;
        if self.active_profile.is_none() {
            return Err(invalid("Interaction edits require an open profile"));
        }
        let request = self
            .capture_configuration_request()
            .ok_or_else(|| invalid("the configuration owner is unavailable"))?;
        let shared_profile_revision =
            self.profile_configuration_revision_for(self.configuration_layers())?;
        let layers = Arc::clone(&self.configuration.layers);
        layers.base_authored.validate_base_settings(&layers.base)?;
        checked(&layers, request.revision)?;
        Ok(InteractionEditCapture {
            request,
            saved: None,
            shared_profile_revision,
            layers,
        })
    }

    /// Explicitly captures the saved generated source, including a typed graph
    /// retained after engine CSS rejection. This does not adopt or repair any
    /// unrelated setting. Temporary and authored destinations are not editable
    /// through this capture.
    ///
    /// # Errors
    /// Rejects unavailable/busy owners and invalid source catalogues/static data.
    pub fn capture_saved_interaction_edits(&self) -> Result<InteractionEditCapture, ConfigError> {
        let saved = self.capture_saved_runtime_edits()?;
        let layers = Arc::clone(&saved.layers);
        layers.base_authored.validate_base_settings(&layers.base)?;
        checked(&layers, saved.configuration_revision())?;
        Ok(InteractionEditCapture {
            request: saved.request.clone(),
            shared_profile_revision: saved.shared_profile_revision(),
            layers,
            saved: Some(saved),
        })
    }

    /// Prepares one atomic complete-section batch through the existing runtime
    /// update boundary. CSS/effective-context validation and optional activation
    /// remain the adapter's next stages; this grants no renderer/input authority.
    ///
    /// # Errors
    /// Rejects stale captures (including shared-value ABA), unsupported sources,
    /// duplicate/missing IDs, pattern/envelope conflicts and invalid final layers.
    pub fn prepare_interaction_edits(
        &self,
        capture: &InteractionEditCapture,
        destination: &PolicySourceIdentity,
        operations: &[InteractionRuleEdit],
    ) -> Result<PreparedInteractionEdits, ConfigError> {
        self.ensure_configuration_writable()?;
        if !self.configuration_request_is_current(&capture.request) {
            return Err(invalid(
                "the Interaction edit capture belongs to a retired configuration",
            ));
        }
        if let Some(saved) = &capture.saved {
            self.check_saved_runtime_edit_capture(saved)?;
        } else if self.profile_configuration_revision_for(&capture.layers)?
            != capture.shared_profile_revision
        {
            return Err(invalid(
                "shared profile settings changed since the Interaction edit capture",
            ));
        }
        let original = capture.editable_layer(destination)?;
        validate_operations(operations)?;
        let empty = RuntimeOverrides::default();
        let lower_generated = if matches!(destination, PolicySourceIdentity::Temporary) {
            &capture.layers.runtime
        } else {
            &empty
        };
        let lower = resolve_site_policy_rules(
            &capture.layers.base,
            &capture.layers.base_authored,
            lower_generated,
            &empty,
            revisions(capture.configuration_revision()),
        )?;
        let mut changes = operations
            .iter()
            .map(|operation| propose(operation, original, &lower))
            .collect::<Result<Vec<_>, _>>()?;
        let mut layers = (*capture.layers).clone();
        let rules = final_rules(&original.site_rules, &changes);
        match destination {
            PolicySourceIdentity::Generated => layers.runtime.site_rules = rules,
            PolicySourceIdentity::Temporary => layers.temporary.site_rules = rules,
            _ => unreachable!("editable_layer checked the destination"),
        }
        // Validate the final graph once, never an intermediate create-before-
        // removal count. The canonical resolver still checks every boundary.
        checked(&layers, capture.configuration_revision())?;
        let configuration = if let Some(saved) = &capture.saved {
            self.prepare_saved_runtime_edits(saved, layers.runtime)?
        } else {
            self.prepare_configuration_update(layers)?
        };
        // Preparation sampled shared state again. The ordinary writer/publisher
        // checks this same stamp under its own reservation/lock afterwards.
        if configuration.profile_revision != capture.shared_profile_revision {
            return Err(invalid(
                "shared profile settings changed while preparing Interaction edits",
            ));
        }
        let sources = configuration.policy_sources();
        let desired = sources.desired_layers();
        let resolved = resolve_site_policy_rules(
            &desired.base,
            &desired.base_authored,
            &desired.runtime,
            &desired.temporary,
            revisions(sources.revision()),
        )?;
        for change in &mut changes {
            change.desired_winner = resolved
                .surviving_rule(&change.id)
                .map(InteractionRuleSnapshot::observed);
        }
        Ok(PreparedInteractionEdits {
            configuration,
            destination: destination.clone(),
            changes,
        })
    }
}

fn validate_operations(operations: &[InteractionRuleEdit]) -> Result<(), ConfigError> {
    if operations.is_empty() || operations.len() > MAX_SITE_POLICY_IMPORT_OPERATIONS {
        return Err(invalid(
            "an Interaction edit batch requires 1..=256 operations",
        ));
    }
    let mut seen = BTreeSet::new();
    for operation in operations {
        operation.validate_identity()?;
        if !seen.insert(operation.id()) {
            return Err(invalid(format_args!(
                "site rule {} occurs more than once in this edit batch",
                operation.id()
            )));
        }
    }
    Ok(())
}

fn original_rule(
    operation: &InteractionRuleEdit,
    original_layer: &RuntimeOverrides,
) -> Result<Option<SiteRule>, ConfigError> {
    let id = operation.id();
    let original = original_layer
        .site_rules
        .iter()
        .find(|rule| rule.id == id)
        .cloned();
    if let Some(existing) = &original
        && existing.pattern != operation.pattern()
    {
        return Err(invalid(format_args!(
            "site rule {id} belongs to a different pattern"
        )));
    }
    Ok(original)
}

fn propose(
    operation: &InteractionRuleEdit,
    original_layer: &RuntimeOverrides,
    lower: &ResolvedSitePolicyRules<'_, '_>,
) -> Result<InteractionRuleChange, ConfigError> {
    let id = operation.id();
    let original = original_rule(operation, original_layer)?;
    let mut seed = None;
    let proposed = match operation {
        InteractionRuleEdit::Reviewed {
            proposed,
            acknowledge_lower_replacement,
            ..
        } => {
            if original.is_none() {
                seed = lower
                    .surviving_rule(id)
                    .map(InteractionRuleSnapshot::observed);
            }
            reviewed_proposal(
                id,
                operation.pattern(),
                proposed.as_ref(),
                original.as_ref(),
                seed.as_ref(),
                *acknowledge_lower_replacement,
            )?
        }
        InteractionRuleEdit::Create {
            pattern,
            priority,
            interaction,
            acknowledge_lower_replacement,
            ..
        } => {
            if original.is_some() {
                return Err(invalid(format_args!(
                    "site rule {id} already exists in the destination"
                )));
            }
            seed = lower
                .surviving_rule(id)
                .map(InteractionRuleSnapshot::observed);
            if seed.is_some() && !acknowledge_lower_replacement {
                return Err(invalid(format_args!(
                    "site rule {id} replaces a lower-source snapshot; explicit acknowledgement is required"
                )));
            }
            if let Some(lower) = &seed
                && !lower.rule.set.is_empty()
                && (lower.rule.pattern != *pattern || lower.rule.priority != *priority)
            {
                return Err(invalid(format_args!(
                    "site rule {id} has ordinary settings; retain its pattern/priority or use a new ID or explicit whole-rule retarget draft"
                )));
            }
            let mut rule = seed.as_ref().map_or_else(
                || SiteRule {
                    id: id.into(),
                    pattern: pattern.clone(),
                    priority: *priority,
                    set: BTreeMap::new(),
                    interaction: None,
                },
                |seed| seed.rule.clone(),
            );
            rule.pattern.clone_from(pattern);
            rule.priority = *priority;
            rule.interaction = Some(interaction.clone());
            Some(rule)
        }
        InteractionRuleEdit::Replace { interaction, .. } => {
            let mut rule = original.clone().ok_or_else(|| {
                invalid(format_args!(
                    "site rule {id} does not exist in the destination"
                ))
            })?;
            rule.interaction = Some(interaction.clone());
            Some(rule)
        }
        InteractionRuleEdit::Unset { .. } => {
            let mut rule = original.clone().ok_or_else(|| {
                invalid(format_args!(
                    "site rule {id} does not exist in the destination"
                ))
            })?;
            if rule.interaction.take().is_none() {
                return Err(invalid(format_args!(
                    "site rule {id} has no Interaction section in the destination"
                )));
            }
            (!rule.set.is_empty()).then_some(rule)
        }
    };
    Ok(InteractionRuleChange {
        id: id.into(),
        original,
        proposed,
        seed,
        desired_winner: None,
    })
}

fn reviewed_proposal(
    id: &str,
    expected_pattern: &str,
    proposed: Option<&SiteRule>,
    original: Option<&SiteRule>,
    seed: Option<&InteractionRuleSnapshot>,
    acknowledge_lower: bool,
) -> Result<Option<SiteRule>, ConfigError> {
    if let Some(seed) = seed {
        if !acknowledge_lower {
            return Err(invalid(
                "copying a lower-source rule requires explicit acknowledgement",
            ));
        }
        if seed.rule.pattern != expected_pattern {
            return Err(invalid(
                "the lower-source rule no longer matches the reviewed snapshot",
            ));
        }
    }
    let basis = original.or_else(|| seed.map(|seed| &seed.rule));
    match proposed {
        Some(rule) => {
            if rule.id != id || basis.map_or(!rule.set.is_empty(), |basis| rule.set != basis.set) {
                return Err(invalid(
                    "a Site Policies draft must preserve the exact rule ID and ordinary settings",
                ));
            }
            validate_site_rule(rule)?;
            Ok((rule.interaction.is_some() || !rule.set.is_empty()).then(|| rule.clone()))
        }
        None if basis.is_some_and(|rule| rule.set.is_empty()) => Ok(None),
        None => Err(invalid(
            "removing Interaction must preserve ordinary settings; the rule cannot be deleted",
        )),
    }
}

fn final_rules(original: &[SiteRule], changes: &[InteractionRuleChange]) -> Vec<SiteRule> {
    let changes_by_id = changes
        .iter()
        .map(|change| (change.id.as_str(), change))
        .collect::<BTreeMap<_, _>>();
    let mut rules = original
        .iter()
        .filter_map(|rule| {
            changes_by_id
                .get(rule.id.as_str())
                .map_or_else(|| Some(rule.clone()), |change| change.proposed.clone())
        })
        .collect::<Vec<_>>();
    rules.extend(
        changes
            .iter()
            .filter(|change| change.original.is_none())
            .filter_map(|change| change.proposed.clone()),
    );
    rules
}
fn revisions(revision: u64) -> SitePolicySourceRevisions {
    SitePolicySourceRevisions {
        authored: revision,
        generated: revision,
        temporary: revision,
    }
}
fn invalid(message: impl fmt::Display) -> ConfigError {
    ConfigError::validation(format_args!("{message}"))
}
