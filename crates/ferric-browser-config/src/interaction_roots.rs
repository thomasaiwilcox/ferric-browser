//! Data-only explicit root paths. Live uniqueness, scope and receiver authority
//! must still be proved by the browser/collector; static validity grants none.
use serde::{Deserialize, Serialize};

pub const MAX_INTERACTION_ROOT_STEPS: usize = ferric_browser_core::MAX_HINT_ROOT_DEPTH;
pub const MAX_INTERACTION_SELECTOR_BYTES: usize = 1_024;
pub const MAX_INTERACTION_RECEIVER_ORIGINS: usize = 8;

/// Scope for new page-command/remap authority only. Raw and pre-existing
/// unchanged passthrough retain their own receiver contracts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
pub enum InteractionPageReceiver {
    TopDocument {},
    FocusedFrame { origins: Vec<String> },
    TopOrFocusedFrame { origins: Vec<String> },
}

impl Default for InteractionPageReceiver {
    fn default() -> Self {
        Self::TopDocument {}
    }
}

impl InteractionPageReceiver {
    pub fn validate(&self) -> Result<(), String> {
        let origins = match self {
            Self::TopDocument {} => return Ok(()),
            Self::FocusedFrame { origins } | Self::TopOrFocusedFrame { origins } => origins,
        };
        if origins.is_empty() || origins.len() > MAX_INTERACTION_RECEIVER_ORIGINS {
            return Err("frame receivers require 1..=8 exact origins".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for origin in origins {
            if !is_canonical_interaction_origin(origin) {
                return Err("receiver origins must be exact canonical HTTP(S) origins".into());
            }
            if !seen.insert(origin) {
                return Err("receiver origins must be unique".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionRootStart {
    TopDocument,
    FocusedFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionRootStepKind {
    OpenShadow,
    SameOriginFrame,
    AssignedSlot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRootStep {
    pub kind: InteractionRootStepKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub host_selector: Option<String>,
    #[serde(
        default,
        rename = "self",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub host_self: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub assigned_selector: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRoot {
    pub start: InteractionRootStart,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub receiver_origin: Option<String>,
    #[serde(default)]
    pub steps: Vec<InteractionRootStep>,
}

impl Default for InteractionRoot {
    fn default() -> Self {
        Self {
            start: InteractionRootStart::TopDocument,
            receiver_origin: None,
            steps: Vec::new(),
        }
    }
}

impl InteractionRoot {
    /// Checks descriptor shape, origin and cumulative root depth. The supplied
    /// selector validator must implement the constrained Interaction grammar;
    /// engine CSS validation and live proof remain separate stages.
    pub fn validate(
        &self,
        prior_depth: usize,
        mut validate_selector: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        match (self.start, self.receiver_origin.as_deref()) {
            (InteractionRootStart::TopDocument, None) => {}
            (InteractionRootStart::FocusedFrame, Some(origin)) => {
                if !is_canonical_interaction_origin(origin) {
                    return Err(
                        "focused-frame root requires an exact canonical HTTP(S) origin".into(),
                    );
                }
            }
            _ => return Err("receiver_origin is required only for focused-frame roots".into()),
        }
        validate_root_steps(&self.steps, prior_depth, false, &mut validate_selector)
    }
}

/// Validates per-item primary paths starting at an Element. Callers carry the depth already spent
/// resolving the collection root; an item path cannot reset the shared limit.
pub fn validate_interaction_primary_root_steps(
    steps: &[InteractionRootStep],
    prior_depth: usize,
    validate_selector: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    validate_root_steps(steps, prior_depth, true, validate_selector)
}

fn validate_root_steps(
    steps: &[InteractionRootStep],
    prior_depth: usize,
    mut current_is_element: bool,
    mut validate_selector: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    if prior_depth > MAX_INTERACTION_ROOT_STEPS
        || steps.len() > MAX_INTERACTION_ROOT_STEPS - prior_depth
    {
        return Err("Interaction root paths exceed shared depth 8".into());
    }
    for step in steps {
        match (step.host_selector.as_deref(), step.host_self) {
            (Some(selector), None) => validate_bounded_selector(selector, &mut validate_selector)?,
            (None, Some(true)) if current_is_element => {}
            _ => return Err("root hop requires exactly one host_selector or self=true".into()),
        }
        match (step.kind, step.assigned_selector.as_deref()) {
            (InteractionRootStepKind::AssignedSlot, Some(selector)) => {
                validate_bounded_selector(selector, &mut validate_selector)?;
            }
            (InteractionRootStepKind::AssignedSlot, None) => {
                return Err("assigned-slot requires assigned_selector".into());
            }
            (_, Some(_)) => return Err("assigned_selector is valid only for assigned-slot".into()),
            (_, None) => {}
        }
        current_is_element = step.kind == InteractionRootStepKind::AssignedSlot;
    }
    Ok(())
}

/// Receiver authority uses browser URL origin serialization, not the existing
/// site-pattern matcher. This rejects shorthand/hex IPv4, malformed IPv6 and
/// noncanonical spellings without changing legacy URL matching semantics.
#[must_use]
pub fn is_canonical_interaction_origin(origin: &str) -> bool {
    url::Url::parse(origin).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|host| !host.contains('*'))
            && url.origin().ascii_serialization() == origin
    })
}

fn validate_bounded_selector(
    selector: &str,
    validate: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    if selector.is_empty() || selector.len() > MAX_INTERACTION_SELECTOR_BYTES {
        return Err("Interaction selector must contain 1..=1024 UTF-8 bytes".into());
    }
    validate(selector)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(source: &str) -> InteractionRootStep {
        toml::from_str(source).unwrap()
    }

    #[test]
    fn root_shape_and_origin_are_strict() {
        for origin in [
            "https://example.test",
            "http://127.0.0.1:8766",
            "https://[::1]",
            "https://[::ffff:c000:280]",
            "https://example.test.",
        ] {
            let root = InteractionRoot {
                start: InteractionRootStart::FocusedFrame,
                receiver_origin: Some(origin.into()),
                steps: vec![],
            };
            assert!(root.validate(0, |_| Ok(())).is_ok(), "{origin}");
            let encoded = toml::to_string(&root).unwrap();
            assert_eq!(toml::from_str::<InteractionRoot>(&encoded).unwrap(), root);
        }
        for origin in [
            "https://example.test/",
            "https://EXAMPLE.test",
            "https://example.test:443",
            "https://*.example.test",
            "https://user@example.test",
            "data:text/plain,a",
            "https://[:::]",
            "http://127.1",
            "http://2130706433",
            "http://0x7f000001",
            "http://0177.0.0.1",
        ] {
            let root = InteractionRoot {
                start: InteractionRootStart::FocusedFrame,
                receiver_origin: Some(origin.into()),
                steps: vec![],
            };
            assert!(root.validate(0, |_| Ok(())).is_err(), "{origin}");
        }
        assert!(toml::from_str::<InteractionRoot>("start='top-document'\nscript='x'").is_err());
        let root = InteractionRoot {
            receiver_origin: Some("https://example.test".into()),
            ..InteractionRoot::default()
        };
        assert!(root.validate(0, |_| Ok(())).is_err());
    }

    #[test]
    fn hop_exclusivity_slots_and_shared_depth_are_checked() {
        let valid = step("kind='open-shadow'\nself=true");
        assert!(
            validate_interaction_primary_root_steps(std::slice::from_ref(&valid), 7, |_| Ok(()))
                .is_ok()
        );
        assert!(validate_interaction_primary_root_steps(&[valid], 8, |_| Ok(())).is_err());
        assert!(validate_interaction_primary_root_steps(&[], usize::MAX, |_| Ok(())).is_err());
        for invalid in [
            "kind='open-shadow'",
            "kind='open-shadow'\nself=false",
            "kind='open-shadow'\nself=true\nhost_selector='a'",
            "kind='assigned-slot'\nself=true",
            "kind='open-shadow'\nself=true\nassigned_selector='a'",
        ] {
            assert!(
                validate_interaction_primary_root_steps(&[step(invalid)], 0, |_| Ok(())).is_err()
            );
        }
        let slot = step("kind='assigned-slot'\nhost_selector='slot'\nassigned_selector='a'");
        let mut seen = Vec::new();
        validate_interaction_primary_root_steps(&[slot], 0, |s| {
            seen.push(s.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, ["slot", "a"]);
        assert!(
            toml::from_str::<InteractionRootStep>("kind='open-shadow'\nself=true\nextra=1")
                .is_err()
        );
    }

    #[test]
    fn selector_bytes_and_validation_stage_are_preserved() {
        for (selector, valid) in [
            ("a".repeat(1_024), true),
            ("a".repeat(1_025), false),
            ("é".repeat(512), true),
            ("é".repeat(513), false),
            (String::new(), false),
        ] {
            let hop = InteractionRootStep {
                kind: InteractionRootStepKind::OpenShadow,
                host_selector: Some(selector),
                host_self: None,
                assigned_selector: None,
            };
            let mut calls = 0;
            let result = validate_interaction_primary_root_steps(&[hop], 0, |_| {
                calls += 1;
                Ok(())
            });
            assert_eq!(result.is_ok(), valid);
            assert_eq!(calls, usize::from(valid));
        }
        let hop = step("kind='open-shadow'\nhost_selector='a'");
        let result = validate_interaction_primary_root_steps(&[hop], 0, |_| {
            Err("constrained selector rejected".into())
        });
        assert_eq!(result.unwrap_err(), "constrained selector rejected");
    }

    #[test]
    fn self_requires_an_element_at_each_hop() {
        let self_hop = step("kind='open-shadow'\nself=true");
        let root = InteractionRoot {
            steps: vec![self_hop.clone()],
            ..InteractionRoot::default()
        };
        assert!(root.validate(0, |_| Ok(())).is_err());
        assert!(
            validate_interaction_primary_root_steps(std::slice::from_ref(&self_hop), 0, |_| Ok(()))
                .is_ok()
        );
        for first in [
            "kind='open-shadow'\nhost_selector='x-host'",
            "kind='same-origin-frame'\nhost_selector='iframe'",
        ] {
            let root = InteractionRoot {
                steps: vec![step(first), self_hop.clone()],
                ..InteractionRoot::default()
            };
            assert!(root.validate(0, |_| Ok(())).is_err());
        }
        let root = InteractionRoot {
            steps: vec![
                step("kind='assigned-slot'\nhost_selector='slot'\nassigned_selector='x-host'"),
                self_hop,
            ],
            ..InteractionRoot::default()
        };
        assert!(root.validate(0, |_| Ok(())).is_ok());
    }

    #[test]
    fn receiver_shape_is_tagged_strict_and_bounded() {
        for source in [
            "scope='top-document'",
            "scope='focused-frame'\norigins=['https://example.test']",
            "scope='top-or-focused-frame'\norigins=['https://example.test']",
        ] {
            let receiver: InteractionPageReceiver = toml::from_str(source).unwrap();
            receiver.validate().unwrap();
            let encoded = toml::to_string(&receiver).unwrap();
            assert_eq!(
                toml::from_str::<InteractionPageReceiver>(&encoded).unwrap(),
                receiver
            );
        }
        for source in [
            "scope='top-document'\norigins=[]",
            "scope='focused-frame'",
            "scope='focused-frame'\norigins=[]\nextra=true",
        ] {
            assert!(
                toml::from_str::<InteractionPageReceiver>(source).is_err(),
                "{source}"
            );
        }
        for count in [0, 8, 9] {
            let receiver = InteractionPageReceiver::FocusedFrame {
                origins: (0..count)
                    .map(|n| format!("https://host{n}.test"))
                    .collect(),
            };
            assert_eq!(receiver.validate().is_ok(), count == 8);
        }
        let receiver = InteractionPageReceiver::FocusedFrame {
            origins: vec!["https://example.test".into(); 2],
        };
        assert!(receiver.validate().is_err());
    }
}
