//! Invocation projection over the existing checked effective policy. No rule
//! matching, input admission, DOM discovery or publication happens here.
use crate::{EffectiveInteractionPolicy, InteractionAutoFollow, InteractionFamily};
use ferric_browser_core::{
    CheckedHintCodebook, HintAutoFollow, HintCodeDeclaration, HintCodebookError,
    HintCodebookOptions, HintReservationCategory,
};
use std::collections::BTreeSet;

/// Keeps the literal invocation and checked policy/provenance associated with
/// its codebook. This is a label namespace, never a live target/action permit.
#[derive(Clone, Debug)]
pub struct CompiledInvocationHintCodebook {
    family: InteractionFamily,
    policy: EffectiveInteractionPolicy,
    codebook: CheckedHintCodebook,
}

impl CompiledInvocationHintCodebook {
    #[must_use]
    pub const fn family(&self) -> InteractionFamily {
        self.family
    }
    #[must_use]
    pub const fn policy(&self) -> &EffectiveInteractionPolicy {
        &self.policy
    }
    #[must_use]
    pub const fn codebook(&self) -> &CheckedHintCodebook {
        &self.codebook
    }
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        InteractionFamily,
        EffectiveInteractionPolicy,
        CheckedHintCodebook,
    ) {
        (self.family, self.policy, self.codebook)
    }
}

impl EffectiveInteractionPolicy {
    /// Projects all declared slots for one literal invocation before discovery.
    /// The effective snapshot already passed complete per-family validation;
    /// selection cannot hide conflicts in another invocation or another root.
    /// `None` preserves the existing ordinary path for disabled/no-op policy.
    ///
    /// # Errors
    /// Rejects enhanced label conflicts, exclusions and unusable alphabets.
    /// Initial ordinary capacity is checked later, after special ownership.
    pub fn compile_hint_codebook(
        &self,
        family: InteractionFamily,
    ) -> Result<Option<CompiledInvocationHintCodebook>, HintCodebookError> {
        if !self.enhanced_requested_for(family) {
            return Ok(None);
        }
        let labels = self.labels();
        let excluded_chars = labels
            .excluded_chars()
            .iter()
            .map(|text| {
                let mut characters = text.chars();
                match (characters.next(), characters.next()) {
                    (Some(character), None) => Ok(character),
                    _ => Err(HintCodebookError::InvalidExclusion),
                }
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let options = HintCodebookOptions {
            alphabet: labels.alphabet().into(),
            min_chars: labels.min_chars(),
            fallback_prefix: labels.fallback_prefix().into(),
            excluded_chars,
            page_source_chars: self.keys().hint_label_source_characters(),
            additional_controls: BTreeSet::new(),
            auto_follow: match labels.auto_follow() {
                InteractionAutoFollow::Always => HintAutoFollow::Always,
                InteractionAutoFollow::UniqueMatch => HintAutoFollow::UniqueMatch,
                InteractionAutoFollow::FullMatch => HintAutoFollow::FullMatch,
                InteractionAutoFollow::Never => HintAutoFollow::Never,
            },
        };
        let mut declarations = Vec::new();
        for anchor in self
            .anchors()
            .iter()
            .filter(|entry| entry.families.contains(&family))
        {
            declarations.push(HintCodeDeclaration {
                category: HintReservationCategory::Anchor,
                rule_id: anchor.id.clone(),
                slot: 0,
                code: anchor.code.clone(),
            });
        }
        for collection in self
            .collections()
            .iter()
            .filter(|entry| entry.families.contains(&family))
        {
            declarations.extend(collection.codes.iter().enumerate().map(|(slot, code)| {
                HintCodeDeclaration {
                    category: HintReservationCategory::Collection,
                    rule_id: collection.id.clone(),
                    slot,
                    code: code.clone(),
                }
            }));
        }
        Ok(Some(CompiledInvocationHintCodebook {
            family,
            policy: self.clone(),
            codebook: CheckedHintCodebook::compile(options, declarations)?,
        }))
    }
}
