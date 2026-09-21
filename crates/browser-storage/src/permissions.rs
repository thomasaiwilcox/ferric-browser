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

fn supported_permission(permission: &str) -> bool {
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
        let origin = origin.map(normalize_permission_origin).transpose()?;
        let limit = i64::try_from(MAX_PERMISSION_RULES).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT origin, permission, decision, expires_at, updated_at
             FROM permission_rules
             WHERE (?1 IS NULL OR origin = ?1)
             ORDER BY updated_at DESC, origin ASC, permission ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![origin, limit], |row| {
            Ok(PermissionRule {
                origin: row.get(0)?,
                permission: row.get(1)?,
                decision: row.get(2)?,
                expires_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
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
        if !supported_permission(permission) {
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
                if !supported_permission(&rule.permission) {
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
        if !supported_permission(permission) {
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
            .any(|permission| !supported_permission(permission))
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
