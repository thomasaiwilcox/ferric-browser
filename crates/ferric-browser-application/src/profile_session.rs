//! Shared resources whose lifetime matches one validated native profile session.

use crate::{
    download_catalog::DownloadCatalog, profile::ProfileActivation,
    session_permissions::durable::PermissionCache,
};
use ferric_browser_config::PermissionDecision;
use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::StorageRoots;
use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex, OnceLock, Weak},
};
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SessionScope {
    Durable { id: Uuid, roots: StorageRoots },
    Memory(Uuid),
}

pub(crate) struct Session {
    pub(crate) scope: SessionScope,
    pub(crate) decisions: Mutex<BTreeMap<(String, String), PermissionDecision>>,
    pub(crate) durable: Mutex<PermissionCache>,
    pub(crate) downloads: Mutex<DownloadCatalog>,
}

#[derive(Clone)]
pub(crate) struct ProfileSession(pub(crate) Arc<Session>);

impl fmt::Debug for ProfileSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Origins and decisions are deliberately absent from profile debug output.
        formatter
            .debug_struct("ProfileSession")
            .finish_non_exhaustive()
    }
}

impl ProfileSession {
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
                durable: Mutex::default(),
                downloads: Mutex::new(if durable {
                    DownloadCatalog::durable()
                } else {
                    DownloadCatalog::default()
                }),
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
