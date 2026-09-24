//! Off-thread profile registry operations used by the Qt adapter.
//!
//! This module owns profile-listing, mutation, and deletion-preview work. The
//! Qt bridge only submits requests and presents their bounded results.

use crate::single_flight::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
use ferric_browser_storage::{
    ProfileDeletionOutcome, ProfileLock, ProfilePrivacy, ProfileRegistry, StorageRoots,
    delete_profile_transaction,
};
use uuid::Uuid;

pub(super) struct ProfileDeleteWorker {
    inner: SingleFlightWorker<ProfileMutation, Result<ProfileMutationResult, String>>,
}

pub(super) struct ProfileListWorker {
    inner: SingleFlightWorker<StorageRoots, Result<String, String>>,
}

pub(super) struct ProfilePreviewWorker {
    inner: SingleFlightWorker<(StorageRoots, String), Result<String, String>>,
}

pub(super) enum ProfileMutation {
    Create {
        roots: StorageRoots,
        name: String,
        label: String,
    },
    Rename {
        roots: StorageRoots,
        name: String,
        label: String,
    },
    Delete {
        roots: StorageRoots,
        name: String,
        active_profile_id: Option<Uuid>,
    },
}

pub(super) enum ProfileMutationResult {
    Created,
    Renamed,
    Deleted(ProfileDeletionOutcome),
}

fn profile_delete_preview_text(roots: &StorageRoots, name: &str) -> Result<String, String> {
    let registry = ProfileRegistry::open(roots)
        .map_err(|error| format!("profile registry unavailable: {error}"))?;
    let profile = registry
        .profiles()
        .iter()
        .find(|profile| profile.name == name)
        .ok_or_else(|| "profile was not found".to_owned())?;
    Ok(format!(
        "Profile: {}\nUUID: {}\nRust data: {}\nSession data: {}\nCache: {}\nQtWebEngine storage is not deleted by this operation.",
        profile.name,
        profile.id,
        roots
            .data
            .join("profiles")
            .join(profile.id.to_string())
            .display(),
        roots
            .state
            .join("sessions")
            .join(profile.id.to_string())
            .display(),
        roots
            .cache
            .join("profiles")
            .join(profile.id.to_string())
            .display(),
    ))
}

pub(super) fn profile_from_list_values(values: &str, name: &str) -> Option<(String, String)> {
    values.lines().find_map(|line| {
        let mut fields = line.splitn(3, '\t');
        let candidate = fields.next()?;
        let label = fields.next()?;
        (candidate == name).then(|| (candidate.to_owned(), label.to_owned()))
    })
}

impl ProfileDeleteWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-profile-mutator",
            |request: ProfileMutation| {
                (|| match request {
                    ProfileMutation::Create { roots, name, label } => {
                        let mut registry = ProfileRegistry::open(&roots)
                            .map_err(|error| format!("profile registry unavailable: {error}"))?;
                        registry
                            .create(&name, &label, ProfilePrivacy::Normal)
                            .map_err(|error| format!("profile creation failed: {error}"))?;
                        Ok(ProfileMutationResult::Created)
                    }
                    ProfileMutation::Rename { roots, name, label } => {
                        let mut registry = ProfileRegistry::open(&roots)
                            .map_err(|error| format!("profile registry unavailable: {error}"))?;
                        registry
                            .rename_label(&name, &label)
                            .map_err(|error| format!("profile rename failed: {error}"))?;
                        Ok(ProfileMutationResult::Renamed)
                    }
                    ProfileMutation::Delete {
                        roots,
                        name,
                        active_profile_id,
                    } => {
                        let registry = ProfileRegistry::open(&roots)
                            .map_err(|error| format!("profile registry unavailable: {error}"))?;
                        let profile = registry
                            .profiles()
                            .iter()
                            .find(|profile| profile.name == name)
                            .cloned()
                            .ok_or_else(|| "profile was not found".to_owned())?;
                        if active_profile_id == Some(profile.id)
                            || ProfileLock::is_held(&roots, profile.id)
                        {
                            return Err("profile is loaded and cannot be deleted".into());
                        }
                        let outcome = delete_profile_transaction(&roots, &profile.name)
                            .map_err(|error| error.to_string())?;
                        Ok(ProfileMutationResult::Deleted(outcome))
                    }
                })()
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request_create(
        &mut self,
        roots: StorageRoots,
        name: String,
        label: String,
    ) -> Result<(), String> {
        self.request(ProfileMutation::Create { roots, name, label })
    }

    pub(super) fn request_rename(
        &mut self,
        roots: StorageRoots,
        name: String,
        label: String,
    ) -> Result<(), String> {
        self.request(ProfileMutation::Rename { roots, name, label })
    }

    pub(super) fn request_delete(
        &mut self,
        roots: StorageRoots,
        name: String,
        active_profile_id: Option<Uuid>,
    ) -> Result<(), String> {
        self.request(ProfileMutation::Delete {
            roots,
            name,
            active_profile_id,
        })
    }

    fn request(&mut self, request: ProfileMutation) -> Result<(), String> {
        match self.inner.submit(request) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("profile mutation is already pending".into()),
            Err(SubmitError::QueueFull) => Err("profile mutation queue is full".into()),
            Err(SubmitError::Stopped) => Err("profile mutation worker stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<ProfileMutationResult, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("profile deletion worker stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl ProfileListWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner =
            SingleFlightWorker::spawn("ferric-browser-profile-reader", |roots: StorageRoots| {
                ProfileRegistry::open(&roots)
                    .map(|registry| {
                        registry
                            .profiles()
                            .iter()
                            .map(|profile| {
                                format!("{}\t{}\t{}", profile.name, profile.label, profile.id)
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .map_err(|error| format!("profile registry unavailable: {error}"))
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, roots: StorageRoots) -> Result<(), String> {
        match self.inner.submit(roots) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("profile list is already pending".into()),
            Err(SubmitError::QueueFull) => Err("profile list queue is full".into()),
            Err(SubmitError::Stopped) => Err("profile reader stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<String, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("profile reader stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl ProfilePreviewWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-profile-preview-reader",
            |(roots, name): (StorageRoots, String)| profile_delete_preview_text(&roots, &name),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, roots: StorageRoots, name: String) -> Result<(), String> {
        match self.inner.submit((roots, name)) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("profile preview is already pending".into()),
            Err(SubmitError::QueueFull) => Err("profile preview queue is full".into()),
            Err(SubmitError::Stopped) => Err("profile preview reader stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<String, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("profile preview reader stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

#[cfg(test)]
mod tests {
    use super::profile_from_list_values;

    #[test]
    fn profile_list_values_keep_the_name_and_label_boundary() {
        let values = "default\tDefault\t123\nwork\tWork\t456";
        assert_eq!(
            profile_from_list_values(values, "work"),
            Some(("work".to_owned(), "Work".to_owned()))
        );
        assert_eq!(profile_from_list_values(values, "missing"), None);
    }
}
