//! Immutable effective values and winning-field provenance. This composer does
//! not match URLs, shadow rules, publish configuration, or grant page authority.
//! The existing source-aware resolver must select surviving rules and supply
//! precedence order before composition. Its catalogue retains shadowed input.

use crate::{
    AuthoredHints, CompiledInteractionKeys, HintAutoFollow, HintsConfig, InteractionAnchor,
    InteractionAutoFollow, InteractionCollection, InteractionFamily, InteractionLabelsPatch,
    InteractionPageReceiver, InteractionPolicyPatch, InteractionRemap, MAX_SITE_RULES, SiteRule,
    validate_site_rule,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

mod trace;
pub use trace::{
    InteractionCompositionTrace, InteractionContribution, InteractionContributionPatch,
};

/// Catalogue identities, never arbitrary filesystem paths or editable-source
/// authority. A document ID is local to its captured configuration revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicySourceIdentity {
    /// Standalone configuration with no authored file catalogue (including
    /// built-in defaults). Never pretends to identify a file for editing.
    InMemory,
    Authored {
        document_id: u32,
    },
    Profile {
        name: String,
    },
    Generated,
    CommandLine,
    Temporary,
}

impl PolicySourceIdentity {
    fn validate(&self) -> Result<(), String> {
        if let Self::Profile { name } = self {
            crate::validate_slug(name, "profile source").map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRuleLocation {
    id: String,
    authored_index: u16,
    priority: i32,
}

impl PolicyRuleLocation {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub const fn authored_index(&self) -> u16 {
        self.authored_index
    }
    #[must_use]
    pub const fn priority(&self) -> i32 {
        self.priority
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyFieldOrigin {
    source: PolicySourceIdentity,
    source_revision: u64,
    rule: Option<PolicyRuleLocation>,
}

impl PolicyFieldOrigin {
    pub(crate) fn selected(
        rule: &SiteRule,
        source: PolicySourceIdentity,
        source_revision: u64,
        authored_index: usize,
    ) -> Result<Self, String> {
        source.validate()?;
        if matches!(
            source,
            PolicySourceIdentity::Profile { .. } | PolicySourceIdentity::CommandLine
        ) {
            return Err("this source does not provide site rules".into());
        }
        if authored_index >= MAX_SITE_RULES {
            return Err("selected site rules exceed the existing 256-rule bound".into());
        }
        Ok(Self {
            source,
            source_revision,
            rule: Some(PolicyRuleLocation {
                id: rule.id.clone(),
                authored_index: u16::try_from(authored_index).map_err(|_| "invalid rule index")?,
                priority: rule.priority,
            }),
        })
    }

    /// Identifies an explicit global contribution. This carries provenance,
    /// never editable-source or publication permission.
    pub fn shared(source: PolicySourceIdentity, source_revision: u64) -> Result<Self, String> {
        source.validate()?;
        Ok(Self {
            source,
            source_revision,
            rule: None,
        })
    }
    #[must_use]
    pub const fn source(&self) -> &PolicySourceIdentity {
        &self.source
    }
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }
    #[must_use]
    pub const fn rule(&self) -> Option<&PolicyRuleLocation> {
        self.rule.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InteractionLabelField {
    Alphabet,
    MinChars,
    FallbackPrefix,
    ExcludedChars,
    AutoFollow,
}

impl InteractionLabelField {
    pub const ALL: [Self; 5] = [
        Self::Alphabet,
        Self::MinChars,
        Self::FallbackPrefix,
        Self::ExcludedChars,
        Self::AutoFollow,
    ];
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InteractionField {
    Enabled,
    Labels(InteractionLabelField),
    Anchors,
    Collections,
    HintPageKeys,
    PageReceiver,
    Remaps,
}

impl InteractionField {
    pub const ALL: [Self; 11] = [
        Self::Enabled,
        Self::Labels(InteractionLabelField::Alphabet),
        Self::Labels(InteractionLabelField::MinChars),
        Self::Labels(InteractionLabelField::FallbackPrefix),
        Self::Labels(InteractionLabelField::ExcludedChars),
        Self::Labels(InteractionLabelField::AutoFollow),
        Self::Anchors,
        Self::Collections,
        Self::HintPageKeys,
        Self::PageReceiver,
        Self::Remaps,
    ];

    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Labels(InteractionLabelField::Alphabet) => "labels.alphabet",
            Self::Labels(InteractionLabelField::MinChars) => "labels.min_chars",
            Self::Labels(InteractionLabelField::FallbackPrefix) => "labels.fallback_prefix",
            Self::Labels(InteractionLabelField::ExcludedChars) => "labels.excluded_chars",
            Self::Labels(InteractionLabelField::AutoFollow) => "labels.auto_follow",
            Self::Anchors => "anchors",
            Self::Collections => "collections",
            Self::HintPageKeys => "hint_page_keys",
            Self::PageReceiver => "page_receiver",
            Self::Remaps => "remaps",
        }
    }
}

/// Browser-owned revisions; these numbers alone are not an action permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionPolicyRevision {
    pub profile: ferric_browser_core::ProfileId,
    pub profile_revision: u64,
    pub configuration_revision: u64,
    pub policy_revision: u64,
}

/// Resolved configured leaves. Alphabet filtering and codebook usability are
/// separate compiler stages; these values are not a ready label namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveInteractionLabels {
    alphabet: String,
    min_chars: u8,
    fallback_prefix: String,
    excluded_chars: Vec<String>,
    auto_follow: InteractionAutoFollow,
}

impl Default for EffectiveInteractionLabels {
    fn default() -> Self {
        let hints = HintsConfig::default();
        Self {
            alphabet: hints.chars,
            min_chars: hints.min_chars,
            fallback_prefix: String::new(),
            excluded_chars: Vec::new(),
            auto_follow: auto_follow(&hints.auto_follow),
        }
    }
}

impl EffectiveInteractionLabels {
    #[must_use]
    pub fn alphabet(&self) -> &str {
        &self.alphabet
    }
    #[must_use]
    pub const fn min_chars(&self) -> u8 {
        self.min_chars
    }
    #[must_use]
    pub fn fallback_prefix(&self) -> &str {
        &self.fallback_prefix
    }
    #[must_use]
    pub fn excluded_chars(&self) -> &[String] {
        &self.excluded_chars
    }
    #[must_use]
    pub const fn auto_follow(&self) -> InteractionAutoFollow {
        self.auto_follow
    }

    fn patch(&self) -> InteractionLabelsPatch {
        InteractionLabelsPatch {
            alphabet: Some(self.alphabet.clone()),
            min_chars: Some(self.min_chars),
            fallback_prefix: Some(self.fallback_prefix.clone()),
            excluded_chars: Some(self.excluded_chars.clone()),
            auto_follow: Some(self.auto_follow),
        }
    }

    fn apply(
        &mut self,
        patch: &InteractionLabelsPatch,
        origin: &Arc<PolicyFieldOrigin>,
        provenance: &mut BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
    ) {
        macro_rules! leaf {
            ($name:ident, $field:ident) => {
                if let Some(value) = &patch.$name {
                    self.$name.clone_from(value);
                    provenance.insert(
                        InteractionField::Labels(InteractionLabelField::$field),
                        Arc::clone(origin),
                    );
                }
            };
        }
        leaf!(alphabet, Alphabet);
        leaf!(min_chars, MinChars);
        leaf!(fallback_prefix, FallbackPrefix);
        leaf!(excluded_chars, ExcludedChars);
        leaf!(auto_follow, AutoFollow);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EffectiveValues {
    enabled: bool,
    labels: EffectiveInteractionLabels,
    anchors: Vec<InteractionAnchor>,
    collections: Vec<InteractionCollection>,
    hint_page_keys: Vec<String>,
    page_receiver: InteractionPageReceiver,
    remaps: Vec<InteractionRemap>,
}

impl Default for EffectiveValues {
    fn default() -> Self {
        Self {
            enabled: true,
            labels: EffectiveInteractionLabels::default(),
            anchors: Vec::new(),
            collections: Vec::new(),
            hint_page_keys: Vec::new(),
            page_receiver: InteractionPageReceiver::default(),
            remaps: Vec::new(),
        }
    }
}

impl EffectiveValues {
    fn patch(&self) -> InteractionPolicyPatch {
        InteractionPolicyPatch {
            enabled: Some(self.enabled),
            labels: Some(self.labels.patch()),
            anchors: Some(self.anchors.clone()),
            collections: Some(self.collections.clone()),
            hint_page_keys: Some(self.hint_page_keys.clone()),
            page_receiver: Some(self.page_receiver.clone()),
            remaps: Some(self.remaps.clone()),
        }
    }
}

/// Ordered, unpublished composition. Raw rules are validated before mutation,
/// including disabled entries. Callers still validate nonselected/shadowed raw
/// sources separately. Shared settings take only explicitly present leaves.
#[derive(Clone, Debug, Default)]
pub struct InteractionPolicyBuilder {
    values: EffectiveValues,
    ordinary: EffectiveInteractionLabels,
    provenance: BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
    ordinary_provenance: BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
    rule_ids: BTreeSet<String>,
    trace: Option<InteractionCompositionTrace>,
}

impl InteractionPolicyBuilder {
    /// Enables owned diagnostic observations of the normal composition path.
    /// The default builder keeps no contribution history.
    #[must_use]
    pub fn traced() -> Self {
        Self {
            trace: Some(Vec::new()),
            ..Self::default()
        }
    }

    /// Applies one validated matching entry from the common resolver. Callers
    /// interleave explicit shared leaves at their source boundaries.
    pub fn apply_resolved_rule(
        &mut self,
        selected: &crate::SelectedSitePolicyRule<'_>,
    ) -> Result<(), String> {
        self.apply_rule_with_origin(selected.rule(), selected.shared_origin())
    }

    pub fn apply_shared_labels(
        &mut self,
        hints: &AuthoredHints,
        source: PolicySourceIdentity,
        source_revision: u64,
    ) -> Result<(), String> {
        hints.validate()?;
        let origin = Arc::new(PolicyFieldOrigin::shared(source, source_revision)?);
        let patch = InteractionLabelsPatch {
            alphabet: hints.chars.clone(),
            min_chars: hints.min_chars,
            auto_follow: hints.auto_follow.as_ref().map(auto_follow),
            ..Default::default()
        };
        self.ordinary
            .apply(&patch, &origin, &mut self.ordinary_provenance);
        self.values
            .labels
            .apply(&patch, &origin, &mut self.provenance);
        if let Some(trace) = &mut self.trace
            && (patch.alphabet.is_some()
                || patch.min_chars.is_some()
                || patch.auto_follow.is_some())
        {
            trace.push(InteractionContribution {
                origin,
                patch: InteractionContributionPatch::SharedLabels(patch),
            });
        }
        Ok(())
    }

    /// Input must already survive same-ID replacement and URL matching. No
    /// profile/CLI site-rule surface is created by this low-level composer.
    pub fn apply_selected_rule(
        &mut self,
        rule: &SiteRule,
        source: PolicySourceIdentity,
        source_revision: u64,
        authored_index: usize,
    ) -> Result<(), String> {
        let origin = Arc::new(PolicyFieldOrigin::selected(
            rule,
            source,
            source_revision,
            authored_index,
        )?);
        validate_site_rule(rule).map_err(|error| error.to_string())?;
        self.apply_rule_with_origin(rule, &origin)
    }

    /// Consumes the common resolver's order and provenance without a second
    /// matcher or a raw include-rule loop. Shared global leaves must still be
    /// interleaved at their source boundaries by the full policy compiler.
    pub fn apply_selected_rules(
        &mut self,
        rules: &crate::SelectedSitePolicyRules<'_>,
    ) -> Result<(), String> {
        // Work on a clone so failure never partially changes this builder.
        let mut candidate = self.clone();
        for selected in rules.selected() {
            candidate.apply_rule_with_origin(selected.rule(), selected.shared_origin())?;
        }
        *self = candidate;
        Ok(())
    }

    /// Compiles a site-rule feed through the same typed composition contract
    /// available to future consumers. The seed may contain only explicitly
    /// resolved shared leaves, never previously applied site rules. The feed
    /// supplies the complete surviving rule set; accepting lower rule data in
    /// a seed could resurrect a shadowed rule which no longer matches the URL.
    /// Full source-boundary interleaving belongs to the policy compiler.
    pub fn finish_selected_rules(
        self,
        rules: &crate::SelectedSitePolicyRules<'_>,
        revision: InteractionPolicyRevision,
    ) -> Result<EffectiveInteractionPolicy, String> {
        crate::compose_site_policy_section(
            rules,
            &InteractionSection {
                seed: self,
                revision,
            },
        )
    }

    fn apply_rule_with_origin(
        &mut self,
        rule: &SiteRule,
        origin: &Arc<PolicyFieldOrigin>,
    ) -> Result<(), String> {
        if self.rule_ids.len() >= MAX_SITE_RULES {
            return Err("selected site rules exceed the existing 256-rule bound".into());
        }
        if self.rule_ids.contains(&rule.id) {
            return Err("same-ID replacement must precede Interaction composition".into());
        }
        self.rule_ids.insert(rule.id.clone());
        let Some(patch) = &rule.interaction else {
            return Ok(());
        };
        self.apply_patch(patch, origin);
        Ok(())
    }

    fn apply_patch(&mut self, patch: &InteractionPolicyPatch, origin: &Arc<PolicyFieldOrigin>) {
        if let Some(labels) = &patch.labels {
            self.values
                .labels
                .apply(labels, origin, &mut self.provenance);
        }
        macro_rules! field {
            ($name:ident, $field:ident) => {
                if let Some(value) = &patch.$name {
                    self.values.$name.clone_from(value);
                    self.provenance
                        .insert(InteractionField::$field, Arc::clone(origin));
                }
            };
        }
        field!(enabled, Enabled);
        field!(anchors, Anchors);
        field!(collections, Collections);
        field!(hint_page_keys, HintPageKeys);
        field!(page_receiver, PageReceiver);
        field!(remaps, Remaps);
        if let Some(trace) = &mut self.trace {
            trace.push(InteractionContribution {
                origin: Arc::clone(origin),
                patch: InteractionContributionPatch::SiteSection(patch.clone()),
            });
        }
    }

    /// Returns history only after the same final effective validation as
    /// `finish`. An effective conflict exposes no partial winning snapshot.
    ///
    /// # Errors
    /// Rejects an untraced builder or an invalid effective combination.
    pub fn finish_with_trace(
        mut self,
        revision: InteractionPolicyRevision,
    ) -> Result<(EffectiveInteractionPolicy, InteractionCompositionTrace), String> {
        let trace = self
            .trace
            .take()
            .ok_or("Interaction tracing is not enabled")?;
        self.finish(revision).map(|policy| (policy, trace))
    }

    /// Enforces composed shape/key/collision/byte limits. Filtered alphabets,
    /// codebook capacity, enhanced auto-follow, engine CSS and live proofs are later
    /// compiler stages. A valid value snapshot is not an available action.
    pub fn finish(
        self,
        revision: InteractionPolicyRevision,
    ) -> Result<EffectiveInteractionPolicy, String> {
        let validated = self.values.patch().validate_effective()?;
        Ok(EffectiveInteractionPolicy {
            snapshot: Arc::new(InteractionSnapshot {
                values: self.values,
                ordinary: self.ordinary,
                provenance: self.provenance,
                ordinary_provenance: self.ordinary_provenance,
                keys: validated.keys,
                revision,
            }),
        })
    }
}

struct InteractionSection {
    seed: InteractionPolicyBuilder,
    revision: InteractionPolicyRevision,
}

impl crate::SitePolicySection for InteractionSection {
    type Patch = InteractionPolicyPatch;
    type Accumulator = InteractionPolicyBuilder;
    type Snapshot = EffectiveInteractionPolicy;

    fn validate_inputs(&self, selected: &crate::SelectedSitePolicyRules<'_>) -> Result<(), String> {
        // The feed's private constructor has already validated every raw
        // Interaction rule, before shadowing and matching. The seed's builder
        // also validates each raw input before mutation. Effective conflicts
        // are checked only after composition, so a later clear can repair one.
        // Site-bearing seeds can resurrect a lower rule even when its higher
        // replacement is absent from selected() because it does not match.
        // Shared-leaf seeds retain ordinary/enhanced accumulator context safely.
        if !self.seed.rule_ids.is_empty() || selected.selected().len() > MAX_SITE_RULES {
            return Err("bulk site composition requires a shared-settings-only seed".into());
        }
        Ok(())
    }

    fn initial(&self) -> Self::Accumulator {
        self.seed.clone()
    }

    fn patch<'a>(
        &'a self,
        selected: &'a crate::SelectedSitePolicyRule<'_>,
    ) -> Option<&'a Self::Patch> {
        selected.rule().interaction.as_ref()
    }

    fn apply(
        &self,
        accumulator: &mut Self::Accumulator,
        patch: &Self::Patch,
        origin: &Arc<PolicyFieldOrigin>,
    ) -> Result<(), String> {
        let id = origin.rule().ok_or("selected rule has no location")?.id();
        if accumulator.rule_ids.len() >= MAX_SITE_RULES || !accumulator.rule_ids.insert(id.into()) {
            return Err("selected rule duplicates seed or exceeds the existing rule bound".into());
        }
        accumulator.apply_patch(patch, origin);
        Ok(())
    }

    fn finish(&self, accumulator: Self::Accumulator) -> Result<Self::Snapshot, String> {
        accumulator.finish(self.revision)
    }
}

#[derive(Clone, Debug)]
struct InteractionSnapshot {
    values: EffectiveValues,
    ordinary: EffectiveInteractionLabels,
    provenance: BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
    ordinary_provenance: BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
    keys: CompiledInteractionKeys,
    revision: InteractionPolicyRevision,
}

/// Immutable resolved data and syntax, shared cheaply by sessions. No mutable
/// view or deserializer can bypass the builder's validation. Winning origins
/// are bounded by the field set; full shadow explanations use the resolver's
/// retained source catalogue rather than accumulating unbounded history here.
#[derive(Clone, Debug)]
pub struct EffectiveInteractionPolicy {
    snapshot: Arc<InteractionSnapshot>,
}

impl EffectiveInteractionPolicy {
    /// Compares compiled meaning and winning source/rule identities, excluding
    /// capture revision counters. Authored document IDs are capture-local:
    /// callers must also compare their source catalogues and owner/context.
    /// This comparison grants no session or native-input authority.
    #[must_use]
    pub fn same_meaning(&self, other: &Self) -> bool {
        fn origins_equal(
            left: &BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
            right: &BTreeMap<InteractionField, Arc<PolicyFieldOrigin>>,
        ) -> bool {
            left.len() == right.len()
                && left.iter().all(|(field, origin)| {
                    right.get(field).is_some_and(|other| {
                        origin.source == other.source && origin.rule == other.rule
                    })
                })
        }
        let left = &self.snapshot;
        let right = &other.snapshot;
        left.values == right.values
            && left.ordinary == right.ordinary
            && left.keys == right.keys
            && origins_equal(&left.provenance, &right.provenance)
            && origins_equal(&left.ordinary_provenance, &right.ordinary_provenance)
    }

    /// Assigns the browser-owned semantic revision to a freshly compiled
    /// capture. Source/profile/configuration revisions remain unchanged.
    #[must_use]
    pub fn with_policy_revision(mut self, revision: u64) -> Self {
        Arc::make_mut(&mut self.snapshot).revision.policy_revision = revision;
        self
    }

    #[must_use]
    pub fn revision(&self) -> InteractionPolicyRevision {
        self.snapshot.revision
    }
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.snapshot.values.enabled
    }
    #[must_use]
    pub fn labels(&self) -> &EffectiveInteractionLabels {
        &self.snapshot.values.labels
    }
    #[must_use]
    pub fn ordinary_labels(&self) -> &EffectiveInteractionLabels {
        &self.snapshot.ordinary
    }
    #[must_use]
    pub fn anchors(&self) -> &[InteractionAnchor] {
        &self.snapshot.values.anchors
    }
    #[must_use]
    pub fn collections(&self) -> &[InteractionCollection] {
        &self.snapshot.values.collections
    }
    #[must_use]
    pub fn hint_page_keys(&self) -> &[String] {
        &self.snapshot.values.hint_page_keys
    }
    #[must_use]
    pub fn page_receiver(&self) -> &InteractionPageReceiver {
        &self.snapshot.values.page_receiver
    }
    #[must_use]
    pub fn remaps(&self) -> &[InteractionRemap] {
        &self.snapshot.values.remaps
    }
    #[must_use]
    pub fn keys(&self) -> &CompiledInteractionKeys {
        &self.snapshot.keys
    }
    /// None denotes a built-in default, not an explicitly supplied default value.
    #[must_use]
    pub fn origin(&self, field: InteractionField) -> Option<&PolicyFieldOrigin> {
        self.snapshot.provenance.get(&field).map(Arc::as_ref)
    }
    #[must_use]
    pub fn ordinary_label_origin(
        &self,
        field: InteractionLabelField,
    ) -> Option<&PolicyFieldOrigin> {
        self.snapshot
            .ordinary_provenance
            .get(&InteractionField::Labels(field))
            .map(Arc::as_ref)
    }
    /// Requested enhanced semantics, before DOM discovery or availability
    /// checks. In particular, absent applicable anchors still request it.
    #[must_use]
    pub fn enhanced_requested_for(&self, family: InteractionFamily) -> bool {
        let values = &self.snapshot.values;
        values.enabled
            && (values.labels != self.snapshot.ordinary
                || !values.hint_page_keys.is_empty()
                || values
                    .anchors
                    .iter()
                    .any(|anchor| anchor.families.contains(&family))
                || values
                    .collections
                    .iter()
                    .any(|collection| collection.families.contains(&family)))
    }
}

fn auto_follow(value: &HintAutoFollow) -> InteractionAutoFollow {
    match value {
        HintAutoFollow::Always => InteractionAutoFollow::Always,
        HintAutoFollow::UniqueMatch => InteractionAutoFollow::UniqueMatch,
        HintAutoFollow::FullMatch => InteractionAutoFollow::FullMatch,
        HintAutoFollow::Never => InteractionAutoFollow::Never,
    }
}

#[cfg(test)]
mod tests;
