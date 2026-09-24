//! Qt adapter for durable profile-management workflows.
//!
//! Profile reads and mutations run in their dedicated workers. This module owns
//! only the QObject-facing request lifecycle and presentation state; it does
//! not implement profile persistence or policy.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::qobject;

impl qobject::BrowserUi {
    pub(super) fn list_profiles(self: Pin<&mut Self>) -> QString {
        self.as_ref().rust().profile_values.clone()
    }

    pub(super) fn request_profile_list_with_mode(
        mut self: Pin<&mut Self>,
        command_request: bool,
    ) -> bool {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .profile_list_command_pending = command_request;
        let Some(roots) = self
            .as_ref()
            .rust()
            .profile_registry_roots
            .clone()
            .or_else(|| self.as_ref().rust().storage_roots.clone())
        else {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .profile_list_command_pending = false;
            self.as_mut().set_profile_values(QString::default());
            self.as_mut().set_profile_values_pending(false);
            return false;
        };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .profile_list_worker
                .as_mut()
                .ok_or_else(|| "profile reader is unavailable".to_owned())
                .and_then(|worker| worker.request(roots))
        };
        match result {
            Ok(()) => {
                self.as_mut().set_profile_values_pending(true);
                true
            }
            Err(error) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .profile_list_command_pending = false;
                self.as_mut().set_profile_values_pending(false);
                self.as_mut()
                    .set_status_text(QString::from(format!("Profile list failed: {error}")));
                false
            }
        }
    }

    pub(super) fn request_profile_list(self: Pin<&mut Self>) -> bool {
        self.request_profile_list_with_mode(false)
    }

    pub(super) fn create_profile(
        mut self: Pin<&mut Self>,
        name: &QString,
        label: &QString,
    ) -> bool {
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.set_status_text(QString::from(
                "Private profiles cannot create durable profiles",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .profile_delete_worker
            .as_mut()
            .ok_or_else(|| "profile mutation worker is unavailable".to_owned())
            .and_then(|worker| worker.request_create(roots, name.to_string(), label.to_string()));
        match result {
            Ok(()) => {
                self.set_status_text(QString::from("Profile creation queued"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Profile creation failed: {error}")));
                false
            }
        }
    }

    pub(super) fn rename_profile(
        mut self: Pin<&mut Self>,
        name: &QString,
        label: &QString,
    ) -> bool {
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.set_status_text(QString::from(
                "Private profiles cannot rename durable profiles",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .profile_delete_worker
            .as_mut()
            .ok_or_else(|| "profile mutation worker is unavailable".to_owned())
            .and_then(|worker| worker.request_rename(roots, name.to_string(), label.to_string()));
        match result {
            Ok(()) => {
                self.set_status_text(QString::from("Profile rename queued"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Profile rename failed: {error}")));
                false
            }
        }
    }

    pub(super) fn profile_delete_preview(self: Pin<&mut Self>, _name: &QString) -> QString {
        self.as_ref().rust().profile_preview_text.clone()
    }

    pub(super) fn request_profile_delete_preview(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.as_mut().set_profile_preview_pending(false);
            self.as_mut().set_profile_preview_text(QString::default());
            return false;
        };
        self.as_mut()
            .set_profile_preview_text(QString::from("Loading validated profile deletion preview…"));
        let result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .profile_preview_worker
                .as_mut()
                .ok_or_else(|| "profile preview reader is unavailable".to_owned())
                .and_then(|worker| worker.request(roots, name.to_string()))
        };
        match result {
            Ok(()) => {
                self.as_mut().set_profile_preview_pending(true);
                true
            }
            Err(error) => {
                self.as_mut().set_profile_preview_pending(false);
                self.as_mut().set_profile_preview_text(QString::default());
                self.as_mut()
                    .set_status_text(QString::from(format!("Profile preview failed: {error}")));
                false
            }
        }
    }

    pub(super) fn delete_profile(
        mut self: Pin<&mut Self>,
        name: &QString,
        confirmed: bool,
    ) -> bool {
        if !confirmed {
            self.set_status_text(QString::from(
                "Profile deletion requires preview and confirmation",
            ));
            return false;
        }
        let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
            self.set_status_text(QString::from(
                "Private profiles cannot delete durable profiles",
            ));
            return false;
        };
        let active_profile_id = self.as_ref().rust().profile_id;
        let name = name.to_string();
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .profile_delete_worker
            .as_mut()
            .ok_or_else(|| "profile deletion worker is unavailable".to_owned())
            .and_then(|worker| worker.request_delete(roots, name, active_profile_id));
        match result {
            Ok(()) => {
                self.set_status_text(QString::from("Profile deletion queued"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Profile deletion failed: {error}")));
                false
            }
        }
    }
}
