//! Pure predicate compilation and bounded matching over borrowed facts.
//! Completeness, candidate/root association and extraction are independently
//! proved at the renderer/browser boundary. These types grant no authority.
use crate::{
    CompiledInteractionNameMatch, InteractionAttributeTest, InteractionHrefMatch,
    InteractionNameEvidence, InteractionNameEvidenceError, InteractionNumericRange,
    InteractionPredicates,
};
use std::fmt;

pub const MAX_INTERACTION_PREDICATE_STEPS: usize = 100_000;
// Preserve the existing hint candidate href transport bound.
pub const MAX_INTERACTION_HREF_EVIDENCE_BYTES: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionPredicateEvidenceError {
    Missing,
    Truncated,
    Name(InteractionNameEvidenceError),
    HrefTooLong,
    InvalidHref,
    InvalidGeometry,
    WorkExhausted,
}

impl fmt::Display for InteractionPredicateEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Missing => "requested predicate evidence is missing",
            Self::Truncated => "requested predicate evidence is truncated",
            Self::Name(error) => return error.fmt(f),
            Self::HrefTooLong => "predicate href exceeds the existing 8192-byte transport bound",
            Self::InvalidHref => "predicate href evidence is invalid",
            Self::InvalidGeometry => "predicate geometry evidence is not finite",
            Self::WorkExhausted => "shared Interaction predicate work limit exceeded",
        })
    }
}

impl std::error::Error for InteractionPredicateEvidenceError {}

/// One instance belongs to the whole pass, shared across rules, roots and
/// items. A step is a predicate entry, inspected/comparison byte, URL input
/// byte, normalized-name byte, or glob state transition. This bounds matching
/// work, not wall-clock time or renderer extraction/UTF-8 decoding work.
#[derive(Debug)]
pub struct InteractionPredicateWork {
    remaining: usize,
}

impl Default for InteractionPredicateWork {
    fn default() -> Self {
        Self {
            remaining: MAX_INTERACTION_PREDICATE_STEPS,
        }
    }
}

impl InteractionPredicateWork {
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.remaining
    }

    /// Root/selector evaluators must charge this same instance rather than
    /// reset a budget per rule. Failed charges exhaust the remaining work.
    pub fn charge(&mut self, steps: usize) -> Result<(), InteractionPredicateEvidenceError> {
        if steps > self.remaining {
            self.remaining = 0;
            return Err(InteractionPredicateEvidenceError::WorkExhausted);
        }
        self.remaining -= steps;
        Ok(())
    }
}

/// Borrowing avoids an invented observed attribute/role size cap. Complete
/// values may be long; comparisons/scans still charge the shared budget.
/// The caller must prove completeness separately; a short value proves none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionAttributeEvidence<'a> {
    Missing,
    Truncated,
    CompleteAbsent,
    CompletePresent(&'a str),
}

impl<'a> InteractionAttributeEvidence<'a> {
    fn value(self) -> Result<Option<&'a str>, InteractionPredicateEvidenceError> {
        match self {
            Self::Missing => Err(InteractionPredicateEvidenceError::Missing),
            Self::Truncated => Err(InteractionPredicateEvidenceError::Truncated),
            Self::CompleteAbsent => Ok(None),
            Self::CompletePresent(value) => Ok(Some(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AttributeOperator {
    Equals(String),
    Prefix(String),
    Present,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledInteractionAttributeTest {
    name: String,
    operator: AttributeOperator,
}

impl TryFrom<&InteractionAttributeTest> for CompiledInteractionAttributeTest {
    type Error = String;

    fn try_from(value: &InteractionAttributeTest) -> Result<Self, Self::Error> {
        value.validate()?;
        let operator = match (&value.equals, &value.prefix, value.present) {
            (Some(text), None, None) => AttributeOperator::Equals(text.clone()),
            (None, Some(text), None) => AttributeOperator::Prefix(text.clone()),
            (None, None, Some(true)) => AttributeOperator::Present,
            _ => {
                return Err(
                    "attribute test requires exactly one equals, prefix or present=true".into(),
                );
            }
        };
        Ok(Self {
            name: value.name.clone(),
            operator,
        })
    }
}

impl CompiledInteractionAttributeTest {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn matches(
        &self,
        evidence: InteractionAttributeEvidence<'_>,
        work: &mut InteractionPredicateWork,
    ) -> Result<bool, InteractionPredicateEvidenceError> {
        work.charge(1)?;
        let Some(value) = evidence.value()? else {
            return Ok(false);
        };
        match &self.operator {
            AttributeOperator::Present => Ok(true),
            AttributeOperator::Equals(expected) => compare_bytes(expected, value, false, work),
            AttributeOperator::Prefix(expected) => compare_bytes(expected, value, true, work),
        }
    }
}

pub(crate) fn compare_bytes(
    expected: &str,
    value: &str,
    prefix: bool,
    work: &mut InteractionPredicateWork,
) -> Result<bool, InteractionPredicateEvidenceError> {
    if expected.len() > value.len() || (!prefix && expected.len() != value.len()) {
        return Ok(false);
    }
    for (left, right) in expected.bytes().zip(value.bytes()) {
        work.charge(1)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledInteractionRoleMatch {
    token: String,
}

impl CompiledInteractionRoleMatch {
    pub fn compile(token: &str) -> Result<Self, String> {
        if !crate::interaction_predicates::is_bounded_lowercase_token(token) {
            return Err("role must be one bounded lowercase role-attribute token".into());
        }
        Ok(Self {
            token: token.into(),
        })
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn matches(
        &self,
        evidence: InteractionAttributeEvidence<'_>,
        work: &mut InteractionPredicateWork,
    ) -> Result<bool, InteractionPredicateEvidenceError> {
        work.charge(1)?;
        let Some(value) = evidence.value()? else {
            return Ok(false);
        };
        let mut start = 0;
        for (index, byte) in value.bytes().enumerate() {
            work.charge(1)?;
            // HTML token separation uses exactly these five ASCII bytes.
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c) {
                if compare_bytes(&self.token, &value[start..index], false, work)? {
                    return Ok(true);
                }
                start = index + 1;
            }
        }
        compare_bytes(&self.token, &value[start..], false, work)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledInteractionHrefMatch {
    origin: Option<String>,
    path: Option<String>,
}

impl TryFrom<&InteractionHrefMatch> for CompiledInteractionHrefMatch {
    type Error = String;

    fn try_from(value: &InteractionHrefMatch) -> Result<Self, Self::Error> {
        InteractionPredicates {
            href: Some(value.clone()),
            ..Default::default()
        }
        .validate()?;
        Ok(Self {
            origin: value.origin.clone(),
            path: value.path.clone(),
        })
    }
}

impl CompiledInteractionHrefMatch {
    pub fn matches(
        &self,
        is_link: Option<bool>,
        evidence: InteractionAttributeEvidence<'_>,
        work: &mut InteractionPredicateWork,
    ) -> Result<bool, InteractionPredicateEvidenceError> {
        work.charge(1)?;
        match is_link {
            Some(true) => {}
            Some(false) => return Ok(false),
            None => return Err(InteractionPredicateEvidenceError::Missing),
        }
        let Some(value) = evidence.value()? else {
            return Ok(false);
        };
        if value.len() > MAX_INTERACTION_HREF_EVIDENCE_BYTES {
            return Err(InteractionPredicateEvidenceError::HrefTooLong);
        }
        work.charge(value.len())?;
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(InteractionPredicateEvidenceError::InvalidHref);
        }
        let url =
            url::Url::parse(value).map_err(|_| InteractionPredicateEvidenceError::InvalidHref)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Ok(false);
        }
        if let Some(origin) = &self.origin
            && !compare_bytes(origin, &url.origin().ascii_serialization(), false, work)?
        {
            return Ok(false);
        }
        let Some(path) = &self.path else {
            return Ok(true);
        };
        crate::routing::glob_matches_with_work(path, url.path(), || work.charge(1).is_ok())
            .ok_or(InteractionPredicateEvidenceError::WorkExhausted)
    }
}

/// Borrowed facts contain no candidate ID, command, transport or permission.
/// Missing geometry/link classification is unavailable when requested. The
/// caller supplies only independently complete, correctly projected facts.
#[derive(Clone, Copy, Debug)]
pub struct InteractionPredicateFacts<'a> {
    pub hint_name: InteractionNameEvidence<'a>,
    pub is_link: Option<bool>,
    pub href: InteractionAttributeEvidence<'a>,
    pub role: InteractionAttributeEvidence<'a>,
    pub width_px: Option<f64>,
    pub height_px: Option<f64>,
    pub viewport_x: Option<f64>,
    pub viewport_y: Option<f64>,
}

impl Default for InteractionPredicateFacts<'_> {
    fn default() -> Self {
        Self {
            hint_name: InteractionNameEvidence::missing(),
            is_link: None,
            href: InteractionAttributeEvidence::Missing,
            role: InteractionAttributeEvidence::Missing,
            width_px: None,
            height_px: None,
            viewport_x: None,
            viewport_y: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledInteractionPredicates {
    name: Option<CompiledInteractionNameMatch>,
    href: Option<CompiledInteractionHrefMatch>,
    role: Option<CompiledInteractionRoleMatch>,
    attributes: Vec<CompiledInteractionAttributeTest>,
    ranges: [Option<InteractionNumericRange>; 4],
}

impl TryFrom<&InteractionPredicates> for CompiledInteractionPredicates {
    type Error = String;

    fn try_from(value: &InteractionPredicates) -> Result<Self, Self::Error> {
        value.validate()?;
        Ok(Self {
            name: value
                .hint_name
                .as_ref()
                .map(CompiledInteractionNameMatch::try_from)
                .transpose()?,
            href: value
                .href
                .as_ref()
                .map(CompiledInteractionHrefMatch::try_from)
                .transpose()?,
            role: value
                .role
                .as_deref()
                .map(CompiledInteractionRoleMatch::compile)
                .transpose()?,
            attributes: value
                .attributes
                .iter()
                .flatten()
                .map(CompiledInteractionAttributeTest::try_from)
                .collect::<Result<_, _>>()?,
            ranges: [
                value.width_px,
                value.height_px,
                value.viewport_x,
                value.viewport_y,
            ],
        })
    }
}

impl CompiledInteractionPredicates {
    /// Every configured constraint is conjunctive. A proven nonmatch may stop
    /// early; unavailable facts never supply a positive match. The attribute
    /// lookup must use already captured facts, not perform DOM work here.
    pub fn matches<'a>(
        &self,
        facts: &InteractionPredicateFacts<'a>,
        mut attribute: impl FnMut(&str) -> InteractionAttributeEvidence<'a>,
        work: &mut InteractionPredicateWork,
    ) -> Result<bool, InteractionPredicateEvidenceError> {
        if let Some(name) = &self.name
            && !name.matches_with_work(facts.hint_name, work)?
        {
            return Ok(false);
        }
        if let Some(href) = &self.href
            && !href.matches(facts.is_link, facts.href, work)?
        {
            return Ok(false);
        }
        if let Some(role) = &self.role
            && !role.matches(facts.role, work)?
        {
            return Ok(false);
        }
        for test in &self.attributes {
            if !test.matches(attribute(test.name()), work)? {
                return Ok(false);
            }
        }
        for (range, evidence) in self.ranges.iter().zip([
            facts.width_px,
            facts.height_px,
            facts.viewport_x,
            facts.viewport_y,
        ]) {
            if let Some(range) = range {
                work.charge(1)?;
                let value = evidence.ok_or(InteractionPredicateEvidenceError::Missing)?;
                if !value.is_finite() {
                    return Err(InteractionPredicateEvidenceError::InvalidGeometry);
                }
                if value < range.min || value > range.max {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
