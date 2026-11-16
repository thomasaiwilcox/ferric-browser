//! Authored shape and selector checks. Canonical keys, codebooks and effective
//! combinations remain separate required stages; this is not publication authority.
use crate::{
    InteractionCollectionOrder, InteractionFamily, InteractionPolicyPatch, InteractionPredicates,
    InteractionRootStepKind, InteractionSelector, MAX_INTERACTION_ANCHORS,
    MAX_INTERACTION_ATTRIBUTE_TESTS, MAX_INTERACTION_AUTHORED_BYTES,
    MAX_INTERACTION_AUTHORED_SELECTOR_BYTES, MAX_INTERACTION_COLLECTION_CODES,
    MAX_INTERACTION_COLLECTIONS, MAX_INTERACTION_HINT_PAGE_KEYS, MAX_INTERACTION_KEY_BYTES,
    MAX_INTERACTION_REMAPS, MAX_INTERACTION_RESERVATIONS, validate_interaction_attribute_name,
    validate_interaction_code, validate_interaction_entry_id,
    validate_interaction_primary_root_steps,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
enum ValidationScope {
    Authored,
    Effective,
}

impl ValidationScope {
    fn validate_entry_counts(self, patch: &InteractionPolicyPatch) -> Result<(), String> {
        for (field, count, limit) in [
            (
                "anchors",
                patch.anchors.as_ref().map_or(0, Vec::len),
                MAX_INTERACTION_ANCHORS,
            ),
            (
                "collections",
                patch.collections.as_ref().map_or(0, Vec::len),
                MAX_INTERACTION_COLLECTIONS,
            ),
            (
                "remaps",
                patch.remaps.as_ref().map_or(0, Vec::len),
                MAX_INTERACTION_REMAPS,
            ),
            (
                "hint_page_keys",
                patch.hint_page_keys.as_ref().map_or(0, Vec::len),
                MAX_INTERACTION_HINT_PAGE_KEYS,
            ),
        ] {
            if count > limit {
                let scope = match self {
                    ValidationScope::Authored => "authored",
                    ValidationScope::Effective => "effective",
                };
                return Err(format!(
                    "{scope} Interaction.{field} has {count} entries; limit {limit}"
                ));
            }
        }
        Ok(())
    }

    const fn selector_budget(self) -> (usize, &'static str) {
        match self {
            Self::Authored => (
                MAX_INTERACTION_AUTHORED_SELECTOR_BYTES,
                "authored selectors exceed 16 KiB",
            ),
            Self::Effective => (
                crate::MAX_INTERACTION_EFFECTIVE_SELECTOR_BYTES,
                "effective selectors exceed 32 KiB",
            ),
        }
    }

    fn validate_serialized(self, patch: &InteractionPolicyPatch) -> Result<(), String> {
        let (limit, message) = match self {
            Self::Authored => (
                MAX_INTERACTION_AUTHORED_BYTES,
                "serialized authored Interaction policy exceeds 64 KiB",
            ),
            Self::Effective => (
                crate::MAX_INTERACTION_EFFECTIVE_BYTES,
                "serialized effective Interaction policy exceeds 128 KiB",
            ),
        };
        if toml::to_string(patch)
            .map_err(|_| "could not serialize Interaction policy")?
            .len()
            > limit
        {
            return Err(message.into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticInteractionValidation {
    pub selectors: Vec<InteractionSelector>,
    pub keys: crate::CompiledInteractionKeys,
}

impl InteractionPolicyPatch {
    /// Validates an authored section. Engine CSS validation, effective label
    /// compilation, native qualification and live proof remain separate stages.
    pub fn validate_authored(&self) -> Result<StaticInteractionValidation, String> {
        self.validate_for_scope(ValidationScope::Authored)
    }

    pub(crate) fn validate_effective(&self) -> Result<StaticInteractionValidation, String> {
        self.validate_for_scope(ValidationScope::Effective)
    }

    fn validate_for_scope(
        &self,
        scope: ValidationScope,
    ) -> Result<StaticInteractionValidation, String> {
        let selectors = self.validate_shape_for_scope(scope)?;
        let keys = self.compile_authored_keys()?;
        let mut excluded = keys.hint_label_source_characters();
        if let Some(characters) = self
            .labels
            .as_ref()
            .and_then(|labels| labels.excluded_chars.as_ref())
        {
            excluded.extend(characters.iter().filter_map(|text| text.chars().next()));
        }
        let mut namespaces = std::collections::BTreeMap::<InteractionFamily, BTreeSet<&str>>::new();
        let anchor_codes = self
            .anchors
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|anchor| (anchor.families.as_slice(), anchor.code.as_str()));
        let collection_codes = self
            .collections
            .as_deref()
            .unwrap_or_default()
            .iter()
            .flat_map(|collection| {
                collection
                    .codes
                    .iter()
                    .map(move |code| (collection.families.as_slice(), code.as_str()))
            });
        for (families, code) in anchor_codes.chain(collection_codes) {
            if code.chars().any(|character| excluded.contains(&character)) {
                return Err(
                    "special code contains an excluded or Hint page-owned source character".into(),
                );
            }
            for family in families {
                // Codes are bounded ASCII; lexicographic neighbors suffice to
                // detect both prefix directions without comparing every pair.
                let codes = namespaces.entry(*family).or_default();
                if !codes.insert(code) {
                    return Err("special codes duplicate within an invocation family".into());
                }
            }
        }
        for codes in namespaces.values() {
            let mut previous: Option<&str> = None;
            for code in codes {
                if previous.is_some_and(|prefix| code.starts_with(prefix)) {
                    return Err(
                        "special codes have a prefix conflict within an invocation family".into(),
                    );
                }
                previous = Some(code);
            }
        }
        if self
            .labels
            .as_ref()
            .and_then(|labels| labels.fallback_prefix.as_ref())
            .is_some_and(|prefix| {
                prefix
                    .chars()
                    .any(|character| excluded.contains(&character))
            })
        {
            return Err(
                "fallback prefix contains an excluded or Hint page-owned source character".into(),
            );
        }
        Ok(StaticInteractionValidation { selectors, keys })
    }
    /// Checks every authored entry even when disabled. Returns selector ASTs
    /// in authored field order, including root/per-item/slot selectors.
    /// Canonical key grammar and effective code collisions are checked later.
    pub fn validate_shape_and_selectors(&self) -> Result<Vec<InteractionSelector>, String> {
        self.validate_shape_for_scope(ValidationScope::Authored)
    }

    fn validate_shape_for_scope(
        &self,
        scope: ValidationScope,
    ) -> Result<Vec<InteractionSelector>, String> {
        let anchors = self.anchors.as_deref().unwrap_or_default();
        let collections = self.collections.as_deref().unwrap_or_default();
        let remaps = self.remaps.as_deref().unwrap_or_default();
        let keys = self.hint_page_keys.as_deref().unwrap_or_default();
        scope.validate_entry_counts(self)?;
        ids(anchors.iter().map(|entry| entry.id.as_str()))?;
        ids(collections.iter().map(|entry| entry.id.as_str()))?;
        ids(remaps.iter().map(|entry| entry.id.as_str()))?;
        if let Some(labels) = &self.labels {
            labels.validate_leaves()?;
        }
        if let Some(receiver) = &self.page_receiver {
            receiver.validate()?;
        }
        let mut selectors = Vec::new();
        let mut bytes = 0_usize;
        let (limit, message) = scope.selector_budget();
        let mut add = |source: &str| {
            bytes = bytes
                .checked_add(source.len())
                .ok_or("selector byte count overflow")?;
            if bytes > limit {
                return Err(message.into());
            }
            selectors.push(InteractionSelector::parse(source).map_err(|error| error.to_string())?);
            Ok(())
        };
        for anchor in anchors {
            families(&anchor.families)?;
            validate_interaction_code(&anchor.code)?;
            if anchor.target_selector.is_none()
                && anchor
                    .predicates
                    .as_ref()
                    .is_none_or(InteractionPredicates::is_empty)
            {
                return Err("anchor requires a target selector or predicate".into());
            }
            if let Some(root) = &anchor.root {
                root.validate(0, &mut add)?;
            }
            if let Some(selector) = &anchor.scope_selector {
                add(selector)?;
            }
            if let Some(selector) = &anchor.target_selector {
                add(selector)?;
            }
            if let Some(predicates) = &anchor.predicates {
                predicates.validate()?;
            }
        }
        let mut reservations = anchors.len();
        for collection in collections {
            validate_collection(collection, &mut add)?;
            reservations += collection.codes.len();
            if reservations > MAX_INTERACTION_RESERVATIONS {
                return Err("policy exceeds 512 special reservations".into());
            }
        }
        let mut seen_keys = BTreeSet::new();
        for key in keys {
            key_bound(key)?;
            if !seen_keys.insert(key) {
                return Err("Hint page-key sources must be unique".into());
            }
        }
        let mut source_contexts = BTreeSet::new();
        for remap in remaps {
            key_bound(&remap.from)?;
            key_bound(&remap.to)?;
            if remap.contexts.is_empty()
                || remap.contexts.len() > crate::MAX_INTERACTION_REMAP_CONTEXTS
            {
                return Err("remap requires explicit unique contexts".into());
            }
            let mut seen = BTreeSet::new();
            for context in &remap.contexts {
                if !seen.insert(context) || !source_contexts.insert((&remap.from, context)) {
                    return Err("remap source/context overlaps an assignment".into());
                }
            }
        }
        scope.validate_serialized(self)?;
        Ok(selectors)
    }
}

fn validate_collection(
    collection: &crate::InteractionCollection,
    mut add: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    families(&collection.families)?;
    if collection.codes.is_empty() || collection.codes.len() > MAX_INTERACTION_COLLECTION_CODES {
        return Err("collection requires 1..=64 codes".into());
    }
    for code in &collection.codes {
        validate_interaction_code(code)?;
    }
    if let Some(root) = &collection.root {
        root.validate(0, &mut add)?;
    }
    selector_choice(
        collection.container_selector.as_deref(),
        collection.container_self,
        &mut add,
    )?;
    add(&collection.item_selector)?;
    let steps = collection.primary_root_steps.as_deref().unwrap_or_default();
    let depth = collection.root.as_ref().map_or(0, |root| root.steps.len());
    validate_interaction_primary_root_steps(steps, depth, &mut add)?;
    selector_choice(
        collection.primary_selector.as_deref(),
        collection.primary_self,
        &mut add,
    )?;
    if collection.primary_self == Some(true)
        && steps
            .last()
            .is_some_and(|step| step.kind != InteractionRootStepKind::AssignedSlot)
    {
        return Err("primary_self requires an Element, not Document or ShadowRoot".into());
    }
    if let Some(attributes) = &collection.item_attributes {
        if attributes.len() > MAX_INTERACTION_ATTRIBUTE_TESTS {
            return Err("item supports at most four attribute tests".into());
        }
        for attribute in attributes {
            attribute.validate()?;
        }
    }
    if let Some(predicates) = &collection.primary_predicates {
        predicates.validate()?;
    }
    if let Some(attributes) = &collection.identity_attributes {
        if attributes.len() > crate::MAX_INTERACTION_IDENTITY_ATTRIBUTES {
            return Err("collection supports at most four identity attributes".into());
        }
        let mut seen = BTreeSet::new();
        for attribute in attributes {
            validate_interaction_attribute_name(attribute)?;
            if !seen.insert(attribute) {
                return Err("identity attributes must be unique".into());
            }
        }
    }
    if let Some(tolerance) = collection.row_tolerance_px
        && (!matches!(
            collection.order,
            InteractionCollectionOrder::RowMajorLtr | InteractionCollectionOrder::RowMajorRtl
        ) || !tolerance.is_finite()
            || !(0.0..=32.0).contains(&tolerance))
    {
        return Err("row tolerance must be finite in [0,32] for row-major order".into());
    }
    Ok(())
}

fn key_bound(key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > MAX_INTERACTION_KEY_BYTES {
        return Err("key token requires 1..=64 UTF-8 bytes".into());
    }
    Ok(())
}
fn ids<'a>(ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        validate_interaction_entry_id(id)?;
        if !seen.insert(id) {
            return Err("IDs must be unique within each Interaction category".into());
        }
    }
    Ok(())
}
fn families(families: &[InteractionFamily]) -> Result<(), String> {
    if families.is_empty()
        || families.len() > crate::MAX_INTERACTION_FAMILIES
        || families.iter().collect::<BTreeSet<_>>().len() != families.len()
    {
        return Err("families must be nonempty and unique invocation families".into());
    }
    Ok(())
}
fn selector_choice(
    selector: Option<&str>,
    own: Option<bool>,
    validate: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    match (selector, own) {
        (Some(selector), None) => validate(selector),
        (None, Some(true)) => Ok(()),
        _ => Err("choose exactly one explicit selector or self=true".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_entry_error_uses_effective_scope_and_exact_field() {
        let patch = InteractionPolicyPatch {
            hint_page_keys: Some(vec!["k".into(); 33]),
            ..Default::default()
        };
        let error = patch.validate_effective().unwrap_err();
        assert_eq!(
            error,
            "effective Interaction.hint_page_keys has 33 entries; limit 32"
        );
    }
    fn example() -> InteractionPolicyPatch {
        let raw: toml::Value = toml::from_str(include_str!(
            "../tests/fixtures/site-policy/frame-shadow-grid.toml"
        ))
        .unwrap();
        raw["site_rules"][0]["interaction"]
            .clone()
            .try_into()
            .unwrap()
    }
    #[test]
    fn disabled_rules_and_root_selectors_are_validated() {
        let mut patch = example();
        patch.enabled = Some(false);
        assert_eq!(patch.validate_shape_and_selectors().unwrap().len(), 5);
        patch.collections.as_mut().unwrap()[0]
            .root
            .as_mut()
            .unwrap()
            .steps[1]
            .host_selector = Some(":has(a)".into());
        assert!(patch.validate_shape_and_selectors().is_err());
    }
    #[test]
    fn collection_exclusivity_primary_type_and_codes_are_checked() {
        let mut patch = example();
        patch.collections.as_mut().unwrap()[0].container_self = Some(true);
        assert!(patch.validate_shape_and_selectors().is_err());
        let mut patch = example();
        let collection = &mut patch.collections.as_mut().unwrap()[0];
        collection.primary_selector = None;
        collection.primary_self = Some(true);
        assert!(patch.validate_shape_and_selectors().is_err());
        let mut patch = example();
        patch.collections.as_mut().unwrap()[0].codes.clear();
        assert!(patch.validate_shape_and_selectors().is_err());
    }
    #[test]
    fn remap_contexts_are_explicit_but_reciprocal_mappings_are_valid_shape() {
        let mut patch: InteractionPolicyPatch = toml::from_str("[[remaps]]\nid='a'\nfrom='p'\nto='k'\ncontexts=['passthrough']\n[[remaps]]\nid='b'\nfrom='k'\nto='p'\ncontexts=['passthrough']").unwrap();
        patch.validate_shape_and_selectors().unwrap();
        patch.remaps.as_mut().unwrap()[1].from = "p".into();
        assert!(patch.validate_shape_and_selectors().is_err());
    }

    #[test]
    fn authored_selector_and_serialization_caps_have_exact_boundaries() {
        let mut patch: InteractionPolicyPatch = toml::from_str(
            "[[anchors]]\nid='a'\ncode='a'\nfamilies=['links']\ntarget_selector='a'",
        )
        .unwrap();
        let mut anchor = patch.anchors.as_ref().unwrap()[0].clone();
        anchor.target_selector = Some(format!(".{}", "a".repeat(1023)));
        patch.anchors = Some(
            (0..16)
                .map(|i| {
                    let mut a = anchor.clone();
                    a.id = format!("a{i}");
                    a
                })
                .collect(),
        );
        assert_eq!(patch.validate_shape_and_selectors().unwrap().len(), 16);
        anchor.id = "last".into();
        patch.anchors.as_mut().unwrap().push(anchor);
        assert!(patch.validate_shape_and_selectors().is_err());

        let mut patch: InteractionPolicyPatch = toml::from_str("[[anchors]]\nid='a'\ncode='a'\nfamilies=['links']\n[anchors.predicates.href]\npath='/'").unwrap();
        let initial = toml::to_string(&patch).unwrap().len();
        let href_value = patch.anchors.as_mut().unwrap()[0]
            .predicates
            .as_mut()
            .unwrap()
            .href
            .as_mut()
            .unwrap()
            .path
            .as_mut()
            .unwrap();
        href_value.push_str(&"a".repeat(MAX_INTERACTION_AUTHORED_BYTES - initial));
        assert_eq!(
            toml::to_string(&patch).unwrap().len(),
            MAX_INTERACTION_AUTHORED_BYTES
        );
        patch.validate_shape_and_selectors().unwrap();
        patch.anchors.as_mut().unwrap()[0]
            .predicates
            .as_mut()
            .unwrap()
            .href
            .as_mut()
            .unwrap()
            .path
            .as_mut()
            .unwrap()
            .push('a');
        assert!(patch.validate_shape_and_selectors().is_err());
    }

    #[test]
    fn special_namespaces_are_family_local_and_source_characters_are_not_destinations() {
        let raw = "hint_page_keys=['p']\n[[anchors]]\nid='a'\ncode='k'\nfamilies=['links']\ntarget_selector='a'\n[[anchors]]\nid='b'\ncode='k'\nfamilies=['buttons']\ntarget_selector='button'\n[[remaps]]\nid='play'\nfrom='p'\nto='k'\ncontexts=['hint-page-key']";
        let mut patch: InteractionPolicyPatch = toml::from_str(raw).unwrap();
        patch.validate_authored().unwrap();
        patch.anchors.as_mut().unwrap()[1].families = vec![InteractionFamily::Links];
        assert!(patch.validate_authored().is_err());
        patch.anchors.as_mut().unwrap()[1].code = "ka".into();
        assert!(patch.validate_authored().is_err());
        patch.anchors.as_mut().unwrap()[1].code = "ap".into();
        assert!(patch.validate_authored().is_err());
        patch.anchors.as_mut().unwrap()[1].code = "z".into();
        patch.hint_page_keys = Some(vec!["Ctrl+k".into()]);
        patch.validate_authored().unwrap();
    }
}
