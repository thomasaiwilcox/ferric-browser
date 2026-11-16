//! Canonical authored key compilation. No native delivery or page ownership is
//! granted by parsing a gesture; routing must admit its source first.
use crate::interaction_capabilities::{
    NativeReceiverScopeRequirement, NativeRemapCapabilityRequirement,
};
use crate::{
    InteractionPolicyPatch, InteractionRemapContext, MAX_INTERACTION_HINT_PAGE_KEYS,
    MAX_INTERACTION_REMAPS, validate_interaction_entry_id,
};
use ferric_browser_core::LogicalKeyToken;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledInteractionRemap {
    id: String,
    requirement: NativeRemapCapabilityRequirement,
}

impl CompiledInteractionRemap {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn destination(&self) -> &LogicalKeyToken {
        self.requirement.destination()
    }
    #[must_use]
    pub const fn capability_requirement(&self) -> &NativeRemapCapabilityRequirement {
        &self.requirement
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompiledInteractionKeys {
    hint_page_keys: BTreeSet<LogicalKeyToken>,
    remaps: BTreeMap<InteractionRemapContext, BTreeMap<LogicalKeyToken, CompiledInteractionRemap>>,
}

impl CompiledInteractionKeys {
    #[must_use]
    pub fn hint_page_keys(&self) -> &BTreeSet<LogicalKeyToken> {
        &self.hint_page_keys
    }
    /// A terminal one-hop lookup: the destination is never looked up again.
    #[must_use]
    pub fn remap(
        &self,
        context: InteractionRemapContext,
        source: &LogicalKeyToken,
    ) -> Option<&CompiledInteractionRemap> {
        self.remaps.get(&context)?.get(source)
    }
    #[must_use]
    pub fn hint_label_source_characters(&self) -> BTreeSet<char> {
        self.hint_page_keys
            .iter()
            .filter_map(LogicalKeyToken::character)
            .collect()
    }
    /// Declarations only. A native capability manifest supplies qualification
    /// results for the current build/backend/layout and receiver route.
    pub fn capability_requirements(
        &self,
    ) -> impl Iterator<Item = &NativeRemapCapabilityRequirement> {
        self.remaps.values().flat_map(|entries| {
            entries
                .values()
                .map(CompiledInteractionRemap::capability_requirement)
        })
    }
}

impl InteractionPolicyPatch {
    /// Compiles key syntax and per-context conflicts, including disabled data.
    /// Receiver safety, source admission and backend qualification remain live
    /// browser responsibilities. Receiver descriptors are statically validated;
    /// other authored fields need their own checks.
    pub fn compile_authored_keys(&self) -> Result<CompiledInteractionKeys, String> {
        let hint_keys = self.hint_page_keys.as_deref().unwrap_or_default();
        let remaps = self.remaps.as_deref().unwrap_or_default();
        if hint_keys.len() > MAX_INTERACTION_HINT_PAGE_KEYS || remaps.len() > MAX_INTERACTION_REMAPS
        {
            return Err("Interaction key list exceeds its bound".into());
        }
        let receiver = NativeReceiverScopeRequirement::compile(self.page_receiver.as_ref())?;
        let mut compiled = CompiledInteractionKeys::default();
        for source in hint_keys {
            let key = parse_key(source)?;
            if key.is_protected_rescue() || key.is_hint_grammar_control() {
                return Err(
                    "Hint page source is owned by Hint grammar or protected recovery".into(),
                );
            }
            if !compiled.hint_page_keys.insert(key) {
                return Err("Hint page-key sources duplicate after canonicalization".into());
            }
        }
        let mut ids = BTreeSet::new();
        for remap in remaps {
            validate_interaction_entry_id(&remap.id)?;
            if !ids.insert(&remap.id) {
                return Err("remap IDs must be unique".into());
            }
            let source = parse_key(&remap.from)?;
            let destination = parse_key(&remap.to)?;
            if remap.contexts.is_empty()
                || remap.contexts.len() > crate::MAX_INTERACTION_REMAP_CONTEXTS
            {
                return Err("remap requires explicit unique contexts".into());
            }
            for context in &remap.contexts {
                let output = CompiledInteractionRemap {
                    id: remap.id.clone(),
                    requirement: NativeRemapCapabilityRequirement::compile(
                        &source,
                        &destination,
                        *context,
                        receiver,
                    )?,
                };
                if compiled
                    .remaps
                    .entry(*context)
                    .or_default()
                    .insert(source.clone(), output)
                    .is_some()
                {
                    return Err("remap source/context overlaps after canonicalization".into());
                }
            }
        }
        Ok(compiled)
    }
}

fn parse_key(source: &str) -> Result<LogicalKeyToken, String> {
    LogicalKeyToken::parse(source).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_and_controls_cannot_bypass_conflicts() {
        for sources in [["Ctrl+P", "Ctrl+Shift+p"], ["+", "Plus"]] {
            let patch = InteractionPolicyPatch {
                hint_page_keys: Some(sources.map(str::to_owned).into()),
                ..Default::default()
            };
            assert!(patch.compile_authored_keys().is_err());
        }
        for source in [
            "/",
            "Space",
            "Shift+Enter",
            "Ctrl+Tab",
            "Ctrl+U",
            "Shift+Ctrl+Escape",
        ] {
            let patch = InteractionPolicyPatch {
                hint_page_keys: Some(vec![source.into()]),
                ..Default::default()
            };
            assert!(patch.compile_authored_keys().is_err(), "{source}");
        }
    }
    #[test]
    fn only_admitted_unmodified_sources_reserve_labels_and_mapping_is_one_hop() {
        let patch: InteractionPolicyPatch = toml::from_str("hint_page_keys=['p','Ctrl+k']\n[[remaps]]\nid='play'\nfrom='p'\nto='k'\ncontexts=['hint-page-key','passthrough']\n[[remaps]]\nid='next'\nfrom='k'\nto='j'\ncontexts=['passthrough']").unwrap();
        let compiled = patch.compile_authored_keys().unwrap();
        assert_eq!(
            compiled.hint_label_source_characters(),
            BTreeSet::from(['p'])
        );
        let source = LogicalKeyToken::parse("p").unwrap();
        assert_eq!(
            compiled
                .remap(InteractionRemapContext::Passthrough, &source)
                .unwrap()
                .destination()
                .as_str(),
            "k"
        );
        assert!(
            compiled
                .remap(InteractionRemapContext::NormalPageKey, &source)
                .is_none()
        );
        assert!(
            !compiled
                .hint_page_keys()
                .contains(&LogicalKeyToken::parse("k").unwrap())
        );
    }
    #[test]
    fn sources_preserve_unicode_but_outputs_and_rescue_are_restricted() {
        for (source, output, valid) in [
            ("é", "k", true),
            ("p", "é", false),
            ("p", "Shift+Ctrl+Escape", false),
            ("Ctrl+Escape", "k", false),
        ] {
            let raw = format!(
                "[[remaps]]\nid='x'\nfrom='{source}'\nto='{output}'\ncontexts=['passthrough']"
            );
            let patch: InteractionPolicyPatch = toml::from_str(&raw).unwrap();
            assert_eq!(patch.compile_authored_keys().is_ok(), valid);
        }
    }
}
