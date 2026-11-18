//! Cardinalities for the planned Site Policies draft/capture service. Lifecycle
//! enforcement belongs to P11/P12, using the existing configuration publisher.
//! No draft registry, preview or capture authority is created by these values.

pub const MAX_SITE_POLICY_DRAFTS_PER_PROFILE: usize = 16;
pub const MAX_SITE_POLICY_PREVIEWS: usize = 8;
pub const MAX_SITE_POLICY_CAPTURES_PER_WINDOW: usize = 1;
