//! Exchange operations seed from one local capture. No imported ordinary
//! settings, filesystem paths or implicit runtime sources enter a draft.
use super::{SitePolicyDraftResolution, SitePolicyDraftRule, SitePolicyDraftView, source_identity};
use crate::{BrowserApplication, InteractionRuleEdit};
use ferric_browser_config::{SitePolicyExchange, SitePolicyExchangeOperation, SiteRule};
use ferric_browser_core::{SitePolicyExportSubject, SitePolicySource};

pub(super) fn resolution(change: &crate::InteractionRuleChange) -> SitePolicyDraftResolution {
    use ferric_browser_config::PolicySourceIdentity;
    SitePolicyDraftResolution {
        winner: change
            .desired_winner
            .as_ref()
            .map(|winner| winner.rule.clone()),
        source: change
            .desired_winner
            .as_ref()
            .map(|winner| match winner.origin.source() {
                PolicySourceIdentity::Generated => "generated".into(),
                PolicySourceIdentity::Temporary => "temporary".into(),
                PolicySourceIdentity::Authored { document_id } => format!("authored:{document_id}"),
                _ => "in-memory".into(),
            }),
    }
}

fn imported_operation(
    operation: &SitePolicyExchangeOperation,
    original: Option<&SiteRule>,
    lower: Option<&SiteRule>,
    replace_existing: bool,
) -> Result<InteractionRuleEdit, String> {
    let basis = original.or(lower);
    let proposed = match operation {
        SitePolicyExchangeOperation::UpsertInteraction {
            id,
            pattern,
            priority,
            interaction,
        } => {
            if basis.is_some() && !replace_existing {
                return Err(format!(
                    "rule {id} already exists; use --replace-existing to review a snapshot replacement"
                ));
            }
            if basis.is_some_and(|rule| {
                !rule.set.is_empty() && (rule.pattern != *pattern || rule.priority != *priority)
            }) {
                return Err(format!(
                    "rule {id} has ordinary settings; retain its exact pattern/priority or use a new ID or explicit whole-rule edit"
                ));
            }
            let mut rule = basis.cloned().unwrap_or_else(|| SiteRule {
                id: id.clone(),
                pattern: pattern.clone(),
                priority: *priority,
                set: std::collections::BTreeMap::new(),
                interaction: None,
            });
            rule.pattern.clone_from(pattern);
            rule.priority = *priority;
            rule.interaction = Some(interaction.as_ref().clone());
            Some(rule)
        }
        SitePolicyExchangeOperation::UnsetInteraction { id, pattern } => {
            let mut rule = original
                .cloned()
                .ok_or_else(|| format!("unset requires existing destination rule {id}"))?;
            if rule.pattern != *pattern {
                return Err(format!(
                    "unset pattern guard does not match destination rule {id}"
                ));
            }
            if rule.interaction.take().is_none() {
                return Err(format!("destination rule {id} has no Interaction section"));
            }
            (!rule.set.is_empty()).then_some(rule)
        }
    };
    Ok(InteractionRuleEdit::Reviewed {
        id: operation.id().into(),
        expected_pattern: basis
            .or(proposed.as_ref())
            .expect("upsert or existing unset basis")
            .pattern
            .clone(),
        proposed,
        acknowledge_lower_replacement: original.is_none() && lower.is_some(),
    })
}

fn export_rule(rule: &SiteRule) -> Result<SitePolicyExchangeOperation, String> {
    Ok(SitePolicyExchangeOperation::UpsertInteraction {
        id: rule.id.clone(),
        pattern: rule.pattern.clone(),
        priority: rule.priority,
        interaction: Box::new(
            rule.interaction
                .clone()
                .ok_or("rule has no Interaction section to export")?,
        ),
    })
}

fn export_draft_rule(rule: &SitePolicyDraftRule) -> Result<SitePolicyExchangeOperation, String> {
    if let Some(proposed) = rule
        .proposed
        .as_ref()
        .filter(|rule| rule.interaction.is_some())
    {
        return export_rule(proposed);
    }
    let original = rule
        .original
        .as_ref()
        .filter(|rule| rule.interaction.is_some())
        .ok_or("an unset export requires an existing destination Interaction section")?;
    if rule.proposed.as_ref().is_some_and(|proposed| {
        proposed.pattern != original.pattern || proposed.priority != original.priority
    }) {
        return Err(
            "unset exchange cannot carry a whole-rule retarget; export that edit separately".into(),
        );
    }
    Ok(SitePolicyExchangeOperation::UnsetInteraction {
        id: rule.id.clone(),
        pattern: original.pattern.clone(),
    })
}

impl BrowserApplication {
    /// Creates one bounded, isolated batch draft after validating every local
    /// replacement guard and the complete prospective configuration.
    /// # Errors
    /// Rejects malformed operations, unacknowledged collisions, mixed-purpose
    /// retargets, stale owners and draft capacity. No partial draft is installed.
    pub fn import_site_policy_draft(
        &self,
        document: &SitePolicyExchange,
        temporary: bool,
        replace_existing: bool,
    ) -> Result<SitePolicyDraftView, String> {
        self.import_site_policy_draft_with_id(
            &uuid::Uuid::new_v4().to_string(),
            document,
            temporary,
            replace_existing,
        )
    }

    /// Completes an asynchronous file request under its preallocated draft ID.
    /// The ID grants no source authority and never replaces an existing draft.
    /// # Errors
    /// Rejects duplicate/invalid IDs and all ordinary import validation failures.
    pub fn import_site_policy_draft_with_id(
        &self,
        id: &str,
        document: &SitePolicyExchange,
        temporary: bool,
        replace_existing: bool,
    ) -> Result<SitePolicyDraftView, String> {
        let id = super::parse_id(id)?;
        document.validate()?;
        let token = if temporary {
            SitePolicySource::Temporary
        } else {
            SitePolicySource::Generated
        };
        let source = source_identity(&token);
        let capture = self.draft_capture(&source)?;
        let ids = document
            .rules()
            .iter()
            .map(SitePolicyExchangeOperation::id)
            .collect::<Vec<_>>();
        let snapshots = capture
            .rule_snapshots_many(&source, &ids)
            .map_err(|error| error.to_string())?;
        let operations = document
            .rules()
            .iter()
            .zip(&snapshots)
            .map(|(operation, (original, lower))| {
                imported_operation(
                    operation,
                    original.as_ref(),
                    lower.as_ref(),
                    replace_existing,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepared = self
            .prepare_interaction_edits(&capture, &source, &operations)
            .map_err(|error| error.to_string())?;
        let rules = prepared
            .changes()
            .iter()
            .map(|change| SitePolicyDraftRule {
                id: change.id.clone(),
                original: change.original.clone(),
                copied_from: change.seed.as_ref().map(|seed| seed.rule.clone()),
                proposed: change.proposed.clone(),
                resolution: Some(resolution(change)),
            })
            .collect();
        self.insert_site_policy_draft(
            id,
            capture,
            source,
            false,
            rules,
            document.required_capabilities(),
        )
    }

    /// Captures exact source data or the current immutable draft version as a
    /// constrained document. Ordinary settings and incidental page data are
    /// excluded; this method performs no filesystem write.
    /// # Errors
    /// Rejects unknown/foreign drafts, unknown sources/IDs, missing sections,
    /// invalid data and unrepresentable removal/retarget combinations.
    pub fn export_site_policy(
        &self,
        subject: &SitePolicyExportSubject,
    ) -> Result<SitePolicyExchange, String> {
        match subject {
            SitePolicyExportSubject::Draft(id) => {
                let draft = self.site_policy_draft(id)?;
                let rules = draft
                    .rules
                    .iter()
                    .map(export_draft_rule)
                    .collect::<Result<Vec<_>, _>>()?;
                SitePolicyExchange::with_requirements(rules, draft.required_capabilities)
            }
            SitePolicyExportSubject::Rule(target) => {
                let source = source_identity(&target.source);
                let capture = self.draft_capture(&source)?;
                let rule = capture
                    .source_rules(&source)
                    .map_err(|error| error.to_string())?
                    .iter()
                    .find(|rule| rule.id == target.id)
                    .ok_or("rule does not exist in the selected source")?;
                SitePolicyExchange::new(vec![export_rule(rule)?])
            }
        }
    }
}
