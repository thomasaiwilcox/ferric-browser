use super::{CommitId, CxxQtType, OperationId, Pin, QString, TabId, qobject};
use crate::MediaFingerprintKeyWorker;

const MEDIA_FINGERPRINT_KEY_PENDING: &str = "ferric-media-fingerprint-key-pending";
const MAX_MEDIA_SOURCE_RECHECKS: usize = 16;

fn exact_archive_cleanup_operation(
    active: Option<OperationId>,
    requested: &str,
) -> Option<OperationId> {
    let requested = requested.parse::<OperationId>().ok()?;
    active.filter(|operation| *operation == requested)
}

impl qobject::BrowserUi {
    pub(crate) fn has_lazy_restore_page(self: Pin<&mut Self>, index: i32) -> bool {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            return false;
        };
        self.as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|application| application.has_lazy_restore_page(tab))
    }

    pub(crate) fn confirm_versioned_commit_delete(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let raw = request_id.to_string();
        let Ok(parsed) = raw.parse::<u64>() else {
            return false;
        };
        if parsed.to_string() != raw {
            return false;
        }
        let ticket = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(window) = this.window else {
                return false;
            };
            this.state.as_mut().and_then(|application| {
                application
                    .confirm_commit_delete_preview(window, parsed)
                    .ok()
            })
        };
        let Some(ticket) = ticket else {
            self.as_mut().set_status_text(QString::from(
                "Commit deletion preview expired or is no longer valid",
            ));
            return false;
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().latest_versioning_read_request = Some(ticket.value());
        }
        self.as_mut()
            .set_versioning_panel_kind(QString::from("delete-pending"));
        self.as_mut().set_versioning_panel_data(QString::from(
            serde_json::json!({ "requestId": ticket.value().to_string() }).to_string(),
        ));
        self.as_mut()
            .set_status_text(QString::from("Revalidating commit deletion"));
        true
    }

    pub(crate) fn cancel_versioned_commit_delete(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let raw = request_id.to_string();
        let Ok(parsed) = raw.parse::<u64>() else {
            return false;
        };
        if parsed.to_string() != raw {
            return false;
        }
        let cancelled = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(window) = this.window else {
                return false;
            };
            this.state
                .as_mut()
                .is_some_and(|application| application.cancel_commit_delete_preview(window, parsed))
        };
        if cancelled {
            self.as_mut()
                .set_status_text(QString::from("Commit deletion cancelled"));
        }
        cancelled
    }

    pub(crate) fn confirm_versioned_policy_restore(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let raw = request_id.to_string();
        let Ok(parsed) = raw.parse::<u64>() else {
            return false;
        };
        if parsed.to_string() != raw {
            return false;
        }
        let confirmed = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(window) = this.window else {
                return false;
            };
            this.state.as_mut().and_then(|application| {
                application
                    .confirm_allowlisted_policy_restore(window, parsed)
                    .ok()
                    .map(|(commit, snapshot)| (window, commit, snapshot))
            })
        };
        let Some((window, commit, snapshot)) = confirmed else {
            self.as_mut().set_status_text(QString::from(
                "Policy preview expired or the owner/configuration changed",
            ));
            return false;
        };
        if self
            .as_mut()
            .accept_versioned_policy_configuration(snapshot)
            .is_err()
        {
            self.as_mut().set_status_text(QString::from(
                "The policy was applied, but its presentation could not be synchronized",
            ));
            return false;
        }
        let restore_ticket = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.state.as_mut().and_then(|application| {
                application
                    .request_new_window_restore_plan(window, commit)
                    .ok()
            })
        };
        if restore_ticket.is_none() {
            self.as_mut()
                .set_versioning_panel_kind(QString::from("error"));
            self.as_mut().set_versioning_panel_data(QString::from(
                serde_json::json!({
                    "message": "The policy setting was applied, but the page restore could not be queued. You can run restore again without --policies."
                })
                .to_string(),
            ));
            self.as_mut().set_status_text(QString::from(
                "Policy applied; page restore could not be queued",
            ));
            return false;
        }
        self.as_mut()
            .set_versioning_panel_kind(QString::from("policy-applied"));
        self.as_mut().set_versioning_panel_data(QString::from(
            serde_json::json!({ "commit": commit.to_string() }).to_string(),
        ));
        self.as_mut().set_status_text(QString::from(
            "Allowlisted policy applied; opening the structural restore",
        ));
        true
    }

    pub(crate) fn cancel_versioned_policy_restore(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let raw = request_id.to_string();
        let Ok(parsed) = raw.parse::<u64>() else {
            return false;
        };
        if parsed.to_string() != raw {
            return false;
        }
        let cancelled = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(window) = this.window else {
                return false;
            };
            this.state.as_mut().is_some_and(|application| {
                application.cancel_allowlisted_policy_restore(window, parsed)
            })
        };
        if cancelled {
            self.as_mut().set_status_text(QString::from(
                "Policy restore cancelled; current policy unchanged",
            ));
        }
        cancelled
    }

    pub(crate) fn forget_safe_get_navigation_for(mut self: Pin<&mut Self>, index: i32) {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            return;
        };
        if let Some(application) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
            application.invalidate_safe_get_navigation(tab);
        }
    }

    pub(crate) fn navigation_typed_safe_get_committed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        url: QString,
    ) -> bool {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .is_some_and(|application| {
                application.note_typed_safe_get_navigation(tab, &url.to_string())
            })
    }

    /// The QML caller has just obtained the exact `WebEngineProfile` instance
    /// used by the owning view host. This is an adapter-owned lifetime token,
    /// never an ID supplied by page content or IPC.
    pub(crate) fn bind_native_versioning_profile(mut self: Pin<&mut Self>) -> bool {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .bind_native_versioning_profile_owner()
    }

    /// Trusted startup of a dedicated blank Qt window. The worker re-reads
    /// the exact commit in that window's own normal-profile application;
    /// source-window runtime IDs are never imported into this reducer.
    pub(crate) fn request_versioned_pristine_restore(
        mut self: Pin<&mut Self>,
        commit_id: &QString,
        restore_media_positions: bool,
        restore_editable_drafts: bool,
    ) -> bool {
        self.as_mut().request_pristine_restore_plan(
            commit_id,
            restore_media_positions,
            restore_editable_drafts,
            false,
        )
    }

    /// Requests the same exact commit plan for a fresh primary runtime window,
    /// whose ordinary tab strip can present every page in the plan.
    pub(crate) fn request_versioned_pristine_tab_restore(
        mut self: Pin<&mut Self>,
        commit_id: &QString,
        restore_media_positions: bool,
        restore_editable_drafts: bool,
    ) -> bool {
        self.as_mut().request_pristine_restore_plan(
            commit_id,
            restore_media_positions,
            restore_editable_drafts,
            true,
        )
    }

    fn request_pristine_restore_plan(
        mut self: Pin<&mut Self>,
        commit_id: &QString,
        restore_media_positions: bool,
        restore_editable_drafts: bool,
        tab_capable_surface: bool,
    ) -> bool {
        let Ok(commit) = commit_id.to_string().parse::<CommitId>() else {
            self.as_mut()
                .set_status_text(QString::from("Invalid commit ID"));
            return false;
        };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.pending_pristine_restore_request.is_some()
                || this.native_versioning_profile.is_none()
            {
                None
            } else {
                let request_id = match (this.window, this.state.as_mut()) {
                    (Some(window), Some(application)) => application
                        .request_new_window_restore_plan(window, commit)
                        .ok()
                        .map(ferric_browser_application::StorageTicket::value),
                    _ => None,
                };
                this.pending_pristine_restore_request = request_id;
                this.pending_pristine_restore_media =
                    request_id.is_some() && restore_media_positions;
                this.pending_pristine_restore_editable =
                    request_id.is_some() && restore_editable_drafts;
                this.pending_pristine_restore_tab_surface =
                    request_id.is_some() && tab_capable_surface;
                request_id
            }
        };
        if result.is_some() {
            self.as_mut()
                .set_status_text(QString::from("Versioned restore plan pending"));
            true
        } else {
            self.as_mut()
                .set_status_text(QString::from("Versioned restore could not start"));
            false
        }
    }

    /// Materializes the saved archive for the exact commit into a private
    /// temporary file. The QML surface receiving the URL is deliberately
    /// separate from the normal `WebEngine` profile and has no page bridge.
    pub(crate) fn request_versioned_archive_view(
        mut self: Pin<&mut Self>,
        commit_id: &QString,
    ) -> bool {
        let Ok(commit) = commit_id.to_string().parse::<CommitId>() else {
            self.as_mut()
                .set_status_text(QString::from("Invalid commit ID"));
            return false;
        };
        let ticket = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.pending_archive_view_request.is_some()
                || this.pending_archive_view_cleanup.is_some()
                || this.active_archive_view_operation.is_some()
            {
                None
            } else {
                match (this.window, this.state.as_mut()) {
                    (Some(window), Some(application)) => application
                        .request_versioned_archive_view(window, commit)
                        .ok(),
                    _ => None,
                }
            }
        };
        let Some(ticket) = ticket else {
            self.as_mut().set_status_text(QString::from(
                "Stored archive view is unavailable for this commit or profile",
            ));
            return false;
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_archive_view_request = Some(ticket.value());
            this.archive_view_cleanup_retries = 0;
        }
        self.as_mut()
            .set_status_text(QString::from("Preparing isolated stored archive view"));
        true
    }

    /// Blanks and hides the isolated surface before scheduling exact-file
    /// cleanup. A failed cleanup remains visible in status and is retried by
    /// application teardown recovery rather than deleting a broader path.
    pub(crate) fn close_versioned_archive_view(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let requested_operation = operation_id.to_string();
        let ticket = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.pending_archive_view_cleanup.is_some() {
                None
            } else {
                match (
                    this.window,
                    exact_archive_cleanup_operation(
                        this.active_archive_view_operation,
                        &requested_operation,
                    ),
                    this.state.as_mut(),
                ) {
                    (Some(window), Some(operation), Some(application)) => application
                        .close_versioned_archive_view(window, operation)
                        .ok(),
                    _ => None,
                }
            }
        };
        let Some(ticket) = ticket else {
            self.as_mut()
                .set_status_text(QString::from("Stored archive cleanup is unavailable"));
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_archive_view_cleanup = Some(ticket.value());
        self.as_mut()
            .set_versioning_archive_view_url(QString::default());
        self.as_mut()
            .set_versioning_archive_view_operation(QString::default());
        self.as_mut()
            .set_versioning_archive_source_url(QString::default());
        self.as_mut()
            .set_versioning_archive_captured_at_ms(QString::default());
        self.as_mut()
            .set_versioning_archive_fidelity_label(QString::default());
        self.as_mut()
            .set_status_text(QString::from("Closing isolated archive view"));
        true
    }

    /// Returns one bounded, app-qualified live geometry-capture request for
    /// the exact selected restored tab. The JSON is passed only between this
    /// adapter and its `ApplicationWorld` helper, never through IPC or logs.
    pub(crate) fn versioned_restore_geometry_capture_request(
        mut self: Pin<&mut Self>,
        request_id: &QString,
        destination_tab_id: &QString,
    ) -> QString {
        let Ok(request_id) = request_id.to_string().parse::<u64>() else {
            return QString::default();
        };
        let Some(destination_tab) = TabId::from_display(&destination_tab_id.to_string()) else {
            return QString::default();
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.poll_media_fingerprint_key();
        if let Some((pending_request_id, pending_json)) = this.pending_media_restore_capture.take()
        {
            if pending_request_id == request_id {
                let Some(profile_id) = this.profile_id else {
                    return QString::default();
                };
                let Ok(mut request) = serde_json::from_str::<serde_json::Value>(&pending_json)
                else {
                    return QString::default();
                };
                if request
                    .get("include_media")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && this.media_fingerprint_key_for(profile_id).is_none()
                {
                    this.request_media_fingerprint_key(false);
                    let pending = this
                        .media_fingerprint_key_worker
                        .as_ref()
                        .is_some_and(MediaFingerprintKeyWorker::is_pending);
                    if pending {
                        this.pending_media_restore_capture = Some((request_id, pending_json));
                        return QString::from(MEDIA_FINGERPRINT_KEY_PENDING);
                    }
                    request["include_media"] = serde_json::Value::Bool(false);
                }
                return serde_json::to_string(&request)
                    .map_or_else(|_| QString::default(), QString::from);
            }
        }
        let (Some(window), Some(application)) = (this.window, this.state.as_mut()) else {
            return QString::default();
        };
        let request_json = application
            .build_restore_geometry_capture_request(window, request_id, destination_tab)
            .ok();
        let Some(request_json) = request_json else {
            return QString::default();
        };
        let Ok(mut request) = serde_json::from_str::<serde_json::Value>(&request_json) else {
            return QString::default();
        };
        if request
            .get("include_media")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            let Some(profile_id) = this.profile_id else {
                request["include_media"] = serde_json::Value::Bool(false);
                return serde_json::to_string(&request)
                    .map_or_else(|_| QString::default(), QString::from);
            };
            if this.media_fingerprint_key_for(profile_id).is_none() {
                if this.media_fingerprint_key_failure == Some(profile_id) {
                    this.media_fingerprint_key_failure = None;
                }
                this.request_media_fingerprint_key(false);
                let pending = this
                    .media_fingerprint_key_worker
                    .as_ref()
                    .is_some_and(MediaFingerprintKeyWorker::is_pending);
                if pending {
                    this.pending_media_restore_capture = Some((request_id, request_json));
                    return QString::from(MEDIA_FINGERPRINT_KEY_PENDING);
                }
                request["include_media"] = serde_json::Value::Bool(false);
            }
        }
        serde_json::to_string(&request).map_or_else(|_| QString::default(), QString::from)
    }

    /// Consumes the one-use observation qualification and returns only a
    /// geometry plan. Malformed or stale page output is discarded closed.
    pub(crate) fn versioned_restore_geometry_script(
        mut self: Pin<&mut Self>,
        request_id: &QString,
        current_sidecar_json: &QString,
    ) -> QString {
        let Ok(request_id) = request_id.to_string().parse::<u64>() else {
            return QString::default();
        };
        let current_sidecar_json = current_sidecar_json.to_string();
        if current_sidecar_json.len() > ferric_browser_core::versioning::MAX_PAGE_SIDECAR_BYTES {
            return QString::default();
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(normalized) = this.normalize_media_page_state(&current_sidecar_json, false) else {
            return QString::default();
        };
        let (Some(window), Some(application)) = (this.window, this.state.as_mut()) else {
            return QString::default();
        };
        let Ok(plan_json) = application.build_restore_geometry_script(
            window,
            request_id,
            normalized.json.as_bytes(),
        ) else {
            return QString::default();
        };
        let Ok(mut plan) = serde_json::from_str::<serde_json::Value>(&plan_json) else {
            return QString::default();
        };
        if let Some(media_targets) = plan
            .pointer_mut("/restore_geometry/media_targets")
            .and_then(serde_json::Value::as_array_mut)
        {
            let mut attached = 0_usize;
            media_targets.retain_mut(|target| {
                if attached >= MAX_MEDIA_SOURCE_RECHECKS {
                    return false;
                }
                let Some(candidate_id) = target
                    .get("candidate_id")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                else {
                    return false;
                };
                let Some(source_url) = normalized.source_urls_by_candidate.get(&candidate_id)
                else {
                    return false;
                };
                target["expected_source_url"] = serde_json::Value::String(source_url.clone());
                attached += 1;
                true
            });
        }
        let Ok(plan_json) = serde_json::to_string(&plan) else {
            return QString::default();
        };
        if plan_json.len() > 65_536 {
            return QString::default();
        }
        QString::from(plan_json)
    }

    /// Projects sensitive stored draft text only into the native recovery
    /// review dialog for the exact still-owned restored page. The QML caller
    /// must display values as plain text and never log or persist them.
    pub(crate) fn versioned_restore_editable_preview(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> QString {
        let Ok(request_id) = request_id.to_string().parse::<u64>() else {
            return QString::default();
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let (Some(window), Some(application)) = (this.window, this.state.as_mut()) else {
            return QString::default();
        };
        application
            .restore_editable_preview_json(window, request_id)
            .map_or_else(|_| QString::default(), QString::from)
    }

    pub(crate) fn confirm_versioned_restore_editable_preview(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let Ok(request_id) = request_id.to_string().parse::<u64>() else {
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let (Some(window), Some(application)) = (this.window, this.state.as_mut()) else {
            return false;
        };
        application.confirm_restore_editable_preview(window, request_id)
    }

    pub(crate) fn discard_versioned_restore_sidecar(
        mut self: Pin<&mut Self>,
        request_id: &QString,
    ) -> bool {
        let Ok(request_id) = request_id.to_string().parse::<u64>() else {
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        rust.as_mut()
            .get_mut()
            .state
            .as_mut()
            .is_some_and(|application| application.discard_restore_sidecar(request_id))
    }
}

#[cfg(test)]
mod archive_cleanup_tests {
    use super::{OperationId, exact_archive_cleanup_operation};

    #[test]
    fn archive_cleanup_requires_the_active_exact_operation() {
        let active = OperationId::fresh();
        let other = OperationId::fresh();
        assert_eq!(
            exact_archive_cleanup_operation(Some(active), &active.to_string()),
            Some(active)
        );
        assert_eq!(
            exact_archive_cleanup_operation(Some(active), &other.to_string()),
            None
        );
        assert_eq!(exact_archive_cleanup_operation(Some(active), ""), None);
        assert_eq!(
            exact_archive_cleanup_operation(None, &active.to_string()),
            None
        );
    }
}
