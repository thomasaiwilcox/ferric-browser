//! Bounded management state in the existing profile session. Drafts never
//! participate in policy resolution and never own a configuration writer.
use std::collections::{BTreeMap, BTreeSet};

use ferric_browser_config::{InteractionPolicyPatch, PolicySourceIdentity, SiteRule};
use ferric_browser_core::{SitePolicyEdit, SitePolicySource};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    BrowserApplication, InteractionEditCapture, InteractionRuleEdit, PreparedInteractionEdits,
};

mod apply;
mod interchange;
mod preview;
pub use apply::{SitePolicyApplyOutcome, SitePolicyDraftApply, SitePolicyDraftOperation};
pub use preview::{PreparedSitePolicyPreview, SitePolicyPreviewReceipt};

pub const MAX_SITE_POLICY_DRAFTS: usize = 16;

#[derive(Clone, Serialize)]
pub struct SitePolicyDraftRule {
    pub id: String,
    pub original: Option<SiteRule>,
    pub copied_from: Option<SiteRule>,
    pub proposed: Option<SiteRule>,
    /// Canonical prospective winner at this draft revision, before URL matching.
    /// None means not yet inspected, never an absence/availability claim.
    pub resolution: Option<SitePolicyDraftResolution>,
}

#[derive(Clone, Serialize)]
pub struct SitePolicyDraftResolution {
    pub winner: Option<SiteRule>,
    pub source: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct SitePolicyDraftValidation {
    pub revision: u64,
    pub valid: bool,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct SitePolicyDraftView {
    pub id: String,
    pub revision: u64,
    pub source: String,
    pub configuration_revision: u64,
    pub profile_revision: u64,
    pub read_only: bool,
    pub dirty: bool,
    pub dirty_fields: Vec<String>,
    pub rules: Vec<SitePolicyDraftRule>,
    pub validation: Option<SitePolicyDraftValidation>,
    pub preview: Option<SitePolicyPreviewReceipt>,
    /// Only the last explicit rebase, not an accumulating history.
    pub rebased_from: Option<Vec<SitePolicyDraftRule>>,
    pub operation: Option<SitePolicyDraftOperation>,
    /// Exchange requirements survive edits/rebase; unknown requirements block
    /// apply rather than being silently dropped by a supporting-field editor.
    pub required_capabilities: Vec<String>,
}

pub(crate) struct SitePolicyDraft {
    capture: InteractionEditCapture,
    source: PolicySourceIdentity,
    view: SitePolicyDraftView,
    pending: Option<apply::PendingApply>,
}

#[derive(Default)]
pub(crate) struct SitePolicyDraftCatalog {
    drafts: BTreeMap<Uuid, SitePolicyDraft>,
}

impl SitePolicyDraft {
    fn check(&self, app: &BrowserApplication, revision: u64, editable: bool) -> Result<(), String> {
        if !self.capture.belongs_to(app) {
            return Err("draft belongs to another window or retired profile".into());
        }
        if self.view.revision != revision {
            return Err("draft revision changed; review the current draft".into());
        }
        if editable && self.view.read_only {
            return Err(
                "authored rules are read-only; explicitly copy to generated or temporary".into(),
            );
        }
        Ok(())
    }

    fn operations(&self) -> Vec<InteractionRuleEdit> {
        self.view
            .rules
            .iter()
            .map(|rule| InteractionRuleEdit::Reviewed {
                id: rule.id.clone(),
                expected_pattern: rule
                    .original
                    .as_ref()
                    .or(rule.copied_from.as_ref())
                    .or(rule.proposed.as_ref())
                    .map_or_else(String::new, |rule| rule.pattern.clone()),
                proposed: rule.proposed.clone(),
                acknowledge_lower_replacement: rule.copied_from.is_some(),
            })
            .collect()
    }
}

impl BrowserApplication {
    /// Bounded management catalogue for this window only. Full rule snapshots
    /// are read separately so refreshing a list never replaces an editor.
    /// # Errors
    /// Rejects missing or unavailable profile sessions.
    pub fn site_policy_draft_summaries(&self) -> Result<serde_json::Value, String> {
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        Ok(serde_json::Value::Array(catalog.drafts.values().filter(|draft|draft.capture.belongs_to(self)).map(|draft|
            serde_json::json!({"id":draft.view.id,"revision":draft.view.revision,"source":draft.view.source,
                "dirty":draft.view.dirty,"read_only":draft.view.read_only,"rule_count":draft.view.rules.len(),
                "rule_ids":draft.view.rules.iter().map(|rule|&rule.id).collect::<Vec<_>>(),
                "validation":draft.view.validation,"operation":draft.view.operation})).collect()))
    }

    /// Opens an isolated exact-source draft. Authored entries remain read-only;
    /// `new` never silently copies/shadows an existing lower-source ID.
    ///
    /// # Errors
    /// Rejects unavailable sources, existing/missing IDs, and the profile draft cap.
    pub fn open_site_policy_draft(
        &self,
        edit: &SitePolicyEdit,
    ) -> Result<SitePolicyDraftView, String> {
        let SitePolicyEdit::Rule {
            target,
            new,
            pattern,
            priority,
            remove_interaction,
            enabled,
        } = edit
        else {
            return Err("use the explicit draft rebase operation".into());
        };
        let source = source_identity(&target.source);
        let read_only = matches!(source, PolicySourceIdentity::Authored { .. });
        if read_only
            && (*new
                || pattern.is_some()
                || priority.is_some()
                || *remove_interaction
                || enabled.is_some())
        {
            return Err("authored rules are read-only; explicitly copy before editing".into());
        }
        let capture = self.draft_capture(&source)?;
        let (original, lower) = capture
            .rule_snapshots(&source, &target.id)
            .map_err(|error| error.to_string())?;
        let mut proposed = if *new {
            if original.is_some() || lower.is_some() {
                return Err(
                    "rule ID already exists; choose a new ID or explicitly copy the existing rule"
                        .into(),
                );
            }
            Some(SiteRule {
                id: target.id.clone(),
                pattern: pattern.clone().ok_or("new rule requires a pattern")?,
                priority: priority.unwrap_or(0),
                set: BTreeMap::new(),
                interaction: Some(InteractionPolicyPatch::default()),
            })
        } else {
            Some(
                original
                    .clone()
                    .ok_or("rule does not exist in the selected source")?,
            )
        };
        if let Some(rule) = &mut proposed {
            if let Some(pattern) = pattern {
                rule.pattern.clone_from(pattern);
            }
            if let Some(priority) = priority {
                rule.priority = *priority;
            }
            if *remove_interaction {
                if rule.interaction.take().is_none() {
                    return Err("rule has no Interaction section".into());
                }
                if rule.set.is_empty() {
                    proposed = None;
                }
            } else if let Some(enabled) = enabled {
                rule.interaction
                    .get_or_insert_with(InteractionPolicyPatch::default)
                    .enabled = Some(*enabled);
            }
        }
        let rules = vec![SitePolicyDraftRule {
            id: target.id.clone(),
            original,
            copied_from: None,
            proposed,
            resolution: None,
        }];
        check_proposal(&rules[0], rules[0].proposed.as_ref())?;
        self.insert_site_policy_draft(
            Uuid::new_v4(),
            capture,
            source,
            read_only,
            rules,
            Vec::new(),
        )
    }

    fn draft_capture(
        &self,
        source: &PolicySourceIdentity,
    ) -> Result<InteractionEditCapture, String> {
        if matches!(source, PolicySourceIdentity::Generated)
            && self
                .profile_snapshot()
                .is_some_and(|profile| profile.durable)
        {
            self.capture_saved_interaction_edits()
        } else {
            self.capture_interaction_edits()
        }
        .map_err(|error| error.to_string())
    }

    fn insert_site_policy_draft(
        &self,
        id: Uuid,
        capture: InteractionEditCapture,
        source: PolicySourceIdentity,
        read_only: bool,
        rules: Vec<SitePolicyDraftRule>,
        required_capabilities: Vec<String>,
    ) -> Result<SitePolicyDraftView, String> {
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        if catalog.drafts.len() >= MAX_SITE_POLICY_DRAFTS {
            return Err(
                "this profile already has 16 drafts; explicitly discard a draft first".into(),
            );
        }
        if catalog.drafts.contains_key(&id) {
            return Err("draft identity already exists".into());
        }
        let token = match &source {
            PolicySourceIdentity::Generated => "generated".into(),
            PolicySourceIdentity::Temporary => "temporary".into(),
            PolicySourceIdentity::Authored { document_id } => format!("authored:{document_id}"),
            _ => return Err("unsupported draft source".into()),
        };
        let dirty_fields = dirty_fields(&rules);
        let unsupported = required_capabilities
            .iter()
            .filter(|name| {
                !ferric_browser_config::SITE_POLICY_EXCHANGE_CAPABILITIES.contains(&name.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        let view = SitePolicyDraftView {
            id: id.to_string(),
            revision: 1,
            source: token,
            configuration_revision: capture.configuration_revision(),
            profile_revision: capture.shared_profile_revision(),
            read_only,
            dirty: !dirty_fields.is_empty(),
            dirty_fields,
            rules,
            validation: (!required_capabilities.is_empty()).then(|| SitePolicyDraftValidation {
                revision: 1,
                valid: unsupported.is_empty(),
                message: if unsupported.is_empty() {
                    "Static validation passed; engine CSS and live targets are not checked".into()
                } else {
                    format!(
                        "Unsupported required capabilities: {}",
                        unsupported.join(", ")
                    )
                },
            }),
            preview: None,
            rebased_from: None,
            operation: None,
            required_capabilities,
        };
        catalog.drafts.insert(
            id,
            SitePolicyDraft {
                capture,
                source,
                view: view.clone(),
                pending: None,
            },
        );
        Ok(view)
    }

    /// Returns only this window's immutable draft snapshot.
    ///
    /// # Errors
    /// Rejects unknown drafts and foreign/retired profile owners.
    pub fn site_policy_draft(&self, id: &str) -> Result<SitePolicyDraftView, String> {
        let id = parse_id(id)?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get(&id).ok_or("unknown draft")?;
        draft.check(self, draft.view.revision, false)?;
        Ok(draft.view.clone())
    }

    /// Replaces the proposed Interaction/envelope snapshot at an exact revision.
    /// Invalid intermediate policies may remain editable; validation is explicit.
    /// Ordinary settings and exact rule identity are never editor inputs.
    ///
    /// # Errors
    /// Rejects stale versions, read-only sources and changes to ordinary settings.
    pub fn update_site_policy_draft(
        &self,
        id: &str,
        revision: u64,
        proposals: Vec<Option<SiteRule>>,
    ) -> Result<SitePolicyDraftView, String> {
        let id = parse_id(id)?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get_mut(&id).ok_or("unknown draft")?;
        draft.check(self, revision, true)?;
        if proposals.len() != draft.view.rules.len() {
            return Err("draft proposal count changed".into());
        }
        let proposals = proposals
            .into_iter()
            .map(|rule| rule.filter(|rule| rule.interaction.is_some() || !rule.set.is_empty()))
            .collect::<Vec<_>>();
        for (rule, proposed) in draft.view.rules.iter().zip(&proposals) {
            check_proposal(rule, proposed.as_ref())?;
        }
        let next = revision
            .checked_add(1)
            .ok_or("draft revision is exhausted")?;
        for (rule, proposed) in draft.view.rules.iter_mut().zip(proposals) {
            rule.proposed = proposed;
            rule.resolution = None;
        }
        draft.view.revision = next;
        draft.view.validation = None;
        draft.view.dirty_fields = dirty_fields(&draft.view.rules);
        draft.view.dirty = !draft.view.dirty_fields.is_empty();
        Ok(draft.view.clone())
    }

    /// Uses the existing configuration compiler/preparation path without
    /// activating or saving. A valid static draft is not a live preview permit.
    ///
    /// # Errors
    /// Returns stale/version/source/compiler failures while retaining the draft.
    pub fn validate_site_policy_draft(
        &self,
        id: &str,
        revision: u64,
    ) -> Result<SitePolicyDraftView, String> {
        let result = self.prepare_site_policy_draft(id, revision);
        let id = parse_id(id)?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get_mut(&id).ok_or("unknown draft")?;
        draft.check(self, revision, true)?;
        for rule in &mut draft.view.rules {
            rule.resolution = None;
        }
        if let Ok(prepared) = &result {
            for (rule, change) in draft.view.rules.iter_mut().zip(prepared.changes()) {
                rule.resolution = Some(interchange::resolution(change));
            }
        }
        draft.view.validation = Some(SitePolicyDraftValidation {
            revision,
            valid: result.is_ok(),
            message: result.err().unwrap_or_default(),
        });
        Ok(draft.view.clone())
    }

    /// Prepares the immutable reviewed version for the existing transaction
    /// coordinator. This does not admit a write or publish configuration.
    ///
    /// # Errors
    /// Rejects stale captures, changed revisions, read-only data or invalid rules.
    pub fn prepare_site_policy_draft(
        &self,
        id: &str,
        revision: u64,
    ) -> Result<PreparedInteractionEdits, String> {
        let (capture, source, operations) = {
            let id = parse_id(id)?;
            let profile = self
                .active_profile
                .as_ref()
                .ok_or("an open profile is required")?;
            let catalog = profile
                .session
                .0
                .site_policy_drafts
                .lock()
                .map_err(|_| "draft catalogue is unavailable")?;
            let draft = catalog.drafts.get(&id).ok_or("unknown draft")?;
            draft.check(self, revision, true)?;
            if draft.view.required_capabilities.iter().any(|name| {
                !ferric_browser_config::SITE_POLICY_EXCHANGE_CAPABILITIES.contains(&name.as_str())
            }) {
                return Err("draft requires unsupported exchange capabilities; review the import requirements".into());
            }
            (
                draft.capture.clone(),
                draft.source.clone(),
                draft.operations(),
            )
        };
        self.prepare_interaction_edits(&capture, &source, &operations)
            .map_err(|error| error.to_string())
    }

    /// Explicitly adopts a fresh source capture while retaining the user's
    /// changed fields. Arrays are reviewed replacements, never index-merged.
    /// Returns a new revision and both snapshots; it never applies the draft.
    ///
    /// # Errors
    /// Rejects stale versions, source creation/removal conflicts and retired owners.
    pub fn rebase_site_policy_draft(
        &self,
        id: &str,
        revision: u64,
    ) -> Result<SitePolicyDraftView, String> {
        let id = parse_id(id)?;
        let (source, previous) = {
            let profile = self
                .active_profile
                .as_ref()
                .ok_or("an open profile is required")?;
            let catalog = profile
                .session
                .0
                .site_policy_drafts
                .lock()
                .map_err(|_| "draft catalogue is unavailable")?;
            let draft = catalog.drafts.get(&id).ok_or("unknown draft")?;
            draft.check(self, revision, false)?;
            (draft.source.clone(), draft.view.rules.clone())
        };
        let next = revision
            .checked_add(1)
            .ok_or("draft revision is exhausted")?;
        let capture = self.draft_capture(&source)?;
        let rules = previous
            .iter()
            .map(|rule| rebase_rule(&capture, &source, rule))
            .collect::<Result<Vec<_>, _>>()?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get_mut(&id).ok_or("unknown draft")?;
        draft.check(self, revision, false)?;
        draft.view.configuration_revision = capture.configuration_revision();
        draft.view.profile_revision = capture.shared_profile_revision();
        draft.capture = capture;
        draft.view.revision = next;
        draft.view.rules = rules;
        draft.view.rebased_from = Some(previous);
        draft.view.validation = None;
        draft.view.dirty_fields = dirty_fields(&draft.view.rules);
        draft.view.dirty = !draft.view.dirty_fields.is_empty();
        Ok(draft.view.clone())
    }

    /// Explicitly discards management state without touching configuration.
    ///
    /// # Errors
    /// Rejects unknown drafts and another window/profile's draft identity.
    pub fn discard_site_policy_draft(&self, id: &str) -> Result<(), String> {
        let id = parse_id(id)?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get(&id).ok_or("unknown draft")?;
        draft.check(self, draft.view.revision, false)?;
        if draft
            .pending
            .as_ref()
            .is_some_and(|pending| pending.admitted)
        {
            return Err(
                "draft apply is pending; closing the editor does not cancel the writer".into(),
            );
        }
        catalog.drafts.remove(&id);
        Ok(())
    }

    pub(crate) fn retire_site_policy_drafts(&self) {
        if let Some(profile) = &self.active_profile {
            let mut catalog = profile
                .session
                .0
                .site_policy_drafts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            catalog
                .drafts
                .retain(|_, draft| !draft.capture.belongs_to(self));
        }
    }
}

fn source_identity(source: &SitePolicySource) -> PolicySourceIdentity {
    match source {
        SitePolicySource::Generated => PolicySourceIdentity::Generated,
        SitePolicySource::Temporary => PolicySourceIdentity::Temporary,
        SitePolicySource::Authored(document_id) => PolicySourceIdentity::Authored {
            document_id: *document_id,
        },
    }
}

fn parse_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|_| "unknown draft".into())
}

fn rebase_rule(
    capture: &InteractionEditCapture,
    source: &PolicySourceIdentity,
    old: &SitePolicyDraftRule,
) -> Result<SitePolicyDraftRule, String> {
    let (original, lower) = capture
        .rule_snapshots(source, &old.id)
        .map_err(|error| error.to_string())?;
    if original.is_some() != old.original.is_some()
        || old.original.is_none() && lower.is_some() != old.copied_from.is_some()
    {
        return Err(format!(
            "rule {} was created or removed concurrently; explicitly resolve its identity",
            old.id
        ));
    }
    let copied_from = old.copied_from.as_ref().and(lower);
    let old_basis = old.original.as_ref().or(old.copied_from.as_ref());
    let fresh_basis = original.as_ref().or(copied_from.as_ref());
    let proposed = match (&old.proposed, old_basis, fresh_basis) {
        (Some(proposed), Some(before), Some(current)) => {
            let before = serde_json::to_value(before).map_err(|error| error.to_string())?;
            let proposed = serde_json::to_value(proposed).map_err(|error| error.to_string())?;
            let fresh = serde_json::to_value(current).map_err(|error| error.to_string())?;
            let merged = merge_changed_fields(Some(&before), Some(&proposed), Some(&fresh))
                .ok_or("draft rebase lost its rule")?;
            let mut merged: SiteRule =
                serde_json::from_value(merged).map_err(|error| error.to_string())?;
            merged.set.clone_from(&current.set);
            Some(merged)
        }
        (None, _, Some(current)) => {
            let mut current = current.clone();
            current.interaction = None;
            (!current.set.is_empty()).then_some(current)
        }
        _ => old.proposed.clone(),
    };
    let rule = SitePolicyDraftRule {
        id: old.id.clone(),
        original,
        copied_from,
        proposed,
        resolution: None,
    };
    check_proposal(&rule, rule.proposed.as_ref())?;
    Ok(rule)
}

fn merge_changed_fields(
    before: Option<&serde_json::Value>,
    proposed: Option<&serde_json::Value>,
    current: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    if before == proposed {
        return current.cloned();
    }
    if let (
        Some(serde_json::Value::Object(before)),
        Some(serde_json::Value::Object(proposed)),
        Some(serde_json::Value::Object(current)),
    ) = (before, proposed, current)
    {
        let keys = before
            .keys()
            .chain(proposed.keys())
            .chain(current.keys())
            .collect::<BTreeSet<_>>();
        Some(serde_json::Value::Object(
            keys.into_iter()
                .filter_map(|key| {
                    merge_changed_fields(before.get(key), proposed.get(key), current.get(key))
                        .map(|value| (key.clone(), value))
                })
                .collect(),
        ))
    } else {
        proposed.cloned()
    }
}

fn check_proposal(rule: &SitePolicyDraftRule, proposed: Option<&SiteRule>) -> Result<(), String> {
    if let Some(proposed) = proposed {
        if proposed.id.len() > 32
            || proposed.id.is_empty()
            || !proposed
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || proposed.pattern.len() > 512
            || proposed.pattern.chars().any(char::is_control)
        {
            return Err("draft rule identity or pattern exceeds the existing rule bounds".into());
        }
        if let Some(interaction) = &proposed.interaction {
            let bytes = serde_json::to_vec(interaction).map_err(|error| error.to_string())?;
            if bytes.len() > ferric_browser_config::MAX_INTERACTION_AUTHORED_BYTES {
                return Err("draft Interaction exceeds 64 KiB".into());
            }
        }
    }
    let basis = rule.original.as_ref().or(rule.copied_from.as_ref());
    match proposed {
        Some(proposed)
            if proposed.id != rule.id
                || basis.map_or(!proposed.set.is_empty(), |basis| proposed.set != basis.set) =>
        {
            Err("a Site Policies draft must preserve its rule ID and ordinary settings".into())
        }
        None if basis.is_none_or(|basis| !basis.set.is_empty()) => {
            Err("remove the Interaction section while preserving ordinary settings".into())
        }
        _ => Ok(()),
    }
}

fn dirty_fields(rules: &[SitePolicyDraftRule]) -> Vec<String> {
    let mut fields = BTreeSet::new();
    for rule in rules {
        let original = serde_json::to_value(&rule.original).expect("typed rule serialization");
        let proposed = serde_json::to_value(&rule.proposed).expect("typed rule serialization");
        changed_fields(&rule.id, &original, &proposed, &mut fields);
    }
    fields.into_iter().collect()
}

fn changed_fields(
    path: &str,
    original: &serde_json::Value,
    proposed: &serde_json::Value,
    fields: &mut BTreeSet<String>,
) {
    if original == proposed {
        return;
    }
    if let (Some(original), Some(proposed)) = (original.as_object(), proposed.as_object()) {
        let keys = original
            .keys()
            .chain(proposed.keys())
            .collect::<BTreeSet<_>>();
        for key in keys {
            changed_fields(
                &format!("{path}.{key}"),
                original.get(key).unwrap_or(&serde_json::Value::Null),
                proposed.get(key).unwrap_or(&serde_json::Value::Null),
                fields,
            );
        }
    } else {
        fields.insert(path.into());
    }
}
