//! Durable profile-store bootstrap outside the Qt object implementation.
//!
//! The worker calls this module with explicit profile identity and paths; the
//! resulting handles are installed by the Qt adapter only after success.

use crate::{
    ProfileStoreSetup,
    native_files::{resolve_browser_roots, set_private_directory_permissions},
};
use ferric_browser_storage::{ProfileLock, ProfileRegistry, ProfileStore, StoreMode};
use std::fs;

pub(super) fn open_profile_store(
    transient_profile: bool,
    profile_name: &str,
    profile_label: &str,
    storage_base: &str,
) -> Result<ProfileStoreSetup, String> {
    if transient_profile {
        return Ok(ProfileStoreSetup {
            store: None,
            profile_id: None,
            session_path: None,
            session_state_root: None,
            profile_lock: None,
            roots: None,
            profile_names: Vec::new(),
        });
    }
    let roots = resolve_browser_roots(storage_base)?;
    let mut registry = ProfileRegistry::open(&roots).map_err(|error| error.to_string())?;
    let profile = registry
        .get_or_create(profile_name, profile_label)
        .map_err(|error| error.to_string())?;
    let profile_id = profile.id;
    let profile_names = registry
        .profiles()
        .iter()
        .map(|profile| profile.name.clone())
        .collect::<Vec<_>>();
    let profile_lock =
        ProfileLock::acquire(&roots, profile_id).map_err(|error| error.to_string())?;
    let profile_root = roots.data.join("profiles").join(profile_id.to_string());
    fs::create_dir_all(&profile_root).map_err(|error| error.to_string())?;
    set_private_directory_permissions(&profile_root).map_err(|error| error.to_string())?;
    let store = ProfileStore::open(profile_root.join("browser.sqlite"), StoreMode::Normal)
        .map_err(|error| error.to_string())?;
    let session_path = roots
        .state
        .join("sessions")
        .join(profile_id.to_string())
        .join("current.json");
    Ok(ProfileStoreSetup {
        store: Some(store),
        profile_id: Some(profile_id),
        session_path: Some(session_path),
        session_state_root: Some(roots.state.clone()),
        profile_lock: Some(profile_lock),
        roots: Some(roots),
        profile_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_files::resolve_browser_roots;
    use uuid::Uuid;

    #[test]
    fn transient_profile_has_no_durable_handles() {
        let setup = open_profile_store(true, "private", "Private", "").expect("setup");
        assert!(setup.store.is_none());
        assert!(setup.profile_id.is_none());
        assert!(setup.roots.is_none());
    }

    #[test]
    fn durable_profiles_are_created_under_the_selected_root() {
        let base = std::env::temp_dir().join(format!("ferric-profile-{}", Uuid::new_v4()));
        let setup =
            open_profile_store(false, "default", "Default", base.to_string_lossy().as_ref())
                .expect("setup");
        assert!(setup.store.is_some());
        assert!(setup.profile_id.is_some());
        assert!(setup.session_path.is_some());
        let setup_data = setup.roots.as_ref().expect("roots").data.clone();
        let roots = resolve_browser_roots(base.to_string_lossy().as_ref()).expect("roots");
        assert_eq!(setup_data, roots.data);
        drop(setup);
        let _ = std::fs::remove_dir_all(base);
    }
}
