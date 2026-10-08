use std::{fmt, str::FromStr};

use serde::Serialize;
use uuid::{Uuid, Variant, Version};

/// Why a textual or storage UUID cannot become a versioning identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningIdError {
    WrongKind,
    NoncanonicalUuid,
    NotRandomV4,
}

impl fmt::Display for VersioningIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind => formatter.write_str("versioning ID kind does not match"),
            Self::NoncanonicalUuid => formatter.write_str("versioning ID UUID is not canonical"),
            Self::NotRandomV4 => {
                formatter.write_str("versioning ID must be an RFC variant UUID v4")
            }
        }
    }
}

impl std::error::Error for VersioningIdError {}

macro_rules! typed_uuid_id {
    ($name:ident, $kind:literal, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(Uuid);

        impl $name {
            /// Allocates a fresh random UUID v4 for this identity kind.
            #[must_use]
            pub fn fresh() -> Self {
                Self(Uuid::new_v4())
            }

            /// Returns the UUID for a validated storage DTO boundary. The UUID
            /// alone is a reference, not authority over a live engine object.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl TryFrom<Uuid> for $name {
            type Error = VersioningIdError;

            fn try_from(uuid: Uuid) -> Result<Self, Self::Error> {
                if uuid.get_variant() != Variant::RFC4122
                    || uuid.get_version() != Some(Version::Random)
                {
                    return Err(VersioningIdError::NotRandomV4);
                }
                Ok(Self(uuid))
            }
        }

        impl FromStr for $name {
            type Err = VersioningIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() > 64 {
                    return Err(VersioningIdError::NoncanonicalUuid);
                }
                let (kind, encoded) = value
                    .split_once(':')
                    .ok_or(VersioningIdError::NoncanonicalUuid)?;
                if kind != $kind {
                    return Err(VersioningIdError::WrongKind);
                }
                if encoded.len() != 36 {
                    return Err(VersioningIdError::NoncanonicalUuid);
                }
                let uuid =
                    Uuid::parse_str(encoded).map_err(|_| VersioningIdError::NoncanonicalUuid)?;
                if uuid.hyphenated().to_string() != encoded {
                    return Err(VersioningIdError::NoncanonicalUuid);
                }
                Self::try_from(uuid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}:{}", $kind, self.0.hyphenated())
            }
        }
    };
}

typed_uuid_id!(
    CommitId,
    "commit",
    "Immutable, durable explicit commit identity."
);
typed_uuid_id!(
    BranchId,
    "branch",
    "Branch lineage identity, separate from its live engine profile."
);
typed_uuid_id!(
    StashId,
    "stash",
    "Explicit stash identity, separate from a retained view lease."
);
typed_uuid_id!(
    LogicalPageId,
    "logical-page",
    "Durable conceptual page slot across commits. A child branch can preserve this lineage while allocating a fresh runtime `TabId`; a deliberate duplicate gets a new logical page ID."
);
typed_uuid_id!(
    LogicalWindowId,
    "logical-window",
    "Durable conceptual window slot across captures, independent of a runtime `WindowId`."
);
typed_uuid_id!(
    RetainedViewId,
    "retained-view",
    "Runtime lease identity for a retained native view."
);
typed_uuid_id!(
    OperationId,
    "operation",
    "One versioning operation; never a permission to retarget callbacks."
);
typed_uuid_id!(
    ProfileInstanceId,
    "profile-instance",
    "Random identity for one actual engine-profile lifetime; never sufficient authority after restart."
);
typed_uuid_id!(
    ApplicationOwnerId,
    "application-owner",
    "Random identity for one application owner, distinct from window and profile IDs."
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use uuid::Uuid;

    #[test]
    fn every_id_has_canonical_typed_v4_round_trip() {
        macro_rules! check_kind {
            ($kind:ty) => {{
                let id = <$kind>::fresh();
                let text = id.to_string();
                assert_eq!(text.parse::<$kind>(), Ok(id));
                text
            }};
        }
        let ids = [
            check_kind!(CommitId),
            check_kind!(BranchId),
            check_kind!(StashId),
            check_kind!(LogicalPageId),
            check_kind!(LogicalWindowId),
            check_kind!(RetainedViewId),
            check_kind!(OperationId),
            check_kind!(ProfileInstanceId),
            check_kind!(ApplicationOwnerId),
        ];
        for id in ids {
            assert_eq!(id.len(), id.split_once(':').unwrap().0.len() + 37);
            assert_eq!(
                Uuid::parse_str(id.split_once(':').unwrap().1)
                    .unwrap()
                    .get_version(),
                Some(uuid::Version::Random)
            );
        }
        let commit = CommitId::fresh();
        assert_ne!(commit, CommitId::fresh());
    }

    #[test]
    fn parser_rejects_ambiguous_or_non_v4_ids() {
        let commit = CommitId::fresh();
        let text = commit.to_string();
        assert!(text.parse::<BranchId>().is_err());
        assert!(text.to_uppercase().parse::<CommitId>().is_err());
        assert!(text.replace('-', "").parse::<CommitId>().is_err());
        assert!(
            "commit:00000000-0000-0000-0000-000000000000"
                .parse::<CommitId>()
                .is_err()
        );
        assert!(
            "commit:018f3950-ec63-7b13-9d50-666c96f46cbf"
                .parse::<CommitId>()
                .is_err()
        );
        assert!(CommitId::try_from(Uuid::nil()).is_err());
        assert!(
            format!("commit:{}", "a".repeat(1_000))
                .parse::<CommitId>()
                .is_err()
        );
    }

    #[test]
    fn equality_and_hash_use_the_full_uuid_without_type_erasure() {
        let uuid = Uuid::new_v4();
        let first = LogicalPageId::try_from(uuid).unwrap();
        let duplicate = LogicalPageId::try_from(uuid).unwrap();
        let other = LogicalPageId::fresh();
        let ids = HashSet::from([first, duplicate, other]);
        assert_eq!(ids.len(), 2);
        assert_eq!(first.as_uuid(), uuid);
        assert_ne!(
            first.to_string(),
            BranchId::try_from(uuid).unwrap().to_string()
        );
    }

    #[test]
    fn runtime_profile_instance_is_not_a_durable_profile_or_application_owner() {
        let instance = ProfileInstanceId::fresh();
        let owner = ApplicationOwnerId::fresh();
        assert_ne!(instance.to_string(), owner.to_string());
        assert!(instance.to_string().parse::<ApplicationOwnerId>().is_err());
        assert_ne!(
            std::any::TypeId::of::<LogicalPageId>(),
            std::any::TypeId::of::<crate::TabId>()
        );
    }
}
