//! Committed context metadata shared by independent native window owners.

use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{ContextRegistry, StorageRoots};

use crate::BrowserApplication;

/// An opaque copy of a committed catalog and its durable storage boundary.
/// It contains no authority to change a window's profile or routing config.
#[derive(Clone, Debug)]
pub struct ContextCatalog {
    roots: StorageRoots,
    registry: ContextRegistry,
}

impl BrowserApplication {
    /// Captures the active durable catalog for other owners of the same roots.
    #[must_use]
    pub fn context_catalog(&self) -> Option<ContextCatalog> {
        let profile = self.active_profile.as_ref()?;
        let snapshot = profile.snapshot();
        if !snapshot.durable || snapshot.privacy != PrivacyKind::Normal {
            return None;
        }
        Some(ContextCatalog {
            roots: snapshot.roots?,
            registry: profile.contexts.clone()?,
        })
    }

    /// Refreshes cached metadata without disk I/O or changing profile identity.
    /// Private, transient and unrelated storage roots cannot receive a catalog.
    pub fn refresh_context_catalog(&mut self, catalog: &ContextCatalog) -> bool {
        let Some(profile) = self.active_profile.as_mut() else {
            return false;
        };
        let snapshot = profile.snapshot();
        if !snapshot.durable
            || snapshot.privacy != PrivacyKind::Normal
            || snapshot.roots.as_ref() != Some(&catalog.roots)
            || profile.contexts.is_none()
            || self.profile_open.is_some()
        {
            return false;
        }
        profile.contexts = Some(catalog.registry.clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextCommand, profile::ProfileActivation};
    use ferric_browser_config::Config;
    use ferric_browser_runtime::BrowserRuntime;
    use ferric_browser_storage::RootSpec;
    use uuid::Uuid;

    fn application(roots: &StorageRoots, privacy: PrivacyKind) -> BrowserApplication {
        let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "test").expect("runtime");
        let mut app = BrowserApplication::from_runtime(runtime, Config::default());
        app.activate_profile(ProfileActivation {
            name: "default".into(),
            id: Some(Uuid::new_v4()),
            privacy,
            roots: Some(roots.clone()),
            lock: None,
            contexts: Some(ContextRegistry::open(roots).expect("contexts")),
            storage_path: (privacy == PrivacyKind::Normal)
                .then(|| roots.data.join("browser.sqlite")),
        })
        .expect("activate");
        app
    }

    #[test]
    fn committed_context_changes_reach_existing_owners_without_crossing_storage_or_privacy() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let other_roots = StorageRoots::resolve(RootSpec::Temporary).expect("other roots");
        let mut source = application(&roots, PrivacyKind::Normal);
        let mut existing = application(&roots, PrivacyKind::Normal);
        let mut unrelated = application(&other_roots, PrivacyKind::Normal);
        let mut private = application(&roots, PrivacyKind::Private);
        let identity = existing.profile_snapshot();
        source
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: None,
            })
            .expect("create");
        let catalog = source.context_catalog().expect("committed catalog");
        assert!(existing.refresh_context_catalog(&catalog));
        assert_eq!(existing.contexts(), source.contexts());
        assert_eq!(existing.profile_snapshot(), identity);
        assert!(!unrelated.refresh_context_catalog(&catalog));
        assert!(unrelated.contexts().expect("other catalog").is_empty());
        assert!(!private.refresh_context_catalog(&catalog));
        assert!(private.context_catalog().is_none());
        assert!(private.contexts().expect("private test catalog").is_empty());
        source
            .change_context(ContextCommand::Remove {
                name: "research".into(),
            })
            .expect("remove");
        assert!(
            existing.refresh_context_catalog(&source.context_catalog().expect("updated catalog"))
        );
        assert!(existing.contexts().expect("refreshed catalog").is_empty());
        source.close_profile();
        assert!(source.context_catalog().is_none());
        existing.close_profile();
        unrelated.close_profile();
        private.close_profile();
        roots.cleanup().expect("cleanup");
        other_roots.cleanup().expect("other cleanup");
    }
}
