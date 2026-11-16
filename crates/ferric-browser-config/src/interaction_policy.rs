//! Sparse authored Interaction data. These types are not runtime authority:
//! complete static/effective validation and compilation precede publication.
use crate::{
    InteractionAttributeTest, InteractionPageReceiver, InteractionPredicates, InteractionRoot,
    InteractionRootStep,
};
use serde::{Deserialize, Serialize};

/// Missing optional fields contribute nothing; explicitly supplied null is
/// invalid data, not another spelling of absence on JSON-facing surfaces.
pub(crate) fn deserialize_present_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

pub const MAX_INTERACTION_ANCHORS: usize = 64;
pub const MAX_INTERACTION_COLLECTIONS: usize = 16;
pub const MAX_INTERACTION_COLLECTION_CODES: usize = 64;
pub const MAX_INTERACTION_RESERVATIONS: usize = ferric_browser_core::MAX_HINT_SPECIAL_RESERVATIONS;
pub const MAX_INTERACTION_REMAPS: usize = 32;
pub const MAX_INTERACTION_HINT_PAGE_KEYS: usize = 32;
pub const MAX_INTERACTION_EXCLUDED_CHARS: usize = 32;
pub const MAX_INTERACTION_LABEL_BYTES: usize = ferric_browser_core::MAX_HINT_CODE_BYTES;
pub const MAX_INTERACTION_FALLBACK_PREFIX_BYTES: usize = MAX_INTERACTION_LABEL_BYTES - 1;
pub const MAX_INTERACTION_IDENTITY_ATTRIBUTES: usize = 4;
pub const MAX_INTERACTION_REMAP_CONTEXTS: usize = 4;
pub const MAX_INTERACTION_FAMILIES: usize = 11;
/// Applied by the enhanced codebook compiler after controls/exclusions/page
/// sources are removed. Do not impose this maximum on the raw authored input.
pub const MIN_INTERACTION_FALLBACK_ALPHABET_CHARS: usize =
    ferric_browser_core::MIN_HINT_ALPHABET_CHARS;
pub const MAX_INTERACTION_FALLBACK_ALPHABET_CHARS: usize =
    ferric_browser_core::MAX_HINT_ALPHABET_CHARS;
pub const MAX_INTERACTION_ID_BYTES: usize = 64;
pub const MAX_INTERACTION_KEY_BYTES: usize = ferric_browser_core::MAX_LOGICAL_KEY_TOKEN_BYTES;
pub const MAX_INTERACTION_AUTHORED_BYTES: usize = 64 * 1024;
pub const MAX_INTERACTION_EFFECTIVE_BYTES: usize = 128 * 1024;
pub const MAX_INTERACTION_AUTHORED_SELECTOR_BYTES: usize = 16 * 1024;
pub const MAX_INTERACTION_EFFECTIVE_SELECTOR_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionFamily {
    All,
    Actions,
    Focus,
    Links,
    Inputs,
    Buttons,
    Images,
    Media,
    Scrollables,
    Regions,
    Surfaces,
}

impl InteractionFamily {
    pub const ALL: [Self; MAX_INTERACTION_FAMILIES] = [
        Self::All,
        Self::Actions,
        Self::Focus,
        Self::Links,
        Self::Inputs,
        Self::Buttons,
        Self::Images,
        Self::Media,
        Self::Scrollables,
        Self::Regions,
        Self::Surfaces,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionAutoFollow {
    Always,
    UniqueMatch,
    FullMatch,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionCollectionOrder {
    Dom,
    TopToBottom,
    RowMajorLtr,
    RowMajorRtl,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionRemapContext {
    PassthroughOnce,
    Passthrough,
    NormalPageKey,
    HintPageKey,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionLabelsPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub alphabet: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub min_chars: Option<u8>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub fallback_prefix: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub excluded_chars: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub auto_follow: Option<InteractionAutoFollow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionAnchor {
    pub id: String,
    pub code: String,
    pub families: Vec<InteractionFamily>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub root: Option<InteractionRoot>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub scope_selector: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub target_selector: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub predicates: Option<InteractionPredicates>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionCollection {
    pub id: String,
    pub families: Vec<InteractionFamily>,
    pub codes: Vec<String>,
    pub item_selector: String,
    pub order: InteractionCollectionOrder,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub root: Option<InteractionRoot>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub container_selector: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub container_self: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub primary_root_steps: Option<Vec<InteractionRootStep>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub primary_selector: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub primary_self: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub item_attributes: Option<Vec<InteractionAttributeTest>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub primary_predicates: Option<InteractionPredicates>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub identity_attributes: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub row_tolerance_px: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRemap {
    pub id: String,
    pub from: String,
    pub to: String,
    pub contexts: Vec<InteractionRemapContext>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionPolicyPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub labels: Option<InteractionLabelsPatch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub anchors: Option<Vec<InteractionAnchor>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub collections: Option<Vec<InteractionCollection>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub hint_page_keys: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub page_receiver: Option<InteractionPageReceiver>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub remaps: Option<Vec<InteractionRemap>>,
}

/// Leaf validation for an authored label patch. Effective filtered alphabet,
/// capacity, source collisions and auto-follow compatibility are later checks.
impl InteractionLabelsPatch {
    pub fn validate_leaves(&self) -> Result<(), String> {
        if self
            .min_chars
            .is_some_and(|value| !(1..=MAX_INTERACTION_LABEL_BYTES).contains(&usize::from(value)))
        {
            return Err("min_chars must be 1..=8".into());
        }
        if let Some(alphabet) = &self.alphabet {
            let mut seen = std::collections::BTreeSet::new();
            if alphabet.is_empty()
                || !alphabet
                    .bytes()
                    .all(|c| (b' '..=b'~').contains(&c) && seen.insert(c))
            {
                return Err("alphabet must contain unique printable ASCII characters".into());
            }
        }
        if let Some(prefix) = &self.fallback_prefix
            && (prefix.len() > MAX_INTERACTION_FALLBACK_PREFIX_BYTES
                || !prefix.bytes().all(is_label_byte))
        {
            return Err("fallback_prefix must have 0..=7 label characters".into());
        }
        if let Some(excluded) = &self.excluded_chars {
            let mut seen = std::collections::BTreeSet::new();
            if excluded.len() > MAX_INTERACTION_EXCLUDED_CHARS
                || !excluded.iter().all(|s| {
                    s.len() == 1 && (b' '..=b'~').contains(&s.as_bytes()[0]) && seen.insert(s)
                })
            {
                return Err(
                    "excluded_chars requires at most 32 unique printable ASCII characters".into(),
                );
            }
        }
        Ok(())
    }
}

pub fn validate_interaction_entry_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > MAX_INTERACTION_ID_BYTES
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
    {
        return Err("Interaction entry ID must be 1..=64 ASCII letters/digits/_/./-".into());
    }
    Ok(())
}

pub fn validate_interaction_code(code: &str) -> Result<(), String> {
    if code.is_empty()
        || code.len() > MAX_INTERACTION_LABEL_BYTES
        || !code.bytes().all(is_label_byte)
    {
        return Err(
            "special code must have 1..=8 ASCII label characters, excluding Space and /".into(),
        );
    }
    Ok(())
}

fn is_label_byte(value: u8) -> bool {
    (b'!'..=b'~').contains(&value) && value != b'/'
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_examples_round_trip_without_invented_defaults() {
        for source in [
            include_str!("../tests/fixtures/site-policy/navigation.toml"),
            include_str!("../tests/fixtures/site-policy/suggestions.toml"),
            include_str!("../tests/fixtures/site-policy/shadow-grid.toml"),
            include_str!("../tests/fixtures/site-policy/frame-shadow-grid.toml"),
        ] {
            let raw: toml::Value = toml::from_str(source).unwrap();
            let section = &raw["site_rules"][0]["interaction"];
            let patch: InteractionPolicyPatch = section.clone().try_into().unwrap();
            let encoded = toml::Value::try_from(&patch).unwrap();
            let mut expected = section.clone();
            // CSS-pixel tolerances accept integer TOML input but serialize as
            // f64. Preserve semantic value and every field's presence, rather
            // than requiring integer and float TOML representations to equal.
            if let Some(collections) = expected
                .get_mut("collections")
                .and_then(toml::Value::as_array_mut)
            {
                for collection in collections {
                    if let Some(tolerance) = collection.get_mut("row_tolerance_px")
                        && tolerance.is_integer()
                    {
                        *tolerance = toml::Value::Float(tolerance.to_string().parse().unwrap());
                    }
                }
            }
            assert_eq!(encoded, expected);
            assert_eq!(encoded.try_into::<InteractionPolicyPatch>().unwrap(), patch);
        }
        for source in [
            "",
            "enabled=false",
            "anchors=[]\ncollections=[]",
            "[labels]\nmin_chars=2",
        ] {
            let patch: InteractionPolicyPatch = toml::from_str(source).unwrap();
            assert_eq!(
                toml::Value::try_from(&patch).unwrap(),
                toml::from_str::<toml::Value>(source).unwrap()
            );
        }
        for source in [
            "enabled=false\nunknown=true",
            "[labels]\nunknown=true",
            "[[remaps]]\nid='x'\nfrom='p'\nto='k'",
        ] {
            assert!(toml::from_str::<InteractionPolicyPatch>(source).is_err());
        }
    }
    #[test]
    fn authored_label_leaves_ids_and_codes_have_bounds() {
        for code in ["m", "ip", "A9", "12345678", "+"] {
            validate_interaction_code(code).unwrap();
        }
        for code in ["", "a b", "a/b", "123456789", "é", "\t"] {
            assert!(validate_interaction_code(code).is_err());
        }
        for id in ["Nav.Mac_1", "x-2"] {
            validate_interaction_entry_id(id).unwrap();
        }
        for id in ["", "a b", "é", &"a".repeat(65)] {
            assert!(validate_interaction_entry_id(id).is_err());
        }
        for source in [
            "min_chars=0",
            "min_chars=9",
            "alphabet='aa'",
            "fallback_prefix='a/b'",
            "excluded_chars=['a','a']",
            "excluded_chars=['ab']",
        ] {
            let labels: InteractionLabelsPatch = toml::from_str(source).unwrap();
            assert!(labels.validate_leaves().is_err(), "{source}");
        }
        let labels: InteractionLabelsPatch = toml::from_str(
            "min_chars=2\nalphabet='as /'\nfallback_prefix='z'\nexcluded_chars=['p']",
        )
        .unwrap();
        labels.validate_leaves().unwrap(); // Controls are removed only during fallback compilation.
    }
}
