//! Pure logical-key ownership policy for the later native Mixed router.
//!
//! This model is deliberately not configuration publication or event routing.
//! Callers must first honor native prompt/chrome/media ownership and held
//! physical-gesture bookkeeping, then pass only a new logical gesture here.

use std::collections::BTreeSet;

use unicode_segmentation::UnicodeSegmentation;

use crate::Mode;

pub const MAX_MIXED_PAGE_KEYS: usize = 64;

/// The only persistent shortcut ownership policies supported by this model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShortcutPolicy {
    /// Preserve Ferric's existing Normal-mode bindings.
    #[default]
    BrowserFirst,
    /// Forward only explicitly mapped logical keys from Normal mode.
    Mixed,
}

impl ShortcutPolicy {
    /// Returns the stable configuration spelling used by the later config layer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrowserFirst => "browser-first",
            Self::Mixed => "mixed",
        }
    }
}

/// The existing precedence order for an effective page-key policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MixedPolicySource {
    Global,
    Profile,
    Site,
    Runtime,
}

impl std::fmt::Display for MixedPolicySource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Global => "global",
            Self::Profile => "profile",
            Self::Site => "site",
            Self::Runtime => "runtime",
        })
    }
}

/// Unpublished input for one policy layer. `None` inherits; `Some([])` clears
/// every inherited page key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MixedPolicyLayer {
    pub shortcut_policy: Option<String>,
    pub page_keys: Option<Vec<String>>,
}

impl MixedPolicyLayer {
    #[must_use]
    pub fn new(shortcut_policy: Option<&str>, page_keys: Option<Vec<String>>) -> Self {
        Self {
            shortcut_policy: shortcut_policy.map(str::to_owned),
            page_keys,
        }
    }
}

/// One validated logical key token, in the spelling produced by
/// `KeyboardPresentation.logicalNormalKeyText`.
///
/// Tokens are layout-dependent logical values. They carry no physical scan
/// code, key-down identity, or promise of layout-independent matching.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalKeyToken(String);

impl LogicalKeyToken {
    /// Parses a token accepted by the existing Normal-mode logical-key
    /// canonicalizer. Reserved controls are rejected separately by the policy
    /// layer so callers can report their specific reason.
    ///
    /// # Errors
    ///
    /// Returns [`LogicalKeyTokenError`] when the input is empty, modifier-only,
    /// a sequence, contains a disallowed control, exceeds the existing QML
    /// text bound, or is not an emitted named token.
    pub fn parse(value: &str) -> Result<Self, LogicalKeyTokenError> {
        if value.is_empty() {
            return Err(LogicalKeyTokenError::Empty);
        }

        if is_named_logical_token(value) {
            return Ok(Self(value.to_owned()));
        }

        if is_modifier_only(value) {
            return Err(LogicalKeyTokenError::ModifierOnly);
        }
        if value.contains('+') {
            return Err(LogicalKeyTokenError::UnknownLogicalToken);
        }

        let utf16_units = value.encode_utf16().count();
        if utf16_units > 4 {
            return Err(LogicalKeyTokenError::TextTooLong);
        }
        if value
            .chars()
            .any(|character| u32::from(character) < 0x20 || character == '\u{7f}')
        {
            return Err(LogicalKeyTokenError::ControlCharacter);
        }
        if value.graphemes(true).count() != 1 {
            return Err(LogicalKeyTokenError::MultiKeySequence);
        }

        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalKeyTokenError {
    Empty,
    ModifierOnly,
    MultiKeySequence,
    ControlCharacter,
    TextTooLong,
    UnknownLogicalToken,
}

impl std::fmt::Display for LogicalKeyTokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "key token is empty",
            Self::ModifierOnly => "modifier-only tokens are not supported",
            Self::MultiKeySequence => "page key must be one logical key gesture",
            Self::ControlCharacter => "control characters are not valid page keys",
            Self::TextTooLong => "logical key text exceeds four UTF-16 code units",
            Self::UnknownLogicalToken => {
                "key token is not emitted by the logical-key canonicalizer"
            }
        })
    }
}

impl std::error::Error for LogicalKeyTokenError {}

/// A fully resolved policy with the source of each effective setting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveMixedKeyPolicy {
    pub shortcut_policy: ShortcutPolicy,
    pub page_keys: BTreeSet<LogicalKeyToken>,
    pub shortcut_policy_source: Option<MixedPolicySource>,
    /// Present for an explicitly supplied list, including an empty list.
    pub page_keys_source: Option<MixedPolicySource>,
}

impl Default for EffectiveMixedKeyPolicy {
    fn default() -> Self {
        Self {
            shortcut_policy: ShortcutPolicy::BrowserFirst,
            page_keys: BTreeSet::new(),
            shortcut_policy_source: None,
            page_keys_source: None,
        }
    }
}

/// Resolves global → profile → site → runtime layers. Every supplied field is
/// validated, even when a later layer replaces it, and an error returns no
/// partially resolved policy.
///
/// # Errors
///
/// Returns [`MixedPolicyError`] if any supplied layer has an unknown policy,
/// invalid, reserved, or duplicate page key, or more than 64 page keys.
pub fn resolve_mixed_key_policy(
    global: Option<&MixedPolicyLayer>,
    profile: Option<&MixedPolicyLayer>,
    site: Option<&MixedPolicyLayer>,
    runtime: Option<&MixedPolicyLayer>,
) -> Result<EffectiveMixedKeyPolicy, MixedPolicyError> {
    let mut effective = EffectiveMixedKeyPolicy::default();
    for (source, layer) in [
        (MixedPolicySource::Global, global),
        (MixedPolicySource::Profile, profile),
        (MixedPolicySource::Site, site),
        (MixedPolicySource::Runtime, runtime),
    ] {
        let Some(layer) = layer else {
            continue;
        };

        if let Some(value) = layer.shortcut_policy.as_deref() {
            effective.shortcut_policy = match value {
                "browser-first" => ShortcutPolicy::BrowserFirst,
                "mixed" => ShortcutPolicy::Mixed,
                _ => {
                    return Err(MixedPolicyError::UnknownShortcutPolicy {
                        source,
                        value: value.to_owned(),
                    });
                }
            };
            effective.shortcut_policy_source = Some(source);
        }

        if let Some(page_keys) = layer.page_keys.as_deref() {
            if page_keys.len() > MAX_MIXED_PAGE_KEYS {
                return Err(MixedPolicyError::TooManyPageKeys {
                    source,
                    count: page_keys.len(),
                    maximum: MAX_MIXED_PAGE_KEYS,
                });
            }

            let mut replacement = BTreeSet::new();
            for key in page_keys {
                if is_reserved_page_key(key) {
                    return Err(MixedPolicyError::ReservedPageKey {
                        source,
                        key: key.clone(),
                    });
                }
                let token = LogicalKeyToken::parse(key).map_err(|reason| {
                    MixedPolicyError::InvalidPageKey {
                        source,
                        key: key.clone(),
                        reason,
                    }
                })?;
                if !replacement.insert(token.clone()) {
                    return Err(MixedPolicyError::DuplicatePageKey { source, key: token });
                }
            }
            effective.page_keys = replacement;
            effective.page_keys_source = Some(source);
        }
    }
    Ok(effective)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedPolicyError {
    UnknownShortcutPolicy {
        source: MixedPolicySource,
        value: String,
    },
    InvalidPageKey {
        source: MixedPolicySource,
        key: String,
        reason: LogicalKeyTokenError,
    },
    ReservedPageKey {
        source: MixedPolicySource,
        key: String,
    },
    DuplicatePageKey {
        source: MixedPolicySource,
        key: LogicalKeyToken,
    },
    TooManyPageKeys {
        source: MixedPolicySource,
        count: usize,
        maximum: usize,
    },
}

impl std::fmt::Display for MixedPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownShortcutPolicy { source, value } => {
                write!(
                    formatter,
                    "{source} shortcut policy {value:?} is unsupported"
                )
            }
            Self::InvalidPageKey {
                source,
                key,
                reason,
            } => {
                write!(formatter, "{source} page key {key:?} is invalid: {reason}")
            }
            Self::ReservedPageKey { source, key } => {
                write!(formatter, "{source} page key {key:?} is reserved by Ferric")
            }
            Self::DuplicatePageKey { source, key } => {
                write!(
                    formatter,
                    "{source} page key {:?} is duplicated",
                    key.as_str()
                )
            }
            Self::TooManyPageKeys {
                source,
                count,
                maximum,
            } => write!(
                formatter,
                "{source} page key list has {count} entries; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for MixedPolicyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserKeySequenceState {
    Idle,
    ActiveCount,
    ActiveKeychain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOwner {
    Browser,
    Page,
}

/// Why a single logical token was assigned to its owner. Source fields make
/// later `binding-explain` output traceable to the effective settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedKeyDecisionReason {
    ExplicitMode,
    ReservedControl,
    ActiveBrowserCount,
    ActiveBrowserKeychain,
    BrowserFirstPolicy,
    ExactPageKey,
    NoPageKeyMapping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedKeyDecision {
    pub owner: InputOwner,
    pub reason: MixedKeyDecisionReason,
    pub shortcut_policy_source: Option<MixedPolicySource>,
    pub page_keys_source: Option<MixedPolicySource>,
}

impl EffectiveMixedKeyPolicy {
    /// Classifies a new logical gesture before the ordinary Normal-mode
    /// binding resolver. Native ownership and held-gesture checks belong to the
    /// caller and must precede this function.
    #[must_use]
    pub fn decide(
        &self,
        mode: Mode,
        key: &LogicalKeyToken,
        sequence: BrowserKeySequenceState,
    ) -> MixedKeyDecision {
        let reason = if mode != Mode::Normal {
            MixedKeyDecisionReason::ExplicitMode
        } else if is_reserved_page_key(key.as_str()) {
            MixedKeyDecisionReason::ReservedControl
        } else if sequence == BrowserKeySequenceState::ActiveCount {
            MixedKeyDecisionReason::ActiveBrowserCount
        } else if sequence == BrowserKeySequenceState::ActiveKeychain {
            MixedKeyDecisionReason::ActiveBrowserKeychain
        } else if self.shortcut_policy == ShortcutPolicy::BrowserFirst {
            MixedKeyDecisionReason::BrowserFirstPolicy
        } else if self.page_keys.contains(key) {
            MixedKeyDecisionReason::ExactPageKey
        } else {
            MixedKeyDecisionReason::NoPageKeyMapping
        };
        let owner = if reason == MixedKeyDecisionReason::ExactPageKey {
            InputOwner::Page
        } else {
            InputOwner::Browser
        };
        MixedKeyDecision {
            owner,
            reason,
            shortcut_policy_source: self.shortcut_policy_source,
            page_keys_source: self.page_keys_source,
        }
    }
}

fn is_reserved_page_key(value: &str) -> bool {
    matches!(
        value,
        ":" | "Escape" | "Ctrl+Escape" | "Ctrl+Shift+Escape" | "Ctrl+Shift+v"
    )
}

fn is_modifier_only(value: &str) -> bool {
    if matches!(value, "Ctrl" | "Alt" | "Shift" | "Meta") {
        return true;
    }
    let trimmed = value.trim_end_matches('+');
    !trimmed.is_empty()
        && value.contains('+')
        && trimmed
            .split('+')
            .all(|part| matches!(part, "Ctrl" | "Alt" | "Shift" | "Meta"))
}

fn is_named_logical_token(value: &str) -> bool {
    matches!(
        value,
        "Alt+m"
            | "Escape"
            | "Ctrl+Escape"
            | "Ctrl+Shift+Escape"
            | "Ctrl+Shift+v"
            | "Ctrl+Alt+p"
            | "Ctrl+Space"
            | "Ctrl+Shift+Space"
            | "Ctrl+^"
            | "Ctrl+Shift+n"
            | "Ctrl+Shift+t"
            | "Ctrl+Shift+w"
            | "Ctrl+b"
            | "Ctrl+d"
            | "Ctrl+f"
            | "Ctrl+F5"
            | "Ctrl+n"
            | "Ctrl+u"
            | "Ctrl+p"
            | "Ctrl+q"
            | "Ctrl+s"
            | "Ctrl+Tab"
            | "Ctrl+t"
            | "Ctrl+v"
            | "Ctrl+w"
            | "Ctrl+PgDown"
            | "Ctrl+PgUp"
            | "F5"
            | "F11"
            | "Back"
            | "Forward"
    ) || matches!(
        value,
        "Alt+1" | "Alt+2" | "Alt+3" | "Alt+4" | "Alt+5" | "Alt+6" | "Alt+7" | "Alt+8" | "Alt+9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{BindingOutcome, BindingRegistry, Mode};

    fn layer(policy: Option<&str>, keys: Option<&[&str]>) -> MixedPolicyLayer {
        MixedPolicyLayer::new(
            policy,
            keys.map(|keys| keys.iter().map(|key| (*key).to_owned()).collect()),
        )
    }

    fn policy(keys: &[&str]) -> EffectiveMixedKeyPolicy {
        let mixed = layer(Some("mixed"), Some(keys));
        resolve_mixed_key_policy(None, Some(&mixed), None, None).unwrap()
    }

    fn token(key: &str) -> LogicalKeyToken {
        LogicalKeyToken::parse(key).unwrap()
    }

    #[test]
    fn logical_tokens_preserve_existing_case_and_modifier_spellings() {
        for key in [
            "g",
            "H",
            " ",
            ":",
            "/",
            "?",
            "Alt+3",
            "Alt+m",
            "Ctrl+Alt+p",
            "Ctrl+Space",
            "Ctrl+Shift+Space",
            "Ctrl+^",
            "Ctrl+Shift+n",
            "Ctrl+Shift+t",
            "Ctrl+Shift+v",
            "Ctrl+Shift+w",
            "Ctrl+b",
            "Ctrl+d",
            "Ctrl+f",
            "Ctrl+F5",
            "Ctrl+n",
            "Ctrl+u",
            "Ctrl+p",
            "Ctrl+q",
            "Ctrl+s",
            "Ctrl+Tab",
            "Ctrl+t",
            "Ctrl+v",
            "Ctrl+w",
            "Ctrl+PgDown",
            "Ctrl+PgUp",
            "F5",
            "F11",
            "Back",
            "Forward",
            "😀",
            "e\u{301}",
        ] {
            assert_eq!(LogicalKeyToken::parse(key).unwrap().as_str(), key);
        }
        assert_ne!(token("h"), token("H"));
        assert!(matches!(
            LogicalKeyToken::parse("AltGr+p"),
            Err(LogicalKeyTokenError::UnknownLogicalToken)
        ));
        assert!(matches!(
            LogicalKeyToken::parse("Ctrl+AltGr+p"),
            Err(LogicalKeyTokenError::UnknownLogicalToken)
        ));
    }

    #[test]
    fn logical_token_parser_rejects_sequences_modifiers_controls_and_unknown_names() {
        for key in ["gg", "g g", "Left", "AltGr+p"] {
            let result = LogicalKeyToken::parse(key);
            assert!(result.is_err(), "unexpectedly accepted {key:?}");
        }
        assert_eq!(
            LogicalKeyToken::parse("gg"),
            Err(LogicalKeyTokenError::MultiKeySequence)
        );
        assert_eq!(
            LogicalKeyToken::parse("Ctrl+Shift+"),
            Err(LogicalKeyTokenError::ModifierOnly)
        );
        assert_eq!(
            LogicalKeyToken::parse("Ctrl"),
            Err(LogicalKeyTokenError::ModifierOnly)
        );
        assert_eq!(
            LogicalKeyToken::parse("a\u{7f}"),
            Err(LogicalKeyTokenError::ControlCharacter)
        );
        assert_eq!(
            LogicalKeyToken::parse("abcde"),
            Err(LogicalKeyTokenError::TextTooLong)
        );
    }

    #[test]
    fn reserved_controls_are_rejected_with_their_layer_provenance() {
        for key in [
            ":",
            "Escape",
            "Ctrl+Escape",
            "Ctrl+Shift+Escape",
            "Ctrl+Shift+v",
        ] {
            let global = layer(Some("mixed"), Some(&[key]));
            assert_eq!(
                resolve_mixed_key_policy(Some(&global), None, None, None),
                Err(MixedPolicyError::ReservedPageKey {
                    source: MixedPolicySource::Global,
                    key: key.to_owned(),
                })
            );
        }
    }

    #[test]
    fn layer_resolution_inherits_fields_and_replaces_lists_in_fixed_order() {
        let global = layer(Some("mixed"), Some(&["g", "f"]));
        let profile = layer(Some("browser-first"), None);
        let site = layer(None, Some(&["f", "?"]));
        let runtime = layer(Some("mixed"), Some(&[]));
        let effective =
            resolve_mixed_key_policy(Some(&global), Some(&profile), Some(&site), Some(&runtime))
                .unwrap();
        assert_eq!(effective.shortcut_policy, ShortcutPolicy::Mixed);
        assert_eq!(
            effective.shortcut_policy_source,
            Some(MixedPolicySource::Runtime)
        );
        assert!(effective.page_keys.is_empty());
        assert_eq!(effective.page_keys_source, Some(MixedPolicySource::Runtime));

        let inherited =
            resolve_mixed_key_policy(Some(&global), Some(&profile), None, None).unwrap();
        assert_eq!(inherited.shortcut_policy, ShortcutPolicy::BrowserFirst);
        assert_eq!(
            inherited.shortcut_policy_source,
            Some(MixedPolicySource::Profile)
        );
        assert_eq!(
            inherited.page_keys,
            [token("f"), token("g")].into_iter().collect()
        );
        assert_eq!(inherited.page_keys_source, Some(MixedPolicySource::Global));
    }

    #[test]
    fn layer_resolution_rejects_duplicates_limits_and_unknown_policy_atomically() {
        let duplicate = layer(Some("mixed"), Some(&["g", "g"]));
        assert!(matches!(
            resolve_mixed_key_policy(Some(&duplicate), None, None, None),
            Err(MixedPolicyError::DuplicatePageKey {
                source: MixedPolicySource::Global,
                ..
            })
        ));

        let keys = (0..=MAX_MIXED_PAGE_KEYS)
            .map(|index| {
                char::from_u32(0x100 + u32::try_from(index).unwrap())
                    .unwrap()
                    .to_string()
            })
            .collect();
        let oversized = MixedPolicyLayer {
            shortcut_policy: Some("mixed".into()),
            page_keys: Some(keys),
        };
        assert!(matches!(
            resolve_mixed_key_policy(None, None, None, Some(&oversized)),
            Err(MixedPolicyError::TooManyPageKeys {
                source: MixedPolicySource::Runtime,
                count: 65,
                maximum: 64,
            })
        ));

        let invalid_base = layer(Some("page-first"), Some(&["g"]));
        let replacing_runtime = layer(Some("mixed"), Some(&[]));
        assert_eq!(
            resolve_mixed_key_policy(None, Some(&invalid_base), None, Some(&replacing_runtime)),
            Err(MixedPolicyError::UnknownShortcutPolicy {
                source: MixedPolicySource::Profile,
                value: "page-first".into(),
            })
        );

        let invalid_list = layer(Some("mixed"), Some(&["gg"]));
        assert!(matches!(
            resolve_mixed_key_policy(Some(&invalid_list), None, None, Some(&replacing_runtime)),
            Err(MixedPolicyError::InvalidPageKey {
                source: MixedPolicySource::Global,
                reason: LogicalKeyTokenError::MultiKeySequence,
                ..
            })
        ));
    }

    #[test]
    fn first_key_decision_preserves_policy_mode_reservation_and_sequence_order() {
        let mixed = policy(&["g", "2", "q", "Ctrl+f"]);
        for key in ["g", "2"] {
            let decision = mixed.decide(Mode::Normal, &token(key), BrowserKeySequenceState::Idle);
            assert_eq!(decision.owner, InputOwner::Page);
            assert_eq!(decision.reason, MixedKeyDecisionReason::ExactPageKey);
            assert_eq!(decision.page_keys_source, Some(MixedPolicySource::Profile));
        }
        let reserved = mixed.decide(
            Mode::Normal,
            &token("Ctrl+Shift+v"),
            BrowserKeySequenceState::Idle,
        );
        assert_eq!(reserved.owner, InputOwner::Browser);
        assert_eq!(reserved.reason, MixedKeyDecisionReason::ReservedControl);

        let explicit_mode = mixed.decide(Mode::Insert, &token("g"), BrowserKeySequenceState::Idle);
        assert_eq!(explicit_mode.owner, InputOwner::Browser);
        assert_eq!(explicit_mode.reason, MixedKeyDecisionReason::ExplicitMode);

        for (state, reason) in [
            (
                BrowserKeySequenceState::ActiveCount,
                MixedKeyDecisionReason::ActiveBrowserCount,
            ),
            (
                BrowserKeySequenceState::ActiveKeychain,
                MixedKeyDecisionReason::ActiveBrowserKeychain,
            ),
        ] {
            let decision = mixed.decide(Mode::Normal, &token("q"), state);
            assert_eq!(decision.owner, InputOwner::Browser);
            assert_eq!(decision.reason, reason);
        }

        let browser_first = EffectiveMixedKeyPolicy::default();
        let decision =
            browser_first.decide(Mode::Normal, &token("g"), BrowserKeySequenceState::Idle);
        assert_eq!(decision.owner, InputOwner::Browser);
        assert_eq!(decision.reason, MixedKeyDecisionReason::BrowserFirstPolicy);
    }

    #[test]
    fn browser_prefix_mismatch_and_timeout_never_replay_to_a_mapped_page_key() {
        let mixed = policy(&["q"]);
        let bindings = BindingRegistry::default_v1(crate::CommandRegistry::default_v1()).unwrap();
        let mut resolver = bindings.resolver(Mode::Normal);

        let prefix_decision =
            mixed.decide(Mode::Normal, &token("g"), BrowserKeySequenceState::Idle);
        assert_eq!(prefix_decision.owner, InputOwner::Browser);
        assert!(matches!(
            resolver.feed("g", 0),
            BindingOutcome::Pending { .. }
        ));

        let suffix_decision = mixed.decide(
            Mode::Normal,
            &token("q"),
            BrowserKeySequenceState::ActiveKeychain,
        );
        assert_eq!(suffix_decision.owner, InputOwner::Browser);
        assert_eq!(
            suffix_decision.reason,
            MixedKeyDecisionReason::ActiveBrowserKeychain
        );
        assert_eq!(resolver.feed("q", 1), BindingOutcome::Consumed);
        assert!(!resolver.is_pending());

        let later_fresh_key =
            mixed.decide(Mode::Normal, &token("q"), BrowserKeySequenceState::Idle);
        assert_eq!(later_fresh_key.owner, InputOwner::Page);

        assert!(matches!(
            resolver.feed("g", 10),
            BindingOutcome::Pending { .. }
        ));
        assert_eq!(resolver.tick(1_010), None);
        assert!(!resolver.is_pending());
        let after_timeout = mixed.decide(Mode::Normal, &token("q"), BrowserKeySequenceState::Idle);
        assert_eq!(after_timeout.owner, InputOwner::Page);

        assert!(matches!(
            resolver.feed("g", 2_000),
            BindingOutcome::Pending { .. }
        ));
        resolver.reset();
        assert!(!resolver.is_pending());
        let after_configuration_change =
            mixed.decide(Mode::Normal, &token("q"), BrowserKeySequenceState::Idle);
        assert_eq!(after_configuration_change.owner, InputOwner::Page);
    }

    #[test]
    fn mapped_first_digit_beats_count_start_but_not_an_active_count() {
        let mixed = policy(&["2"]);
        let mapped_first = mixed.decide(Mode::Normal, &token("2"), BrowserKeySequenceState::Idle);
        assert_eq!(mapped_first.owner, InputOwner::Page);

        let bindings = BindingRegistry::default_v1(crate::CommandRegistry::default_v1()).unwrap();
        let mut resolver = bindings.resolver(Mode::Normal);
        let first_count_digit =
            mixed.decide(Mode::Normal, &token("1"), BrowserKeySequenceState::Idle);
        assert_eq!(first_count_digit.owner, InputOwner::Browser);
        assert!(matches!(
            resolver.feed("1", 0),
            BindingOutcome::Pending { count: 1, .. }
        ));

        let second_digit = mixed.decide(
            Mode::Normal,
            &token("2"),
            BrowserKeySequenceState::ActiveCount,
        );
        assert_eq!(second_digit.owner, InputOwner::Browser);
        assert_eq!(
            second_digit.reason,
            MixedKeyDecisionReason::ActiveBrowserCount
        );
        assert!(matches!(
            resolver.feed("2", 1),
            BindingOutcome::Pending { count: 12, .. }
        ));
    }
}
