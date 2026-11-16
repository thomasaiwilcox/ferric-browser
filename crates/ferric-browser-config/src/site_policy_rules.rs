//! Shared source-aware rule selection. Raw include documents are validated but
//! never matched: only the last surviving authored array supplies base rules.

use std::{collections::BTreeMap, sync::Arc};

use crate::routing::site_pattern_matches;
use crate::{
    Config, ConfigError, ConfigurationSources, MAX_SITE_RULES, PolicyFieldOrigin,
    PolicySourceIdentity, RuntimeOverrides, SiteRule, validate_runtime_overrides,
    validate_site_rule,
};

/// Revisions belong to the browser's captured source catalogue, not page data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SitePolicySourceRevisions {
    pub authored: u64,
    pub generated: u64,
    pub temporary: u64,
}

/// A surviving, matching rule with immutable source/index/priority provenance.
/// Constructed only by the common selector; consumers do not rematch URLs.
#[derive(Clone, Debug)]
pub struct SelectedSitePolicyRule<'a> {
    rule: &'a SiteRule,
    origin: Arc<PolicyFieldOrigin>,
}

impl SelectedSitePolicyRule<'_> {
    #[must_use]
    pub const fn rule(&self) -> &SiteRule {
        self.rule
    }

    #[must_use]
    pub fn origin(&self) -> &PolicyFieldOrigin {
        &self.origin
    }

    pub(crate) fn shared_origin(&self) -> &Arc<PolicyFieldOrigin> {
        &self.origin
    }
}

/// Whole-rule replacement, retained even when the replacing rule does not
/// match the current URL. A chain records each actual replacement in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShadowedSitePolicyRule {
    pub replaced: Arc<PolicyFieldOrigin>,
    pub replacement: Arc<PolicyFieldOrigin>,
}

/// Diagnostic metadata only. A nonmatching survivor does not escape as a
/// `SelectedSitePolicyRule` that could be handed to a consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SitePolicySurvivorTrace {
    pub origin: Arc<PolicyFieldOrigin>,
    pub matched: bool,
}

/// An authored array identity, including an explicit empty array. It grants
/// neither editable-source authority nor a selected rule to any consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoredSitePolicyArray {
    pub document_id: u32,
    pub source_revision: u64,
}

/// Raw diagnostic input removed by include-array replacement, never matched.
#[derive(Clone, Debug, PartialEq)]
pub struct RetiredSitePolicyRule {
    pub origin: Arc<PolicyFieldOrigin>,
    pub raw: SiteRule,
}

/// The replacing array can be empty or contain unrelated IDs. Its document
/// identity must not be presented as a fictitious replacing rule.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplacedSitePolicyArray {
    pub array: AuthoredSitePolicyArray,
    pub replaced_by: AuthoredSitePolicyArray,
    pub rules: Vec<RetiredSitePolicyRule>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SitePolicySelectionTrace {
    pub survivors: Vec<SitePolicySurvivorTrace>,
    pub shadowed: Vec<ShadowedSitePolicyRule>,
    pub authored_array: Option<AuthoredSitePolicyArray>,
    pub replaced_arrays: Vec<ReplacedSitePolicyArray>,
}

/// One ordered feed shared by every typed consumer of this match context.
#[derive(Clone, Debug)]
pub struct SelectedSitePolicyRules<'a> {
    selected: Vec<SelectedSitePolicyRule<'a>>,
    shadowed: Vec<ShadowedSitePolicyRule>,
}

impl<'a> SelectedSitePolicyRules<'a> {
    #[must_use]
    pub fn selected(&self) -> &[SelectedSitePolicyRule<'a>] {
        &self.selected
    }

    #[must_use]
    pub fn shadowed(&self) -> &[ShadowedSitePolicyRule] {
        &self.shadowed
    }
}

/// Validated whole-rule replacement and ordering, before URL matching. This
/// permits validation of pending sources without admitting pending matches.
#[derive(Clone, Debug)]
pub struct ResolvedSitePolicyRules<'a, 'catalogue> {
    rules: Vec<SelectedSitePolicyRule<'a>>,
    shadowed: Vec<ShadowedSitePolicyRule>,
    authored: &'catalogue ConfigurationSources,
    authored_revision: u64,
}

/// An exact-ID full-rule observation after source replacement, before matching.
/// This is authoring/diagnostic data, never a selected consumer input or editable
/// source permission. In particular it cannot be handed to a policy builder as
/// a `SelectedSitePolicyRule` for a URL it has not matched.
#[derive(Clone, Copy, Debug)]
pub struct SurvivingSitePolicyRule<'a> {
    rule: &'a SiteRule,
    origin: &'a PolicyFieldOrigin,
}

impl SurvivingSitePolicyRule<'_> {
    #[must_use]
    pub const fn rule(&self) -> &SiteRule {
        self.rule
    }

    #[must_use]
    pub const fn origin(&self) -> &PolicyFieldOrigin {
        self.origin
    }
}

impl<'a> ResolvedSitePolicyRules<'a, '_> {
    /// Observes the highest surviving full rule for this exact, case-sensitive
    /// ID, regardless of its URL pattern. Retired include arrays are not part
    /// of the resolved feed and cannot be revived by this lookup.
    #[must_use]
    pub fn surviving_rule(&self, id: &str) -> Option<SurvivingSitePolicyRule<'_>> {
        self.rules
            .iter()
            .find(|selected| selected.rule.id == id)
            .map(|selected| SurvivingSitePolicyRule {
                rule: selected.rule,
                origin: &selected.origin,
            })
    }

    #[must_use]
    pub fn shadowed(&self) -> &[ShadowedSitePolicyRule] {
        &self.shadowed
    }

    #[must_use]
    pub fn select(&self, url: &str) -> SelectedSitePolicyRules<'a> {
        self.select_observed(url, |_, _| {})
    }

    #[must_use]
    pub fn select_with_trace(
        &self,
        url: &str,
    ) -> (SelectedSitePolicyRules<'a>, SitePolicySelectionTrace) {
        let mut survivors = Vec::with_capacity(self.rules.len());
        let selected = self.select_observed(url, |rule, matched| {
            survivors.push(SitePolicySurvivorTrace {
                origin: Arc::clone(&rule.origin),
                matched,
            });
        });
        let (authored_array, replaced_arrays) = self.authored_array_trace();
        (
            selected,
            SitePolicySelectionTrace {
                survivors,
                shadowed: self.shadowed.clone(),
                authored_array,
                replaced_arrays,
            },
        )
    }

    fn authored_array_trace(
        &self,
    ) -> (
        Option<AuthoredSitePolicyArray>,
        Vec<ReplacedSitePolicyArray>,
    ) {
        let mut previous: Option<(AuthoredSitePolicyArray, &[SiteRule])> = None;
        let mut replaced = Vec::new();
        for (index, document) in self.authored.documents().iter().enumerate() {
            let Some(rules) = document.site_rules() else {
                continue;
            };
            // Resolution checked the final array's document ID, hence every
            // earlier array index too. This immutable catalogue cannot change.
            let array = AuthoredSitePolicyArray {
                document_id: u32::try_from(index).expect("validated authored document index"),
                source_revision: self.authored_revision,
            };
            if let Some((retired, rules)) = previous {
                replaced.push(ReplacedSitePolicyArray {
                    array: retired,
                    replaced_by: array,
                    rules: rules
                        .iter()
                        .enumerate()
                        .map(|(index, rule)| {
                            // Every raw array was validated before resolving its
                            // replacement; no retired rule gains match authority.
                            let origin = PolicyFieldOrigin::selected(
                                rule,
                                PolicySourceIdentity::Authored {
                                    document_id: retired.document_id,
                                },
                                retired.source_revision,
                                index,
                            )
                            .expect("validated authored rule origin");
                            RetiredSitePolicyRule {
                                origin: Arc::new(origin),
                                raw: rule.clone(),
                            }
                        })
                        .collect(),
                });
            }
            previous = Some((array, rules));
        }
        (previous.map(|(array, _)| array), replaced)
    }

    fn select_observed(
        &self,
        url: &str,
        mut observe: impl FnMut(&SelectedSitePolicyRule<'a>, bool),
    ) -> SelectedSitePolicyRules<'a> {
        SelectedSitePolicyRules {
            selected: self
                .rules
                .iter()
                .filter(|selected| {
                    let matched = site_pattern_matches(&selected.rule.pattern, url);
                    observe(selected, matched);
                    matched
                })
                .cloned()
                .collect(),
            shadowed: self.shadowed.clone(),
        }
    }
}

/// Selects the existing authored/generated/temporary rule surfaces. Profile
/// and CLI shared settings do not acquire new site-rule surfaces here.
///
/// Every raw rule is statically validated before replacement or URL matching.
/// Engine CSS validation and effective consumer compilation are later stages.
/// The supplied base must be the exact include-merged configuration associated
/// with `authored`; a mismatched catalogue fails rather than inventing origins.
///
/// # Errors
/// Returns a validation error for invalid input, inconsistent authored metadata
/// or more than 256 surviving rules at any existing layer boundary.
pub fn select_site_policy_rules<'a>(
    base: &'a Config,
    authored: &ConfigurationSources,
    generated: &'a RuntimeOverrides,
    temporary: &'a RuntimeOverrides,
    revisions: SitePolicySourceRevisions,
    url: &str,
) -> Result<SelectedSitePolicyRules<'a>, ConfigError> {
    Ok(resolve_site_policy_rules(base, authored, generated, temporary, revisions)?.select(url))
}

/// Validates all raw source rules and resolves replacement/order without a URL
/// or runtime authority. Shares the exact resolver used by URL selection.
///
/// # Errors
/// Rejects invalid raw input, inconsistent catalogues or oversized rule sets.
pub fn resolve_site_policy_rules<'a, 'catalogue>(
    base: &'a Config,
    authored: &'catalogue ConfigurationSources,
    generated: &'a RuntimeOverrides,
    temporary: &'a RuntimeOverrides,
    revisions: SitePolicySourceRevisions,
) -> Result<ResolvedSitePolicyRules<'a, 'catalogue>, ConfigError> {
    let base_source = authored_rule_source(base, authored)?;
    validate_runtime_overrides(generated)?;
    validate_runtime_overrides(temporary)?;
    let layers = [
        (&base.site_rules, base_source, revisions.authored),
        (
            &generated.site_rules,
            PolicySourceIdentity::Generated,
            revisions.generated,
        ),
        (
            &temporary.site_rules,
            PolicySourceIdentity::Temporary,
            revisions.temporary,
        ),
    ];
    let mut surviving = BTreeMap::<&str, (usize, usize, SelectedSitePolicyRule<'a>)>::new();
    let mut shadowed = Vec::new();
    for (layer, (rules, source, revision)) in layers.into_iter().enumerate() {
        validate_rules(rules)?;
        for (index, rule) in rules.iter().enumerate() {
            let origin = Arc::new(
                PolicyFieldOrigin::selected(rule, source.clone(), revision, index)
                    .map_err(|message| ConfigError::validation(format_args!("{message}")))?,
            );
            let selected = SelectedSitePolicyRule { rule, origin };
            if let Some((_, _, previous)) =
                surviving.insert(&rule.id, (layer, index, selected.clone()))
            {
                shadowed.push(ShadowedSitePolicyRule {
                    replaced: previous.origin,
                    replacement: selected.origin,
                });
            }
        }
        if surviving.len() > MAX_SITE_RULES {
            return Err(invalid("at most 256 surviving site rules are supported"));
        }
    }
    // Same-ID replacement has finished before the first URL test. Sort by
    // layer first: a lower rule's numeric priority cannot outrank a new source.
    let mut ordered = surviving.into_values().collect::<Vec<_>>();
    ordered.sort_by_key(|(layer, index, selected)| (*layer, selected.rule.priority, *index));
    Ok(ResolvedSitePolicyRules {
        rules: ordered.into_iter().map(|(_, _, rule)| rule).collect(),
        shadowed,
        authored,
        authored_revision: revisions.authored,
    })
}

fn authored_rule_source(
    base: &Config,
    authored: &ConfigurationSources,
) -> Result<PolicySourceIdentity, ConfigError> {
    let mut surviving = None;
    for (index, document) in authored.documents().iter().enumerate() {
        if let Some(rules) = document.site_rules() {
            validate_rules(rules)?;
            surviving = Some((index, rules));
        }
    }
    match surviving {
        Some((index, rules)) if rules == base.site_rules => Ok(PolicySourceIdentity::Authored {
            document_id: u32::try_from(index)
                .map_err(|_| invalid("authored source catalogue is too large"))?,
        }),
        None if authored.documents().is_empty() => Ok(PolicySourceIdentity::InMemory),
        None if base.site_rules.is_empty() => Ok(PolicySourceIdentity::InMemory),
        _ => Err(invalid(
            "authored site-rule catalogue does not match the merged base",
        )),
    }
}

fn validate_rules(rules: &[SiteRule]) -> Result<(), ConfigError> {
    if rules.len() > MAX_SITE_RULES {
        return Err(invalid("at most 256 site rules are supported per source"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for rule in rules {
        validate_site_rule(rule)?;
        if !ids.insert(&rule.id) {
            return Err(invalid("site rule IDs must be unique within a source"));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> ConfigError {
    ConfigError::validation(format_args!("{message}"))
}

#[cfg(test)]
mod tests;
