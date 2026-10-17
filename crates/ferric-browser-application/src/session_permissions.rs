//! Memory-only grants owned by a validated profile session, shared by its windows.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex, OnceLock, Weak},
};

use ferric_browser_config::PermissionDecision;
use ferric_browser_core::{PrivacyKind, ShutdownState};
use ferric_browser_storage::{
    StorageRoots, StoreError, is_rememberable_permission, normalize_permission_origin,
};
use uuid::Uuid;

use crate::{BrowserApplication, profile::ProfileActivation};

#[derive(Debug, Eq, PartialEq)]
enum SessionScope {
    Durable { id: Uuid, roots: StorageRoots },
    Memory(Uuid),
}

struct Session {
    scope: SessionScope,
    decisions: Mutex<BTreeMap<(String, String), PermissionDecision>>,
}

#[derive(Clone)]
pub(crate) struct SessionPermissions(Arc<Session>);

impl fmt::Debug for SessionPermissions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Origins and decisions are deliberately absent from profile debug output.
        formatter
            .debug_struct("SessionPermissions")
            .finish_non_exhaustive()
    }
}

impl SessionPermissions {
    pub(crate) fn new(activation: &ProfileActivation) -> Self {
        static SESSIONS: OnceLock<Mutex<Vec<Weak<Session>>>> = OnceLock::new();

        let scope = if activation.privacy == PrivacyKind::Normal
            && activation.storage_path.is_some()
            && let (Some(id), Some(roots)) = (activation.id, activation.roots.as_ref())
        {
            SessionScope::Durable {
                id,
                roots: roots.clone(),
            }
        } else {
            SessionScope::Memory(Uuid::new_v4())
        };
        let durable = matches!(scope, SessionScope::Durable { .. });
        let make = |scope| {
            Arc::new(Session {
                scope,
                decisions: Mutex::new(BTreeMap::new()),
            })
        };
        if !durable {
            return Self(make(scope));
        }
        // Durable owners opened independently in this process still belong to
        // one profile session. Weak entries cannot keep grants alive themselves.
        let Ok(mut sessions) = SESSIONS.get_or_init(Mutex::default).lock() else {
            return Self(make(scope));
        };
        sessions.retain(|session| session.strong_count() > 0);
        if let Some(existing) = sessions
            .iter()
            .filter_map(Weak::upgrade)
            .find(|existing| existing.scope == scope)
        {
            return Self(existing);
        }
        let session = make(scope);
        sessions.push(Arc::downgrade(&session));
        Self(session)
    }
}

#[derive(Debug)]
pub enum SessionPermissionError {
    ProfileUnavailable,
    InvalidOrigin(StoreError),
    UnsupportedPermission,
    InvalidDecision,
    Unavailable,
}

impl fmt::Display for SessionPermissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileUnavailable => formatter.write_str("The window profile is not ready."),
            Self::InvalidOrigin(error) => write!(formatter, "Permission origin rejected: {error}"),
            Self::UnsupportedPermission => {
                formatter.write_str("This permission cannot be remembered for a session.")
            }
            Self::InvalidDecision => {
                formatter.write_str("Only allow or deny decisions can be remembered.")
            }
            Self::Unavailable => formatter.write_str(
                "Session permissions are unavailable. Reopen this window and try again.",
            ),
        }
    }
}
impl std::error::Error for SessionPermissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidOrigin(error) => Some(error),
            _ => None,
        }
    }
}

fn capabilities(permission: &str) -> Result<[Option<&str>; 2], SessionPermissionError> {
    if permission == "camera-and-microphone" {
        return Ok([Some("camera"), Some("microphone")]);
    }
    if is_rememberable_permission(permission) {
        Ok([Some(permission), None])
    } else {
        Err(SessionPermissionError::UnsupportedPermission)
    }
}

impl BrowserApplication {
    fn permission_session(&self) -> Result<&SessionPermissions, SessionPermissionError> {
        if self.profile_open.is_some() || self.runtime.view().shutdown() != ShutdownState::Running {
            return Err(SessionPermissionError::ProfileUnavailable);
        }
        self.active_profile
            .as_ref()
            .map(|profile| &profile.session_permissions)
            .ok_or(SessionPermissionError::ProfileUnavailable)
    }

    /// Tests the actual session resource, without exposing profile IDs or roots.
    #[must_use]
    pub fn shares_permission_session(&self, other: &Self) -> bool {
        self.permission_session()
            .ok()
            .zip(other.permission_session().ok())
            .is_some_and(|(left, right)| Arc::ptr_eq(&left.0, &right.0))
    }

    /// Reads one exact-origin capability. Screen capture is never remembered.
    #[must_use]
    pub fn session_permission(&self, origin: &str, permission: &str) -> Option<PermissionDecision> {
        if !is_rememberable_permission(permission) {
            return None;
        }
        let origin = normalize_permission_origin(origin).ok()?;
        self.permission_session()
            .ok()?
            .0
            .decisions
            .lock()
            .ok()?
            .get(&(origin, permission.to_owned()))
            .cloned()
    }

    /// Remembers allow/deny for this profile session and all its existing owners.
    ///
    /// # Errors
    /// Rejects unavailable profiles, unsafe origins, unsupported capabilities,
    /// ask decisions, and unavailable session state.
    pub fn remember_session_permission(
        &mut self,
        origin: &str,
        permission: &str,
        decision: &PermissionDecision,
    ) -> Result<(), SessionPermissionError> {
        if decision == &PermissionDecision::Ask {
            return Err(SessionPermissionError::InvalidDecision);
        }
        let origin =
            normalize_permission_origin(origin).map_err(SessionPermissionError::InvalidOrigin)?;
        let permissions = capabilities(permission)?;
        let mut decisions = self
            .permission_session()?
            .0
            .decisions
            .lock()
            .map_err(|_| SessionPermissionError::Unavailable)?;
        for capability in permissions.into_iter().flatten() {
            decisions.insert((origin.clone(), capability.to_owned()), decision.clone());
        }
        Ok(())
    }

    /// Atomically removes every capability in a combined decision.
    ///
    /// # Errors
    /// Rejects unavailable profiles, unsafe origins, unsupported capabilities,
    /// and unavailable session state.
    pub fn reset_session_permission(
        &mut self,
        origin: &str,
        permission: &str,
    ) -> Result<bool, SessionPermissionError> {
        let origin =
            normalize_permission_origin(origin).map_err(SessionPermissionError::InvalidOrigin)?;
        let permissions = capabilities(permission)?;
        let mut decisions = self
            .permission_session()?
            .0
            .decisions
            .lock()
            .map_err(|_| SessionPermissionError::Unavailable)?;
        let mut removed = false;
        for capability in permissions.into_iter().flatten() {
            removed |= decisions
                .remove(&(origin.clone(), capability.to_owned()))
                .is_some();
        }
        Ok(removed)
    }
}

#[cfg(test)]
#[path = "session_permissions_tests.rs"]
mod tests;
