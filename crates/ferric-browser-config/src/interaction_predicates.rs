//! Typed conjunctive predicates. Authored text stays intact; name compilation
//! normalizes only its own operand and complete bounded name evidence.
use serde::{Deserialize, Serialize};

pub const MAX_INTERACTION_ATTRIBUTE_TESTS: usize = 4;
pub const MAX_INTERACTION_MATCH_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionTextCase {
    Sensitive,
    AsciiInsensitive,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionNameMatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub equals: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub contains: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub case: Option<InteractionTextCase>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionHrefMatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub origin: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionAttributeTest {
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub equals: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub prefix: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub present: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionNumericRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionPredicates {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub hint_name: Option<InteractionNameMatch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub href: Option<InteractionHrefMatch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub role: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub attributes: Option<Vec<InteractionAttributeTest>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub width_px: Option<InteractionNumericRange>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub height_px: Option<InteractionNumericRange>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub viewport_x: Option<InteractionNumericRange>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub viewport_y: Option<InteractionNumericRange>,
}

impl InteractionPredicates {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hint_name.is_none()
            && self.href.is_none()
            && self.role.is_none()
            && self.attributes.as_ref().is_none_or(Vec::is_empty)
            && self.width_px.is_none()
            && self.height_px.is_none()
            && self.viewport_x.is_none()
            && self.viewport_y.is_none()
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.hint_name {
            name.validate()?;
        }
        if let Some(href) = &self.href {
            if href.origin.is_none() && href.path.is_none() {
                return Err("href requires origin or path".into());
            }
            if href
                .origin
                .as_ref()
                .is_some_and(|origin| !crate::is_canonical_interaction_origin(origin))
            {
                return Err("href origin must be an exact canonical HTTP(S) origin".into());
            }
            if let Some(path) = &href.path
                && (!path.starts_with('/')
                    || path.contains(['?', '#'])
                    || path.chars().any(|c| c.is_control() || c.is_whitespace()))
            {
                return Err("href path must use the existing absolute path-glob grammar".into());
            }
        }
        if let Some(role) = &self.role
            && !is_bounded_lowercase_token(role)
        {
            return Err("role must be one bounded lowercase role-attribute token".into());
        }
        if let Some(attributes) = &self.attributes {
            if attributes.len() > MAX_INTERACTION_ATTRIBUTE_TESTS {
                return Err("predicates support at most four attribute tests".into());
            }
            for attribute in attributes {
                attribute.validate()?;
            }
        }
        for range in [self.width_px, self.height_px].into_iter().flatten() {
            range.validate(1_000_000.0)?;
        }
        for range in [self.viewport_x, self.viewport_y].into_iter().flatten() {
            range.validate(1.0)?;
        }
        Ok(())
    }
}

impl InteractionNameMatch {
    pub fn validate(&self) -> Result<(), String> {
        let ((Some(value), None) | (None, Some(value))) = (&self.equals, &self.contains) else {
            return Err("hint_name requires exactly one equals or contains".into());
        };
        validate_match_value(value)
    }
}

impl InteractionAttributeTest {
    pub fn validate(&self) -> Result<(), String> {
        validate_interaction_attribute_name(&self.name)?;
        match (&self.equals, &self.prefix, self.present) {
            (Some(value), None, None) | (None, Some(value), None) => validate_match_value(value),
            (None, None, Some(true)) => Ok(()),
            _ => Err("attribute test requires exactly one equals, prefix or present=true".into()),
        }
    }
}

impl InteractionNumericRange {
    fn validate(self, ceiling: f64) -> Result<(), String> {
        if !self.min.is_finite()
            || !self.max.is_finite()
            || self.min < 0.0
            || self.max > ceiling
            || self.min > self.max
        {
            return Err("predicate range must be finite, ordered and within its bounds".into());
        }
        Ok(())
    }
}

pub fn validate_interaction_attribute_name(name: &str) -> Result<(), String> {
    if !is_bounded_lowercase_token(name) || name.contains(['"', '\'', '<', '>', '/', '=']) {
        return Err("attribute name must be bounded lowercase HTML attribute text".into());
    }
    Ok(())
}

pub(crate) fn is_bounded_lowercase_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_INTERACTION_MATCH_BYTES
        && value.to_lowercase() == value
        && !value
            .chars()
            .any(|c| c.is_control() || c.is_ascii_whitespace())
}

fn validate_match_value(value: &str) -> Result<(), String> {
    if value.len() > MAX_INTERACTION_MATCH_BYTES {
        return Err("predicate match text exceeds 256 UTF-8 bytes".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid(source: &str) -> bool {
        toml::from_str::<InteractionPredicates>(source).is_ok_and(|p| p.validate().is_ok())
    }
    #[test]
    fn choices_unknown_fields_and_bounds_are_checked() {
        for source in [
            "",
            "[hint_name]\nequals='Mac'",
            "[href]\npath='/watch*'",
            "[[attributes]]\nname='data-id'\npresent=true",
            "[[attributes]]\nname='data-é'\npresent=true",
            "role='sélecteur'",
            "role='custom:token'",
            "[width_px]\nmin=0\nmax=1000000",
            "[viewport_x]\nmin=0\nmax=1",
        ] {
            assert!(valid(source), "{source}");
        }
        for source in [
            "unknown=true",
            "role='two tokens'",
            "role='CAPITAL'",
            "[[attributes]]\nname='data=x'\npresent=true",
            "[hint_name]\ncase='sensitive'",
            "[hint_name]\nequals='a'\ncontains='b'",
            "[href]",
            "[href]\npath='/x?query'",
            "[[attributes]]\nname='Data-ID'\npresent=true",
            "[[attributes]]\nname='data-id'\npresent=false",
            "[[attributes]]\nname='data-id'\nequals='a'\nprefix='a'",
            "[width_px]\nmin=2\nmax=1",
            "[height_px]\nmin=0\nmax=inf",
            "[viewport_y]\nmin=nan\nmax=1",
            "[viewport_x]\nmin=0\nmax=1.01",
        ] {
            assert!(!valid(source), "{source}");
        }
        for count in [4, 5] {
            let source = "[[attributes]]\nname='data-id'\npresent=true\n".repeat(count);
            assert_eq!(valid(&source), count == 4);
        }
        for count in [128, 129] {
            assert_eq!(
                valid(&format!("[hint_name]\nequals='{}'", "é".repeat(count))),
                count == 128
            );
        }
    }
    #[test]
    fn serialization_preserves_sparse_data_and_authored_text() {
        let source = "[hint_name]\nequals='  Mac  '\ncase='ascii-insensitive'\n";
        let predicate: InteractionPredicates = toml::from_str(source).unwrap();
        predicate.validate().unwrap();
        let encoded = toml::Value::try_from(&predicate).unwrap();
        assert_eq!(encoded, toml::from_str::<toml::Value>(source).unwrap());
        assert!(!predicate.is_empty());
        assert!(InteractionPredicates::default().is_empty());
        assert!(
            toml::from_str::<InteractionPredicates>("attributes=[]")
                .unwrap()
                .is_empty()
        );
    }
}
