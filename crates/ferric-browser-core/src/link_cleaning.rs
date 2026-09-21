use std::collections::BTreeSet;

const MAX_RULES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostPattern {
    Any,
    Exact(String),
    Subdomains(String),
}

impl HostPattern {
    fn matches(&self, host: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => host == expected,
            Self::Subdomains(expected) => {
                host == expected || host.ends_with(&format!(".{expected}"))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleaningRule {
    pub id: String,
    pub host: HostPattern,
    pub remove_parameters: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleaningRules {
    pub revision: String,
    pub source: String,
    pub rules: Vec<CleaningRule>,
}

impl CleaningRules {
    /// Creates a rule set after checking stable IDs and bounded rule data.
    ///
    /// # Errors
    ///
    /// Returns an error for empty or duplicate IDs, empty parameter names, or
    /// more than 512 rules.
    pub fn new(
        revision: impl Into<String>,
        source: impl Into<String>,
        rules: Vec<CleaningRule>,
    ) -> Result<Self, LinkCleanError> {
        if rules.len() > MAX_RULES {
            return Err(LinkCleanError::RuleLimit);
        }
        let mut ids = BTreeSet::new();
        for rule in &rules {
            if rule.id.trim().is_empty() || !ids.insert(rule.id.clone()) {
                return Err(LinkCleanError::InvalidRules);
            }
            if rule.remove_parameters.is_empty()
                || rule.remove_parameters.iter().any(|parameter| {
                    parameter.is_empty() || parameter.contains(['&', '=', '#', '?'])
                })
            {
                return Err(LinkCleanError::InvalidRules);
            }
        }
        Ok(Self {
            revision: revision.into(),
            source: source.into(),
            rules,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanLinkResult {
    pub original: String,
    pub cleaned: String,
    pub changed: bool,
    pub applied_rules: Vec<String>,
    pub removed_parameters: Vec<String>,
    pub retained_parameters: Vec<String>,
    pub explanation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkCleanError {
    InvalidInput,
    InvalidRules,
    RuleLimit,
}

impl std::fmt::Display for LinkCleanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput => {
                formatter.write_str("link is empty or contains a control character")
            }
            Self::InvalidRules => formatter.write_str("clean-link rules are invalid"),
            Self::RuleLimit => formatter.write_str("clean-link rule count exceeds 512"),
        }
    }
}

impl std::error::Error for LinkCleanError {}

/// Applies explicitly supplied, reviewed data rules to one captured URL.
///
/// Only HTTP(S) query parameters are considered. Component order, encoding,
/// fragments, and all non-matching values are copied byte-for-byte. Unsupported
/// schemes and ambiguous authority/query shapes return an unchanged result with
/// an explanation; this function never performs network or wrapper expansion.
///
/// # Errors
///
/// Returns an error only for an empty/control-character input or invalid rules.
#[allow(clippy::too_many_lines)]
pub fn clean_link(input: &str, rules: &CleaningRules) -> Result<CleanLinkResult, LinkCleanError> {
    if input.is_empty() || input.chars().any(char::is_control) {
        return Err(LinkCleanError::InvalidInput);
    }
    let original = input.to_owned();
    let Some((scheme, after_scheme)) = input.split_once(':') else {
        return Ok(unchanged(
            original,
            "No scheme; the original URL was retained.",
        ));
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Ok(unchanged(
            original,
            "Only HTTP(S) query parameters are eligible for cleaning.",
        ));
    }
    let Some(authority_body) = after_scheme.strip_prefix("//") else {
        return Ok(unchanged(
            original,
            "The URL has no authority; the original URL was retained.",
        ));
    };
    let authority_end = authority_body
        .find(['/', '?', '#'])
        .map_or(after_scheme.len(), |offset| offset + 2);
    let authority = &after_scheme[2..authority_end];
    let Some(host) = normalized_host(authority) else {
        return Ok(unchanged(
            original,
            "The URL authority was ambiguous; the original URL was retained.",
        ));
    };
    let Some(query_start) = after_scheme.find('?') else {
        return Ok(unchanged(
            original,
            "The URL has no query parameters to clean.",
        ));
    };
    if query_start < authority_end {
        return Ok(unchanged(
            original,
            "The URL authority/query shape was ambiguous; the original URL was retained.",
        ));
    }
    let query_end = after_scheme[query_start + 1..]
        .find('#')
        .map_or(after_scheme.len(), |offset| query_start + 1 + offset);
    let query = &after_scheme[query_start + 1..query_end];
    let mut removable = BTreeSet::new();
    let mut matching_rules = BTreeSet::new();
    for rule in &rules.rules {
        if rule.host.matches(&host) {
            removable.extend(rule.remove_parameters.iter().cloned());
        }
    }
    if removable.is_empty() {
        return Ok(unchanged(original, "No configured rule matches this host."));
    }
    let mut kept = Vec::new();
    let mut removed_parameters = Vec::new();
    let mut retained_parameters = Vec::new();
    for component in query.split('&') {
        let name = component
            .split_once('=')
            .map_or(component, |(name, _)| name);
        if removable.contains(name) {
            removed_parameters.push(name.to_owned());
            for rule in &rules.rules {
                if rule.host.matches(&host) && rule.remove_parameters.contains(name) {
                    matching_rules.insert(rule.id.clone());
                }
            }
        } else {
            if !name.is_empty() {
                retained_parameters.push(name.to_owned());
            }
            kept.push(component);
        }
    }
    if removed_parameters.is_empty() {
        return Ok(unchanged(
            original,
            "Configured rules matched the host, but no removable parameter was present.",
        ));
    }
    let mut cleaned = String::with_capacity(input.len());
    cleaned.push_str(scheme);
    cleaned.push(':');
    cleaned.push_str(&after_scheme[..query_start]);
    if !kept.is_empty() {
        cleaned.push('?');
        cleaned.push_str(&kept.join("&"));
    }
    cleaned.push_str(&after_scheme[query_end..]);
    Ok(CleanLinkResult {
        original,
        cleaned,
        changed: true,
        applied_rules: matching_rules.into_iter().collect(),
        retained_parameters,
        explanation: format!(
            "Removed {} reviewed tracking parameter(s); retained all other components.",
            removed_parameters.len()
        ),
        removed_parameters,
    })
}

#[must_use]
pub fn builtin_rules() -> CleaningRules {
    let parameters = [
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "gclid",
        "fbclid",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    CleaningRules {
        revision: "builtin-1".into(),
        source: "builtin".into(),
        rules: vec![CleaningRule {
            id: "builtin-tracking-query".into(),
            host: HostPattern::Any,
            remove_parameters: parameters,
        }],
    }
}

fn normalized_host(authority: &str) -> Option<String> {
    let authority = authority.rsplit('@').next()?;
    if authority.is_empty() || authority.contains(['/', '?', '#']) {
        return None;
    }
    let host = if authority.starts_with('[') {
        authority
            .split_once(']')
            .map(|(host, _)| host.strip_prefix('[').unwrap_or(host))?
    } else {
        authority.split(':').next()?
    };
    if host.is_empty() || host.chars().any(char::is_whitespace) {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn unchanged(original: String, explanation: &str) -> CleanLinkResult {
    CleanLinkResult {
        cleaned: original.clone(),
        original,
        changed: false,
        applied_rules: Vec::new(),
        removed_parameters: Vec::new(),
        retained_parameters: Vec::new(),
        explanation: explanation.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_cleaning_preserves_order_encoding_and_fragment() {
        let result = clean_link(
            "https://example.test/p?a=1&utm_source=news&b=%2F&fbclid=abc#keep",
            &builtin_rules(),
        )
        .expect("clean");
        assert_eq!(result.cleaned, "https://example.test/p?a=1&b=%2F#keep");
        assert_eq!(result.removed_parameters, ["utm_source", "fbclid"]);
        assert_eq!(result.retained_parameters, ["a", "b"]);
        assert!(result.changed);
    }

    #[test]
    fn unsupported_and_ambiguous_inputs_are_noop() {
        let rules = builtin_rules();
        for input in [
            "mailto:test@example.test?utm_source=x",
            "https://?utm_source=x",
            "https://example.test/?safe=1",
        ] {
            let result = clean_link(input, &rules).expect("result");
            assert!(!result.changed);
            assert_eq!(result.cleaned, input);
        }
    }

    #[test]
    fn encoded_names_and_auth_values_are_not_decoded_or_removed() {
        let result = clean_link(
            "https://user:secret@example.test/?%75tm_source=x&token=abc",
            &builtin_rules(),
        )
        .expect("clean");
        assert!(!result.changed);
        assert_eq!(
            result.cleaned,
            "https://user:secret@example.test/?%75tm_source=x&token=abc"
        );
    }

    #[test]
    fn rule_sets_are_bounded_and_deduplicated() {
        let rule = CleaningRule {
            id: "one".into(),
            host: HostPattern::Exact("example.test".into()),
            remove_parameters: ["track".into()].into_iter().collect(),
        };
        assert!(CleaningRules::new("1", "test", vec![rule.clone(), rule]).is_err());
        assert!(
            CleaningRules::new(
                "1",
                "test",
                (0..=MAX_RULES)
                    .map(|index| CleaningRule {
                        id: index.to_string(),
                        host: HostPattern::Any,
                        remove_parameters: ["x".into()].into_iter().collect()
                    })
                    .collect()
            )
            .is_err()
        );
    }
}
