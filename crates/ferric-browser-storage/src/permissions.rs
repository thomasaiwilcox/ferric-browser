use rusqlite::params;

use crate::{ProfileStore, StoreError};

const MAX_PERMISSION_RULES: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRule {
    pub origin: String,
    pub permission: String,
    pub decision: String,
    pub expires_at: Option<i64>,
    pub updated_at: i64,
}

/// A bounded list whose completeness is established by the same SQLite read.
/// A truncated list cannot establish that an omitted site has no saved rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRulesSnapshot {
    pub rules: Vec<PermissionRule>,
    pub complete: bool,
}

/// Whether a capability may have a remembered session or durable decision.
/// Screen/window capture always requires fresh per-request consent.
#[must_use]
pub fn is_rememberable_permission(permission: &str) -> bool {
    matches!(
        permission,
        "camera" | "microphone" | "notifications" | "geolocation" | "clipboard" | "local-fonts"
    )
}

/// Normalizes an exact, persistent permission origin. Durable rules are
/// HTTPS-only, with an explicit HTTP loopback exception for development.
///
/// # Errors
///
/// Returns an error when the origin is malformed, unsafe, or not eligible for
/// a durable permission rule.
pub fn normalize_permission_origin(origin: &str) -> Result<String, StoreError> {
    if origin.is_empty() || origin.len() > 512 || origin.chars().any(char::is_control) {
        return Err(StoreError::InvalidInput("permission origin is invalid"));
    }
    // Qt exposes a security origin as a URL with a single root slash. Match
    // that representation to the same exact origin without accepting paths.
    let origin = origin.strip_suffix('/').unwrap_or(origin);
    let (scheme, authority) = origin.split_once("://").ok_or(StoreError::InvalidInput(
        "permission origin must have a scheme",
    ))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return Err(StoreError::InvalidInput(
            "permission origin must use HTTP or HTTPS",
        ));
    }
    if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
        return Err(StoreError::InvalidInput(
            "permission origin authority is invalid",
        ));
    }
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']').ok_or(StoreError::InvalidInput(
            "permission IPv6 origin is invalid",
        ))?;
        let host = &authority[1..close];
        let suffix = &authority[close + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(suffix.strip_prefix(':').ok_or(StoreError::InvalidInput(
                "permission origin port is invalid",
            ))?)
        };
        (host.to_ascii_lowercase(), port)
    } else {
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if port.chars().all(|character| character.is_ascii_digit()) => {
                (host, Some(port))
            }
            Some((_, _)) => {
                return Err(StoreError::InvalidInput(
                    "permission origin port is invalid",
                ));
            }
            None => (authority, None),
        };
        (host.to_ascii_lowercase(), port)
    };
    if host.is_empty() || host.contains(':') && !authority.starts_with('[') {
        return Err(StoreError::InvalidInput(
            "permission origin host is invalid",
        ));
    }
    if let Some(port) = port
        && (port.is_empty() || port.parse::<u16>().is_err())
    {
        return Err(StoreError::InvalidInput(
            "permission origin port is invalid",
        ));
    }
    let is_loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
    if scheme == "http" && !is_loopback {
        return Err(StoreError::InvalidInput(
            "persistent HTTP permission rules require a loopback origin",
        ));
    }
    let default_port =
        (scheme == "https" && port == Some("443")) || (scheme == "http" && port == Some("80"));
    let authority = if default_port {
        if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        }
    } else if let Some(port) = port {
        if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        }
    } else if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    Ok(format!("{scheme}://{authority}"))
}

impl ProfileStore {
    /// Lists bounded exact-origin permission rules, optionally for one origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the optional origin is invalid or SQLite fails.
    pub fn permission_rules(
        &self,
        origin: Option<&str>,
    ) -> Result<Vec<PermissionRule>, StoreError> {
        self.permission_rules_snapshot(origin)
            .map(|snapshot| snapshot.rules)
    }

    /// Reads one bounded snapshot, optionally restricted to an exact origin.
    /// Completeness and rows come from a single statement, so concurrent
    /// inserts or deletes cannot make a truncated result appear complete.
    /// # Errors
    /// Rejects an invalid origin or an unavailable database.
    pub fn permission_rules_snapshot(
        &self,
        origin: Option<&str>,
    ) -> Result<PermissionRulesSnapshot, StoreError> {
        let origin = origin.map(normalize_permission_origin).transpose()?;
        let limit = i64::try_from(MAX_PERMISSION_RULES + 1).unwrap_or(i64::MAX);
        // Keep the point read indexable. An optional-origin OR predicate makes
        // SQLite scan the entire table even when a single site was requested.
        let filter = if origin.is_some() {
            "origin = ?1"
        } else {
            "?1 IS NULL"
        };
        let sql = format!(
            "SELECT origin, permission, decision, expires_at, updated_at
             FROM permission_rules WHERE {filter}
             ORDER BY updated_at DESC, origin ASC, permission ASC LIMIT ?2"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![origin, limit], |row| {
            Ok(PermissionRule {
                origin: row.get(0)?,
                permission: row.get(1)?,
                decision: row.get(2)?,
                expires_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut rules = rows.collect::<Result<Vec<_>, _>>()?;
        let complete = rules.len() <= MAX_PERMISSION_RULES;
        rules.truncate(MAX_PERMISSION_RULES);
        Ok(PermissionRulesSnapshot { rules, complete })
    }

    /// Replaces one exact-origin durable allow/deny rule.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin, permission, or decision is invalid or
    /// SQLite fails.
    pub fn set_permission_rule(
        &self,
        origin: &str,
        permission: &str,
        decision: &str,
        expires_at: Option<i64>,
        updated_at: i64,
    ) -> Result<(), StoreError> {
        let origin = normalize_permission_origin(origin)?;
        if !is_rememberable_permission(permission) {
            return Err(StoreError::InvalidInput("permission type is unsupported"));
        }
        if !matches!(decision, "allow" | "deny") {
            return Err(StoreError::InvalidInput(
                "durable permission decision must be allow or deny",
            ));
        }
        self.connection.execute(
            "INSERT INTO permission_rules (origin, permission, decision, expires_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(origin, permission) DO UPDATE SET decision = excluded.decision,
             expires_at = excluded.expires_at, updated_at = excluded.updated_at",
            params![origin, permission, decision, expires_at, updated_at],
        )?;
        Ok(())
    }

    /// Replaces a bounded group of exact-origin durable rules atomically.
    ///
    /// Validation and all upserts happen in one SQLite transaction. If any
    /// rule is invalid or an upsert fails, none of the rules are committed.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch is empty, any origin, permission, or
    /// decision is invalid, or SQLite fails.
    pub fn set_permission_rule_batch(&self, rules: &[PermissionRule]) -> Result<(), StoreError> {
        if rules.is_empty() {
            return Err(StoreError::InvalidInput(
                "permission rule batch cannot be empty",
            ));
        }
        let validated = rules
            .iter()
            .map(|rule| {
                let origin = normalize_permission_origin(&rule.origin)?;
                if !is_rememberable_permission(&rule.permission) {
                    return Err(StoreError::InvalidInput("permission type is unsupported"));
                }
                if !matches!(rule.decision.as_str(), "allow" | "deny") {
                    return Err(StoreError::InvalidInput(
                        "durable permission decision must be allow or deny",
                    ));
                }
                Ok((origin, rule))
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let transaction = self.connection.unchecked_transaction()?;
        for (origin, rule) in validated {
            transaction.execute(
                "INSERT INTO permission_rules (origin, permission, decision, expires_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(origin, permission) DO UPDATE SET decision = excluded.decision,
                 expires_at = excluded.expires_at, updated_at = excluded.updated_at",
                params![
                    origin,
                    rule.permission,
                    rule.decision,
                    rule.expires_at,
                    rule.updated_at
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Removes one exact-origin permission rule.
    ///
    /// # Errors
    ///
    /// Returns an error when the origin or permission is invalid or SQLite
    /// fails.
    pub fn reset_permission_rule(
        &self,
        origin: &str,
        permission: &str,
    ) -> Result<bool, StoreError> {
        let origin = normalize_permission_origin(origin)?;
        if !is_rememberable_permission(permission) {
            return Err(StoreError::InvalidInput("permission type is unsupported"));
        }
        Ok(self.connection.execute(
            "DELETE FROM permission_rules WHERE origin = ?1 AND permission = ?2",
            params![origin, permission],
        )? != 0)
    }

    /// Removes a bounded group of exact-origin permission rules atomically.
    ///
    /// Validation happens before the transaction is opened, so an invalid
    /// permission cannot leave an earlier rule in the batch removed.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch is empty, any origin or permission is
    /// invalid, or SQLite fails.
    pub fn reset_permission_rule_batch(
        &self,
        origin: &str,
        permissions: &[String],
    ) -> Result<bool, StoreError> {
        if permissions.is_empty() {
            return Err(StoreError::InvalidInput(
                "permission reset batch cannot be empty",
            ));
        }
        let origin = normalize_permission_origin(origin)?;
        if permissions
            .iter()
            .any(|permission| !is_rememberable_permission(permission))
        {
            return Err(StoreError::InvalidInput("permission type is unsupported"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut removed = false;
        for permission in permissions {
            removed |= transaction.execute(
                "DELETE FROM permission_rules WHERE origin = ?1 AND permission = ?2",
                params![origin, permission],
            )? != 0;
        }
        transaction.commit()?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots, StoreMode};

    #[test]
    fn list_limit_never_claims_an_omitted_origin_is_absent() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        roots.ensure().unwrap();
        let store =
            ProfileStore::open(roots.data.join("permissions.sqlite"), StoreMode::Normal).unwrap();
        let origin = "https://blocked.example.test";
        let mut rules = vec![PermissionRule {
            origin: origin.into(),
            permission: "geolocation".into(),
            decision: "deny".into(),
            expires_at: None,
            updated_at: 1,
        }];
        for index in 1..MAX_PERMISSION_RULES {
            rules.push(PermissionRule {
                origin: format!("https://noise-{index}.example.test"),
                permission: "notifications".into(),
                decision: "deny".into(),
                expires_at: None,
                updated_at: i64::try_from(index + 10).unwrap(),
            });
        }
        store.set_permission_rule_batch(&rules).unwrap();
        let exact_limit = store.permission_rules_snapshot(None).unwrap();
        assert_eq!(exact_limit.rules.len(), MAX_PERMISSION_RULES);
        assert!(exact_limit.complete);
        store
            .set_permission_rule(
                "https://new.example.test",
                "notifications",
                "deny",
                None,
                10_000,
            )
            .unwrap();
        let truncated = store.permission_rules_snapshot(None).unwrap();
        assert_eq!(truncated.rules.len(), MAX_PERMISSION_RULES);
        assert!(!truncated.complete);
        assert!(!truncated.rules.iter().any(|rule| rule.origin == origin));
        let exact_origin = store.permission_rules_snapshot(Some(origin)).unwrap();
        assert!(exact_origin.complete);
        assert_eq!(exact_origin.rules.len(), 1);
        assert_eq!(exact_origin.rules[0].decision, "deny");
        let absent = store
            .permission_rules_snapshot(Some("https://absent.example.test"))
            .unwrap();
        assert!(absent.complete);
        assert!(absent.rules.is_empty());
        drop(store);
        roots.cleanup().unwrap();
    }
}
