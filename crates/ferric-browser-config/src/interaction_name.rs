//! Compiled name predicates, independent of DOM discovery or input routing.
//! This bounded name is not the complete accessibility-name algorithm. A
//! renderer must separately prove which candidate/root produced the evidence.

use crate::{InteractionNameMatch, InteractionTextCase};
use icu_normalizer::ComposingNormalizerBorrowed;
use std::fmt;

// Match the existing hint candidate decoder's text limits. Bound input before
// normalization: canonical normalization can expand UTF-8, which is retained
// exactly rather than truncated into a different match.
pub const MAX_INTERACTION_NAME_EVIDENCE_BYTES: usize = 2_048;
pub const MAX_INTERACTION_NAME_EVIDENCE_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionNameEvidenceError {
    Missing,
    Truncated,
    TooManyBytes,
    TooManyCharacters,
}

impl fmt::Display for InteractionNameEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Missing => "requested hint name evidence is missing",
            Self::Truncated => "requested hint name evidence is truncated",
            Self::TooManyBytes => "hint name evidence exceeds 2048 UTF-8 bytes",
            Self::TooManyCharacters => "hint name evidence exceeds 512 characters",
        })
    }
}

impl std::error::Error for InteractionNameEvidenceError {}

/// Complete empty names are distinct from missing or truncated observations.
/// Construction asserts only text quality/size, never discovery or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionNameEvidence<'a> {
    text: Result<&'a str, InteractionNameEvidenceError>,
}

impl<'a> InteractionNameEvidence<'a> {
    /// The caller must establish completeness independently. Being within the
    /// byte/character limits does not prove that extraction was complete; the
    /// legacy renderer text silently truncates and is insufficient on its own.
    pub fn complete(text: &'a str) -> Result<Self, InteractionNameEvidenceError> {
        if text.len() > MAX_INTERACTION_NAME_EVIDENCE_BYTES {
            return Err(InteractionNameEvidenceError::TooManyBytes);
        }
        if text.chars().count() > MAX_INTERACTION_NAME_EVIDENCE_CHARS {
            return Err(InteractionNameEvidenceError::TooManyCharacters);
        }
        Ok(Self { text: Ok(text) })
    }

    #[must_use]
    pub const fn missing() -> Self {
        Self {
            text: Err(InteractionNameEvidenceError::Missing),
        }
    }

    #[must_use]
    pub const fn truncated() -> Self {
        Self {
            text: Err(InteractionNameEvidenceError::Truncated),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionNameOperator {
    Equals,
    Contains,
}

/// Immutable compiled operand. No deserializer can bypass authored validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledInteractionNameMatch {
    operator: InteractionNameOperator,
    value: String,
    case: InteractionTextCase,
}

impl TryFrom<&InteractionNameMatch> for CompiledInteractionNameMatch {
    type Error = String;

    fn try_from(authored: &InteractionNameMatch) -> Result<Self, Self::Error> {
        authored.validate()?;
        let (operator, value) = match (&authored.equals, &authored.contains) {
            (Some(value), None) => (InteractionNameOperator::Equals, value),
            (None, Some(value)) => (InteractionNameOperator::Contains, value),
            _ => return Err("hint_name requires exactly one equals or contains".into()),
        };
        let case = authored.case.unwrap_or(InteractionTextCase::Sensitive);
        Ok(Self {
            operator,
            value: normalized(value, case),
            case,
        })
    }
}

impl CompiledInteractionNameMatch {
    #[must_use]
    pub const fn operator(&self) -> InteractionNameOperator {
        self.operator
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn case(&self) -> InteractionTextCase {
        self.case
    }

    /// Unavailable evidence is not a negative match or a proof of absence.
    /// Normalize outside the synchronous key handler, at bounded proof time.
    pub fn matches(
        &self,
        evidence: InteractionNameEvidence<'_>,
    ) -> Result<bool, InteractionNameEvidenceError> {
        let value = normalized(evidence.text?, self.case);
        Ok(match self.operator {
            InteractionNameOperator::Equals => value == self.value,
            InteractionNameOperator::Contains => value.contains(&self.value),
        })
    }

    /// Shared-pass matching for the policy compiler. Normalization has bounded
    /// input; comparisons and contains retries charge the caller's same budget.
    pub fn matches_with_work(
        &self,
        evidence: InteractionNameEvidence<'_>,
        work: &mut crate::InteractionPredicateWork,
    ) -> Result<bool, crate::InteractionPredicateEvidenceError> {
        use crate::interaction_predicate_match::compare_bytes;
        work.charge(1)?;
        let text = evidence
            .text
            .map_err(crate::InteractionPredicateEvidenceError::Name)?;
        work.charge(text.len())?;
        let value = normalized(text, self.case);
        work.charge(value.len())?;
        match self.operator {
            InteractionNameOperator::Equals => compare_bytes(&self.value, &value, false, work),
            InteractionNameOperator::Contains => {
                if self.value.is_empty() {
                    return Ok(true);
                }
                for (index, _) in value.char_indices() {
                    work.charge(1)?;
                    if compare_bytes(&self.value, &value[index..], true, work)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

fn normalized(text: &str, case: InteractionTextCase) -> String {
    let canonical = ComposingNormalizerBorrowed::new_nfc().normalize(text);
    let mut result = String::with_capacity(canonical.len());
    // Unicode White_Space, rather than JavaScript's different \s set. Normalize
    // both authored values and independently complete evidence here in Rust.
    for word in canonical.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(word);
    }
    if case == InteractionTextCase::AsciiInsensitive {
        result.make_ascii_lowercase();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(
        equals: Option<&str>,
        contains: Option<&str>,
        case: Option<InteractionTextCase>,
    ) -> CompiledInteractionNameMatch {
        CompiledInteractionNameMatch::try_from(&InteractionNameMatch {
            equals: equals.map(str::to_owned),
            contains: contains.map(str::to_owned),
            case,
        })
        .unwrap()
    }

    fn matches(rule: &CompiledInteractionNameMatch, text: &str) -> bool {
        rule.matches(InteractionNameEvidence::complete(text).unwrap())
            .unwrap()
    }

    #[test]
    fn canonical_equivalence_and_whitespace_apply_to_both_operands() {
        let rule = compile(Some(" \tCafe\u{301}\u{a0} Mac\n"), None, None);
        assert_eq!(rule.value(), "Café Mac");
        assert!(matches(&rule, "\u{2003}Café\u{85}\tMac "));
        assert!(matches(
            &compile(Some("가"), None, None),
            "\u{1100}\u{1161}"
        ));
        let contains = compile(None, Some("ä T"), None);
        assert!(matches(&contains, "A a\u{308}\tTarget"));
        assert!(!matches(&contains, "A a\u{308}Target"));
        assert!(!matches(&contains, "A ä\u{200b}Target"));
    }

    #[test]
    fn default_case_ascii_only_folding_and_no_compatibility_normalization() {
        let sensitive = compile(Some("Mac"), None, None);
        assert_eq!(sensitive.case(), InteractionTextCase::Sensitive);
        assert!(!matches(&sensitive, "mac"));
        let insensitive = compile(
            Some("ÄBC"),
            None,
            Some(InteractionTextCase::AsciiInsensitive),
        );
        assert_eq!(insensitive.value(), "Äbc");
        assert!(matches(&insensitive, "ÄbC"));
        assert!(!matches(&insensitive, "äbc"));
        let contains = compile(
            None,
            Some("aB"),
            Some(InteractionTextCase::AsciiInsensitive),
        );
        assert!(matches(&contains, "cABd"));
        assert!(!matches(&contains, "a b"));
        assert!(!matches(&sensitive, "Ｍａｃ"));
        assert!(!matches(&sensitive, "\u{feff}Mac"));
    }

    #[test]
    fn unavailable_is_not_empty_or_a_negative_match() {
        for rule in [compile(Some(""), None, None), compile(None, Some(""), None)] {
            assert!(matches(&rule, ""));
            assert_eq!(
                rule.matches(InteractionNameEvidence::missing()),
                Err(InteractionNameEvidenceError::Missing)
            );
            assert_eq!(
                rule.matches(InteractionNameEvidence::truncated()),
                Err(InteractionNameEvidenceError::Truncated)
            );
        }
    }

    #[test]
    fn evidence_limits_check_decoded_input_without_truncation() {
        for count in [511, 512] {
            assert!(InteractionNameEvidence::complete(&"a".repeat(count)).is_ok());
            assert!(InteractionNameEvidence::complete(&"🦀".repeat(count)).is_ok());
        }
        assert_eq!(
            InteractionNameEvidence::complete(&"a".repeat(513)),
            Err(InteractionNameEvidenceError::TooManyCharacters)
        );
        assert_eq!(
            InteractionNameEvidence::complete(&"🦀".repeat(513)),
            Err(InteractionNameEvidenceError::TooManyBytes)
        );
    }

    #[test]
    fn authored_limits_are_before_normalization_and_expansions_are_preserved() {
        let authored = "\u{344}".repeat(128);
        assert_eq!(authored.len(), 256);
        let rule = compile(Some(&authored), None, None);
        let expanded = "\u{308}\u{301}".repeat(128);
        assert_eq!(rule.value(), expanded);
        assert!(matches(&rule, &expanded));
        for value in ["a".repeat(257), "é".repeat(129)] {
            let authored = InteractionNameMatch {
                equals: Some(value),
                ..Default::default()
            };
            assert!(CompiledInteractionNameMatch::try_from(&authored).is_err());
        }
        for authored in [
            InteractionNameMatch::default(),
            InteractionNameMatch {
                equals: Some("a".into()),
                contains: Some("a".into()),
                case: None,
            },
        ] {
            assert!(CompiledInteractionNameMatch::try_from(&authored).is_err());
        }
    }
}
