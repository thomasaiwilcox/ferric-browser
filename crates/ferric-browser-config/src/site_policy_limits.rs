//! Local interchange bounds. Parsing and atomic reviewed-batch publication are
//! implemented by their service phases; constants alone grant no authority.

pub const MAX_SITE_POLICY_INTERCHANGE_BYTES: u64 = crate::MAX_CONFIGURATION_BYTES;
pub const MAX_SITE_POLICY_IMPORT_OPERATIONS: usize = crate::MAX_SITE_RULES;
