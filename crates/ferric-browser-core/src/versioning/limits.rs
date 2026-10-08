use std::fmt;

// Fixed admission and collection bounds from the initial Versioned Browsing
// policy. These are not native Qt limits and can be revised only with evidence.
pub const MAX_MUTATING_OPERATIONS_PER_OWNER: usize = 1;
pub const MAX_OPERATION_QUEUE: usize = 16;
pub const MAX_PAGE_SAVES_PER_PROFILE: usize = 1;
pub const MAX_PAGE_SAVES_PER_PROCESS: usize = 2;
pub const MAX_BRANCH_NESTING: usize = 8;
pub const MAX_PAGES_PER_WINDOW_CAPTURE: usize = 128;
pub const MAX_FRAMES_PER_SIDECAR: usize = 32;
pub const MAX_FRAME_DEPTH: usize = 8;
pub const MAX_DOM_NODES_PER_FRAME: usize = 4_096;
pub const MAX_CONTROLS_PER_PAGE: usize = 256;
pub const MAX_EDITABLE_VALUE_BYTES: usize = 16 * 1024;
pub const MAX_EDITABLE_PAGE_BYTES: usize = 256 * 1024;
pub const MAX_SIDECAR_BYTES: usize = 512 * 1024;
pub const MAX_SIDECAR_DEPTH: usize = 16;
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_JOURNAL_EVENTS: usize = 10_000;
pub const MAX_COOKIE_OBSERVATIONS: usize = 4_096;
pub const MAX_COOKIE_MATERIAL_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_LIST_ITEMS: usize = 100;
pub const MAX_LIST_ITEMS: usize = 1_000;
pub const CONFIRMATION_PLAN_TTL_MS: u64 = 60_000;
pub const COLLECTOR_CALLBACK_DEADLINE_MS: u64 = 2_000;
pub const ARCHIVE_SAVE_DEADLINE_MS: u64 = 60_000;
pub const LIVE_RESTORE_READINESS_MS: u64 = 15_000;

pub const MAX_RETAINED_VIEWS: u32 = 32;
pub const MAX_BRANCH_PROFILES: u32 = 16;
pub const MAX_DISK_BUDGET_MIB: u32 = 65_536;

/// Only the initially configurable resource limits; fixed bounds stay above.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersioningLimits {
    max_retained_views: u32,
    max_branch_profiles: u32,
    disk_budget_mib: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningLimitError {
    RetainedViewsOutOfRange,
    BranchProfilesOutOfRange,
    DiskBudgetOutOfRange,
}

impl fmt::Display for VersioningLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RetainedViewsOutOfRange => {
                formatter.write_str("retained view limit is out of range")
            }
            Self::BranchProfilesOutOfRange => {
                formatter.write_str("branch profile limit is out of range")
            }
            Self::DiskBudgetOutOfRange => {
                formatter.write_str("versioning disk budget is out of range")
            }
        }
    }
}

impl std::error::Error for VersioningLimitError {}

impl VersioningLimits {
    /// Validates proposed configuration without consulting process environment.
    ///
    /// # Errors
    ///
    /// Returns a field-specific error when a value is zero or exceeds its cap.
    pub const fn new(
        max_retained_views: u32,
        max_branch_profiles: u32,
        disk_budget_mib: u32,
    ) -> Result<Self, VersioningLimitError> {
        if max_retained_views == 0 || max_retained_views > MAX_RETAINED_VIEWS {
            return Err(VersioningLimitError::RetainedViewsOutOfRange);
        }
        if max_branch_profiles == 0 || max_branch_profiles > MAX_BRANCH_PROFILES {
            return Err(VersioningLimitError::BranchProfilesOutOfRange);
        }
        if disk_budget_mib == 0 || disk_budget_mib > MAX_DISK_BUDGET_MIB {
            return Err(VersioningLimitError::DiskBudgetOutOfRange);
        }
        Ok(Self {
            max_retained_views,
            max_branch_profiles,
            disk_budget_mib,
        })
    }

    #[must_use]
    pub const fn max_retained_views(self) -> u32 {
        self.max_retained_views
    }

    #[must_use]
    pub const fn max_branch_profiles(self) -> u32 {
        self.max_branch_profiles
    }

    #[must_use]
    pub const fn disk_budget_mib(self) -> u32 {
        self.disk_budget_mib
    }

    #[must_use]
    pub fn disk_budget_bytes(self) -> u64 {
        u64::from(self.disk_budget_mib) * 1024 * 1024
    }
}

impl Default for VersioningLimits {
    fn default() -> Self {
        Self {
            max_retained_views: 8,
            max_branch_profiles: 4,
            disk_budget_mib: 2_048,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_initial_bounded_policy() {
        let limits = VersioningLimits::default();
        assert_eq!(limits.max_retained_views(), 8);
        assert_eq!(limits.max_branch_profiles(), 4);
        assert_eq!(limits.disk_budget_mib(), 2048);
        assert_eq!(MAX_OPERATION_QUEUE, 16);
        assert_eq!(MAX_PAGES_PER_WINDOW_CAPTURE, 128);
        assert_eq!(MAX_FRAMES_PER_SIDECAR, 32);
        assert_eq!(MAX_SIDECAR_BYTES, 512 * 1024);
        assert_eq!(MAX_ARCHIVE_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn configurable_limits_reject_zero_and_values_above_the_warning_caps() {
        assert!(VersioningLimits::new(0, 4, 2048).is_err());
        assert!(VersioningLimits::new(8, 0, 2048).is_err());
        assert!(VersioningLimits::new(33, 4, 2048).is_err());
        assert!(VersioningLimits::new(8, 17, 2048).is_err());
        assert!(VersioningLimits::new(8, 4, 0).is_err());
        assert!(VersioningLimits::new(8, 4, 65_537).is_err());
        assert!(VersioningLimits::new(1, 1, 1).is_ok());
        assert!(VersioningLimits::new(32, 16, 65_536).is_ok());
    }
}
