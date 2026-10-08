use super::{CxxQtType, Pin, QString, StorageRequest, permission_session_key, qobject};

impl qobject::BrowserUi {
    pub(super) fn reset_permission(
        mut self: Pin<&mut Self>,
        origin: &QString,
        permission: &QString,
    ) -> bool {
        let origin = origin.to_string();
        let permission = permission.to_string();
        let session_removed = ferric_browser_storage::normalize_permission_origin(&origin)
            .is_ok_and(|origin| {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                let permission_scope_id = this.profile_id.unwrap_or(this.session_id);
                if permission == "camera-and-microphone" {
                    this.session_permissions
                        .remove(&permission_session_key(
                            permission_scope_id,
                            &origin,
                            "camera",
                        ))
                        .is_some()
                        || this
                            .session_permissions
                            .remove(&permission_session_key(
                                permission_scope_id,
                                &origin,
                                "microphone",
                            ))
                            .is_some()
                } else {
                    this.session_permissions
                        .remove(&permission_session_key(
                            permission_scope_id,
                            &origin,
                            &permission,
                        ))
                        .is_some()
                }
            });
        let normalized = match ferric_browser_storage::normalize_permission_origin(&origin) {
            Ok(origin) => origin,
            Err(_) => {
                self.as_mut().set_status_text(QString::from(
                    "Permission reset rejected; details suppressed",
                ));
                return false;
            }
        };
        if permission == "screen-capture" {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!("permission-reset\t{origin}\t{permission}"));
            self.as_mut().set_status_text(QString::from(
                "Screen-capture consent reset; active captures will be stopped",
            ));
            return true;
        }
        let permissions = if permission == "camera-and-microphone" {
            vec!["camera".to_owned(), "microphone".to_owned()]
        } else if matches!(
            permission.as_str(),
            "camera" | "microphone" | "notifications" | "geolocation" | "clipboard" | "local-fonts"
        ) {
            vec![permission.clone()]
        } else {
            self.as_mut().set_status_text(QString::from(
                "Permission reset rejected: unsupported permission type",
            ));
            return false;
        };
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
        {
            self.as_mut()
                .set_status_text(QString::from(if session_removed {
                    "Permission decision reset"
                } else {
                    "Permission rule was not found"
                }));
            if session_removed {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action =
                    Some(format!("permission-reset\t{origin}\t{permission}"));
            }
            return session_removed;
        }
        let request_result = Some(
            self.as_mut()
                .submit_storage(StorageRequest::PermissionReset {
                    origin: normalized,
                    permissions,
                }),
        );
        match request_result {
            Some(Ok(())) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.pending_permission_reset_session = Some(session_removed);
                this.storage_library_dirty.set(true);
                this.pending_engine_action =
                    Some(format!("permission-reset\t{origin}\t{permission}"));
                drop(rust);
                self.as_mut()
                    .set_status_text(QString::from("Permission reset queued"));
                true
            }
            Some(Err(_)) => {
                self.as_mut().set_status_text(QString::from(
                    "Permission reset unavailable; details suppressed",
                ));
                false
            }
            None => {
                self.as_mut().set_status_text(QString::from(
                    "Permission reset unavailable; retry the command",
                ));
                false
            }
        }
    }
}
