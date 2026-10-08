use std::fmt;

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKey {
    Structural,
    LiveRetention,
    ArchiveCapture,
    ArchiveOffline,
    SemanticRestore,
    CleanBranch,
    CookieIdentity,
    CookieSnapshot,
    CookieSeedVerify,
    CookieMergeApply,
}

impl CapabilityKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::LiveRetention => "live_retention",
            Self::ArchiveCapture => "archive_capture",
            Self::ArchiveOffline => "archive_offline",
            Self::SemanticRestore => "semantic_restore",
            Self::CleanBranch => "clean_branch",
            Self::CookieIdentity => "cookie_identity",
            Self::CookieSnapshot => "cookie_snapshot",
            Self::CookieSeedVerify => "cookie_seed_verify",
            Self::CookieMergeApply => "cookie_merge_apply",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveGate {
    Supported,
    Unsupported,
    Unqualified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "release_evidence", rename_all = "kebab-case")]
pub enum ProductionGate {
    Disabled,
    Qualified(EvidenceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityFactError {
    InvalidToken,
    MissingSupportEvidence,
    ProductionWithoutSupport,
    ReusedReleaseEvidence,
}

impl fmt::Display for CapabilityFactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidToken => "invalid capability token",
            Self::MissingSupportEvidence => {
                "supported capability requires scope and probe evidence"
            }
            Self::ProductionWithoutSupport => {
                "production qualification requires a supported primitive"
            }
            Self::ReusedReleaseEvidence => {
                "release evidence must be distinct from primitive evidence"
            }
        })
    }
}

impl std::error::Error for CapabilityFactError {}

fn safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

macro_rules! token_type {
    ($name:ident, $max:expr) => {
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        pub struct $name(String);

        impl TryFrom<&str> for $name {
            type Error = CapabilityFactError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                safe_token(value, $max)
                    .then(|| Self(value.to_owned()))
                    .ok_or(CapabilityFactError::InvalidToken)
            }
        }

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

token_type!(EvidenceId, 64);
token_type!(CapabilityScope, 128);
token_type!(SafeReasonCode, 64);

/// A bounded, printable runtime fingerprint. No URL or free-form diagnostic is accepted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityFingerprint {
    qt: String,
    qt_webengine: String,
    chromium_base: String,
    security_patch: String,
    package: String,
}

impl CapabilityFingerprint {
    /// # Errors
    /// Rejects empty, unsafe, or unknown-sentinel version tokens.
    pub fn new(
        qt: &str,
        qt_webengine: &str,
        chromium_base: &str,
        security_patch: &str,
        package: &str,
    ) -> Result<Self, CapabilityFactError> {
        for value in [qt, qt_webengine, chromium_base, security_patch, package] {
            if !valid_fingerprint_component(value) {
                return Err(CapabilityFactError::InvalidToken);
            }
        }
        Ok(Self {
            qt: qt.into(),
            qt_webengine: qt_webengine.into(),
            chromium_base: chromium_base.into(),
            security_patch: security_patch.into(),
            package: package.into(),
        })
    }
}

fn valid_fingerprint_component(value: &str) -> bool {
    const UNKNOWN_SENTINELS: [&str; 7] = [
        "unknown",
        "unavailable",
        "unreported",
        "missing",
        "unset",
        "none",
        "null",
    ];

    safe_token(value, 64)
        && !UNKNOWN_SENTINELS
            .iter()
            .any(|sentinel| value.eq_ignore_ascii_case(sentinel))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityFact {
    key: CapabilityKey,
    primitive: PrimitiveGate,
    production: ProductionGate,
    scope: Option<CapabilityScope>,
    evidence: Vec<EvidenceId>,
    fingerprint: CapabilityFingerprint,
    reason: SafeReasonCode,
}

impl CapabilityFact {
    /// # Errors
    /// Supported primitives need bounded scope/probe evidence; release qualification
    /// additionally needs distinct evidence and cannot contradict the primitive gate.
    pub fn new(
        key: CapabilityKey,
        primitive: PrimitiveGate,
        production: ProductionGate,
        scope: Option<CapabilityScope>,
        evidence: Vec<EvidenceId>,
        fingerprint: CapabilityFingerprint,
        reason: SafeReasonCode,
    ) -> Result<Self, CapabilityFactError> {
        if primitive == PrimitiveGate::Supported && (scope.is_none() || evidence.is_empty()) {
            return Err(CapabilityFactError::MissingSupportEvidence);
        }
        if let ProductionGate::Qualified(release_evidence) = &production {
            if primitive != PrimitiveGate::Supported {
                return Err(CapabilityFactError::ProductionWithoutSupport);
            }
            if evidence.contains(release_evidence) {
                return Err(CapabilityFactError::ReusedReleaseEvidence);
            }
        }
        Ok(Self {
            key,
            primitive,
            production,
            scope,
            evidence,
            fingerprint,
            reason,
        })
    }

    #[must_use]
    pub const fn key(&self) -> CapabilityKey {
        self.key
    }
    #[must_use]
    pub const fn primitive(&self) -> PrimitiveGate {
        self.primitive
    }
    #[must_use]
    pub const fn production(&self) -> &ProductionGate {
        &self.production
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityRequirement {
    Structural,
    LiveRetention,
    ArchiveCapture,
    ArchiveOffline,
    SemanticRestore,
    CleanBranch,
    SnapshotBranch,
    CookieMergeApply,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequirementAvailability {
    Available,
    Missing(CapabilityKey),
    Unsupported(CapabilityKey),
    Unqualified(CapabilityKey),
    ProductionDisabled(CapabilityKey),
}

impl CapabilityRequirement {
    #[must_use]
    pub const fn primitives(self) -> &'static [CapabilityKey] {
        match self {
            Self::Structural => &[CapabilityKey::Structural],
            Self::LiveRetention => &[CapabilityKey::LiveRetention],
            Self::ArchiveCapture => &[CapabilityKey::ArchiveCapture],
            Self::ArchiveOffline => &[CapabilityKey::ArchiveCapture, CapabilityKey::ArchiveOffline],
            Self::SemanticRestore => &[CapabilityKey::SemanticRestore],
            Self::CleanBranch => &[CapabilityKey::CleanBranch],
            Self::SnapshotBranch => &[
                CapabilityKey::CleanBranch,
                CapabilityKey::CookieIdentity,
                CapabilityKey::CookieSnapshot,
                CapabilityKey::CookieSeedVerify,
            ],
            Self::CookieMergeApply => &[CapabilityKey::CookieMergeApply],
        }
    }

    /// Checks the required facts only when every certificate was recorded for
    /// the exact runtime that is active now. A version mismatch is
    /// unqualified; it never inherits support from a nearby Qt/Chromium build.
    #[must_use]
    pub fn check_for_fingerprint(
        self,
        facts: &[CapabilityFact],
        production: bool,
        active_fingerprint: &CapabilityFingerprint,
    ) -> RequirementAvailability {
        for key in self.primitives() {
            let mut matching = facts.iter().filter(|fact| fact.key == *key);
            let Some(fact) = matching.next() else {
                return RequirementAvailability::Missing(*key);
            };
            // Conflicting evidence remains unqualified regardless of input order.
            if matching.next().is_some() || &fact.fingerprint != active_fingerprint {
                return RequirementAvailability::Unqualified(*key);
            }
            match fact.primitive {
                PrimitiveGate::Supported => {}
                PrimitiveGate::Unsupported => {
                    return RequirementAvailability::Unsupported(*key);
                }
                PrimitiveGate::Unqualified => {
                    return RequirementAvailability::Unqualified(*key);
                }
            }
            if production && matches!(fact.production, ProductionGate::Disabled) {
                return RequirementAvailability::ProductionDisabled(*key);
            }
        }
        RequirementAvailability::Available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> CapabilityFingerprint {
        CapabilityFingerprint::new(
            "6.11.2",
            "6.11.2",
            "140.0.7339.225",
            "151.0.7922.71",
            "6.11.2-1",
        )
        .unwrap()
    }

    fn fact(key: CapabilityKey, gate: PrimitiveGate, production: ProductionGate) -> CapabilityFact {
        CapabilityFact::new(
            key,
            gate,
            production,
            Some(CapabilityScope::try_from("synthetic-scope").unwrap()),
            vec![EvidenceId::try_from("VB-005").unwrap()],
            fingerprint(),
            SafeReasonCode::try_from("fixture-probe").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn snapshot_request_cannot_fall_back_to_clean_branch() {
        let facts = [
            fact(
                CapabilityKey::CleanBranch,
                PrimitiveGate::Supported,
                ProductionGate::Disabled,
            ),
            fact(
                CapabilityKey::CookieIdentity,
                PrimitiveGate::Unsupported,
                ProductionGate::Disabled,
            ),
            fact(
                CapabilityKey::CookieSnapshot,
                PrimitiveGate::Unsupported,
                ProductionGate::Disabled,
            ),
            fact(
                CapabilityKey::CookieSeedVerify,
                PrimitiveGate::Unsupported,
                ProductionGate::Disabled,
            ),
        ];
        assert_eq!(
            CapabilityRequirement::SnapshotBranch.check_for_fingerprint(
                &facts,
                false,
                &fingerprint(),
            ),
            RequirementAvailability::Unsupported(CapabilityKey::CookieIdentity)
        );
        assert_eq!(
            CapabilityRequirement::CleanBranch.check_for_fingerprint(&facts, true, &fingerprint(),),
            RequirementAvailability::ProductionDisabled(CapabilityKey::CleanBranch)
        );
    }

    #[test]
    fn support_and_production_require_distinct_evidence() {
        assert!(
            CapabilityFact::new(
                CapabilityKey::ArchiveCapture,
                PrimitiveGate::Supported,
                ProductionGate::Disabled,
                None,
                vec![EvidenceId::try_from("VB-004").unwrap()],
                fingerprint(),
                SafeReasonCode::try_from("probe").unwrap(),
            )
            .is_err()
        );
        assert!(
            CapabilityFact::new(
                CapabilityKey::CookieSnapshot,
                PrimitiveGate::Unsupported,
                ProductionGate::Qualified(EvidenceId::try_from("VB-094").unwrap()),
                Some(CapabilityScope::try_from("general").unwrap()),
                vec![EvidenceId::try_from("VB-005").unwrap()],
                fingerprint(),
                SafeReasonCode::try_from("gap").unwrap(),
            )
            .is_err()
        );
        assert!(EvidenceId::try_from("https://example.invalid/secret").is_err());
        assert!(SafeReasonCode::try_from("cookie=secret").is_err());
    }

    #[test]
    fn unknown_runtime_fingerprint_components_fail_closed() {
        for sentinel in [
            "unknown",
            "UNAVAILABLE",
            "unreported",
            "missing",
            "unset",
            "none",
            "null",
        ] {
            assert!(
                CapabilityFingerprint::new(
                    "6.11.2",
                    "6.11.2",
                    "140.0.7339.225",
                    "151.0.7922.71",
                    sentinel,
                )
                .is_err(),
                "accepted unknown package sentinel {sentinel:?}"
            );
        }
    }

    #[test]
    fn duplicate_capability_facts_fail_closed() {
        let facts = [
            fact(
                CapabilityKey::CleanBranch,
                PrimitiveGate::Supported,
                ProductionGate::Disabled,
            ),
            fact(
                CapabilityKey::CleanBranch,
                PrimitiveGate::Unsupported,
                ProductionGate::Disabled,
            ),
        ];
        assert_eq!(
            CapabilityRequirement::CleanBranch
                .check_for_fingerprint(&facts, false, &fingerprint(),),
            RequirementAvailability::Unqualified(CapabilityKey::CleanBranch),
        );
    }

    #[test]
    fn runtime_fingerprint_mismatch_invalidates_the_capability_fact() {
        let facts = [fact(
            CapabilityKey::ArchiveCapture,
            PrimitiveGate::Supported,
            ProductionGate::Disabled,
        )];
        let changed_runtime = CapabilityFingerprint::new(
            "6.11.3",
            "6.11.2",
            "140.0.7339.225",
            "151.0.7922.71",
            "6.11.2-1",
        )
        .unwrap();

        assert_eq!(
            CapabilityRequirement::ArchiveCapture.check_for_fingerprint(
                &facts,
                false,
                &changed_runtime
            ),
            RequirementAvailability::Unqualified(CapabilityKey::ArchiveCapture)
        );
        assert_eq!(
            CapabilityRequirement::ArchiveCapture.check_for_fingerprint(
                &facts,
                false,
                &fingerprint()
            ),
            RequirementAvailability::Available,
            "support is available only when the recorded runtime is explicitly supplied"
        );
    }

    #[test]
    fn changing_any_runtime_fingerprint_component_invalidates_support() {
        let facts = [fact(
            CapabilityKey::ArchiveCapture,
            PrimitiveGate::Supported,
            ProductionGate::Disabled,
        )];
        let changed_runtimes = [
            CapabilityFingerprint::new(
                "6.11.3",
                "6.11.2",
                "140.0.7339.225",
                "151.0.7922.71",
                "6.11.2-1",
            )
            .unwrap(),
            CapabilityFingerprint::new(
                "6.11.2",
                "6.11.3",
                "140.0.7339.225",
                "151.0.7922.71",
                "6.11.2-1",
            )
            .unwrap(),
            CapabilityFingerprint::new(
                "6.11.2",
                "6.11.2",
                "140.0.7339.226",
                "151.0.7922.71",
                "6.11.2-1",
            )
            .unwrap(),
            CapabilityFingerprint::new(
                "6.11.2",
                "6.11.2",
                "140.0.7339.225",
                "151.0.7922.72",
                "6.11.2-1",
            )
            .unwrap(),
            CapabilityFingerprint::new(
                "6.11.2",
                "6.11.2",
                "140.0.7339.225",
                "151.0.7922.71",
                "6.11.2-2",
            )
            .unwrap(),
        ];

        for active_fingerprint in &changed_runtimes {
            assert_eq!(
                CapabilityRequirement::ArchiveCapture.check_for_fingerprint(
                    &facts,
                    false,
                    active_fingerprint,
                ),
                RequirementAvailability::Unqualified(CapabilityKey::ArchiveCapture),
            );
        }
    }

    #[test]
    fn changed_runtime_does_not_reuse_a_negative_capability_probe() {
        let facts = [fact(
            CapabilityKey::CookieMergeApply,
            PrimitiveGate::Unsupported,
            ProductionGate::Disabled,
        )];
        let changed_runtime = CapabilityFingerprint::new(
            "6.11.2",
            "6.11.2",
            "140.0.7339.226",
            "151.0.7922.71",
            "6.11.2-1",
        )
        .unwrap();

        assert_eq!(
            CapabilityRequirement::CookieMergeApply.check_for_fingerprint(
                &facts,
                false,
                &changed_runtime,
            ),
            RequirementAvailability::Unqualified(CapabilityKey::CookieMergeApply),
            "an unsupported result is tied to its recorded runtime too"
        );
        assert_eq!(
            CapabilityRequirement::CookieMergeApply.check_for_fingerprint(
                &facts,
                false,
                &fingerprint(),
            ),
            RequirementAvailability::Unsupported(CapabilityKey::CookieMergeApply)
        );
    }
}
