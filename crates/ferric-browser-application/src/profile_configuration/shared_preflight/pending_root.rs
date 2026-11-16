//! Actual resolved-root membership while a profile worker prepares its inputs.
use super::{
    Arc, ContextStore, Entry, Mutex, OwnerInputs, OwnerRegistration, OwnerState,
    SharedConfiguration,
};
use crate::profile::ProfileActivation;
use ferric_browser_core::PrivacyKind;
use std::path::Path;

/// The worker owns this real pending membership and shared-root lifetime.
/// A validation capture can retain neither the lease nor the root authority.
pub(crate) struct PendingRootOwner {
    _registration: OwnerRegistration,
    authority: Arc<SharedConfiguration>,
    revision: Option<u64>,
}
impl PendingRootOwner {
    pub(crate) fn for_root(root: &Path) -> Self {
        let group = SharedConfiguration::for_root(Some(root));
        let revision = group.state.lock().ok().map(|state| state.revision);
        let state = Arc::new(Mutex::new(OwnerState {
            inputs: Arc::new(OwnerInputs {
                sources: None,
                profile: None,
                pending: true,
            }),
            contexts: ContextStore::unavailable(),
            busy: 0,
            dirty: false,
            wake: None,
        }));
        let registration = OwnerRegistration {
            group: Arc::downgrade(&group),
            marker: Arc::new(()),
            state: Arc::clone(&state),
        };
        {
            let mut owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            owners.entries.push(Entry {
                owner: Arc::downgrade(&registration.marker),
                state,
            });
            owners.membership = Arc::new(());
        }
        Self {
            _registration: registration,
            authority: group,
            revision,
        }
    }

    pub(crate) fn check_revision(&self) -> Result<(), ferric_browser_config::ConfigError> {
        let current = self.authority.state.lock().ok().map(|state| state.revision);
        if self.revision.is_none() || current != self.revision {
            return Err(ferric_browser_config::ConfigError::validation(
                format_args!(
                    "Shared settings changed while this profile was being prepared. Retry opening it."
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn for_activation(activation: &ProfileActivation) -> Option<Self> {
        if activation.privacy == PrivacyKind::Normal && activation.storage_path.is_some() {
            activation
                .roots
                .as_ref()
                .map(|roots| Self::for_root(&roots.state))
        } else {
            None
        }
    }
}
