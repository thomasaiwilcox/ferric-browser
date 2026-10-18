use super::{
    BrowserApplication, CollisionPolicy, CxxQtType, DownloadState, ParsedCommand, PathBuf,
    PermissionRule, Pin, QString, StorageRequest, Uuid, ValidatedUrl, Value, choose_download_path,
    cleanup_staged_download, configured_download_directory_path, fs, path_to_file_url, qobject,
    sanitize_download_filename, stage_download_path, unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn allocate_download_id(&self) -> QString {
        let rust = self.rust();
        let Some(window) = rust.window else {
            return QString::default();
        };
        if rust.profile_bootstrap_pending
            || !rust
                .state
                .as_ref()
                .is_some_and(|state| state.windows().contains_key(&window))
        {
            return QString::default();
        }
        // Qt request IDs restart with the native profile. Persistent history
        // needs an application identity retained by that request's owner.
        QString::from(Uuid::new_v4().to_string())
    }

    pub(super) fn default_download_directory(self: Pin<&mut Self>) -> QString {
        QString::from(
            configured_download_directory_path(&self.as_ref().rust().config)
                .to_string_lossy()
                .as_ref(),
        )
    }

    pub(super) fn offer_download(
        mut self: Pin<&mut Self>,
        id: &QString,
        source_url: &QString,
        suggested_name: &QString,
    ) -> QString {
        let safe_name = sanitize_download_filename(&suggested_name.to_string());
        let result = Uuid::parse_str(&id.to_string())
            .map_err(|_| "This download request is invalid.".to_owned())
            .and_then(|id| {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .state
                    .as_mut()
                    .ok_or_else(|| "The window profile is not ready.".to_owned())?
                    .offer_session_download(
                        id,
                        &source_url.to_string(),
                        &safe_name,
                        unix_timestamp(),
                    )
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            self.set_status_text(QString::from(error));
            return QString::default();
        }
        QString::from(safe_name)
    }

    fn record_download_destination(
        mut self: Pin<&mut Self>,
        id: &str,
        path: &std::path::Path,
    ) -> Result<(), String> {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .ok_or_else(|| "The window profile is not ready.".to_owned())?
            .set_session_download_destination(id, path)
            .map_err(|error| error.to_string())
    }

    pub(super) fn accept_download(
        mut self: Pin<&mut Self>,
        id: &QString,
        directory: &QString,
        suggested_name: &QString,
    ) -> QString {
        if !self.as_ref().get_ref().download_save_capacity() {
            self.set_status_text(QString::from(
                "Too many files are being saved. Wait for a download to finish, then retry.",
            ));
            return QString::default();
        }
        let directory = PathBuf::from(directory.to_string());
        if let Err(error) = fs::create_dir_all(&directory) {
            self.set_status_text(QString::from(format!(
                "Download directory unavailable: {error}"
            )));
            return QString::default();
        }
        let path = match choose_download_path(
            &directory,
            &suggested_name.to_string(),
            CollisionPolicy::AutoRename,
        ) {
            Ok(path) => path,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Download destination rejected: {error}"
                )));
                return QString::default();
            }
        };
        let staged = match stage_download_path(
            self.as_ref().rust().storage_roots.as_ref(),
            self.as_ref().rust().session_id,
            &path,
        ) {
            Ok(staged) => staged,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Download staging unavailable: {error}"
                )));
                return QString::default();
            }
        };
        if let Err(error) = self
            .as_mut()
            .record_download_destination(&id.to_string(), &path)
        {
            cleanup_staged_download(&staged);
            self.set_status_text(QString::from(error));
            return QString::default();
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .staged_downloads
            .insert(id.to_string(), staged);
        self.as_ref().rust().storage_library_dirty.set(true);
        path.file_name()
            .and_then(|name| name.to_str())
            .map_or_else(QString::default, QString::from)
    }

    pub(super) fn accept_download_path(
        mut self: Pin<&mut Self>,
        id: &QString,
        selected_path: &QString,
    ) -> QString {
        if !self.as_ref().get_ref().download_save_capacity() {
            self.set_status_text(QString::from(
                "Too many files are being saved. Wait for a download to finish, then retry.",
            ));
            return QString::default();
        }
        let result = (|| {
            let path = PathBuf::from(selected_path.to_string());
            if !path.is_absolute() || selected_path.to_string().chars().any(char::is_control) {
                return Err("selected download path is not a safe absolute path".to_owned());
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "selected download path has no valid filename".to_owned())?;
            let directory = path
                .parent()
                .ok_or_else(|| "selected download path has no containing directory".to_owned())?;
            let chosen = choose_download_path(directory, name, CollisionPolicy::Ask)
                .map_err(|error| error.to_string())?;
            if chosen != path {
                return Err(
                    "selected download path is not a safe non-colliding destination".into(),
                );
            }
            let staged = stage_download_path(
                self.as_ref().rust().storage_roots.as_ref(),
                self.as_ref().rust().session_id,
                &path,
            )?;
            if let Err(error) = self
                .as_mut()
                .record_download_destination(&id.to_string(), &path)
            {
                cleanup_staged_download(&staged);
                return Err(error);
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .staged_downloads
                .insert(id.to_string(), staged);
            self.as_ref().rust().storage_library_dirty.set(true);
            Ok(name.to_owned())
        })();
        match result {
            Ok(name) => QString::from(name),
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Download destination rejected: {error}"
                )));
                QString::default()
            }
        }
    }

    pub(super) fn download_staging_directory(self: Pin<&mut Self>, id: &QString) -> QString {
        self.as_ref()
            .rust()
            .staged_downloads
            .get(&id.to_string())
            .map_or_else(QString::default, |staged| {
                QString::from(staged.staging_directory().to_string_lossy().as_ref())
            })
    }

    pub(super) fn discard_download_staging(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().cancel_download_finalization(id);
        if let Some(staged) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .staged_downloads
            .remove(&id.to_string())
        {
            cleanup_staged_download(&staged);
        }
    }

    pub(super) fn update_download(
        mut self: Pin<&mut Self>,
        id: &QString,
        state: &QString,
        bytes_received: i64,
        finished: bool,
    ) -> bool {
        let state = match state.to_string().as_str() {
            "offered" => DownloadState::Offered,
            "selecting-destination" => DownloadState::SelectingDestination,
            "in-progress" => DownloadState::InProgress,
            "paused" => DownloadState::Paused,
            "completed" => DownloadState::Completed,
            "interrupted" => DownloadState::Interrupted,
            "cancelled" => DownloadState::Cancelled,
            _ => {
                self.set_status_text(QString::from("Unknown download state"));
                return false;
            }
        };
        let completed_at = (finished && state == DownloadState::Completed).then(unix_timestamp);
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .ok_or_else(|| "The window profile is not ready.".to_owned())
            .and_then(|application| {
                application
                    .update_session_download(
                        &id.to_string(),
                        state,
                        bytes_received,
                        finished,
                        completed_at,
                    )
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                // Session-only transfers must not announce their existence on
                // the ordinary durable-profile IPC event stream.
                if self.as_ref().rust().profile_persistence.is_durable() {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    super::publish_ipc_event(
                        &mut this.ipc_sequence,
                        "download.changed",
                        serde_json::json!({ "download_id": id.to_string(), "state": state.as_str(), "bytes_received": bytes_received }),
                    );
                }
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn current_download_rows(
        &self,
    ) -> Result<Vec<ferric_browser_application::DownloadRow>, String> {
        let rust = self.rust();
        rust.state
            .as_ref()
            .ok_or_else(|| "The window profile is not ready.".to_owned())?
            .session_download_rows(
                rust.storage_library
                    .as_ref()
                    .map_or(&[], |library| library.downloads.as_slice()),
            )
            .map_err(|error| error.to_string())
    }

    pub(super) fn prepare_download_history(mut self: Pin<&mut Self>) {
        let needed = {
            let rust = self.as_ref().get_ref().rust();
            rust.profile_persistence.is_durable()
                && (rust.storage_library.is_none() || rust.storage_library_dirty.get())
                && rust.storage_library_read_state.permits_query_request()
        };
        if needed {
            self.as_mut().request_storage_library();
        }
    }

    pub(super) fn list_downloads(mut self: Pin<&mut Self>) -> QString {
        self.as_mut().prepare_download_history();
        let rows = self
            .as_ref()
            .get_ref()
            .current_download_rows()
            .unwrap_or_default();
        QString::from(
            rows.iter()
                .map(|row| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        row.record.id,
                        row.record.state.as_str(),
                        row.record.bytes_received,
                        row.record.destination.replace(['\t', '\n', '\r'], " ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    pub(super) fn download_rows(mut self: Pin<&mut Self>) -> super::QVariant {
        self.as_mut().prepare_download_history();
        super::download_presentation::project(
            &self
                .as_ref()
                .get_ref()
                .current_download_rows()
                .unwrap_or_default(),
        )
    }

    pub(super) fn download_history_status(&self) -> QString {
        use ferric_browser_application::DownloadHistoryStatus;
        QString::from(
            match self
                .rust()
                .state
                .as_ref()
                .map(BrowserApplication::download_history_status)
            {
                Some(DownloadHistoryStatus::NotStored) => "not-stored",
                Some(status @ (DownloadHistoryStatus::Ready | DownloadHistoryStatus::Saving)) => {
                    if matches!(
                        self.rust().storage_library_read_state,
                        super::CatalogReadState::Failed(_)
                    ) {
                        "unavailable"
                    } else if self.rust().storage_library_read_state.is_pending() {
                        "loading"
                    } else if status == DownloadHistoryStatus::Saving {
                        "saving"
                    } else {
                        "ready"
                    }
                }
                Some(DownloadHistoryStatus::Unavailable) | None => "unavailable",
            },
        )
    }

    pub(super) fn refresh_download_history(mut self: Pin<&mut Self>) {
        self.as_mut().ensure_storage_worker();
        if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
            state.retry_download_history();
        }
        self.as_ref().rust().storage_library_dirty.set(true);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .storage_library_read_state = super::CatalogReadState::Unrequested;
        self.request_storage_library();
    }

    pub(super) fn resolve_download_desktop_uri(
        self: Pin<&mut Self>,
        id: &QString,
        reveal: bool,
    ) -> Result<String, String> {
        (|| {
            let row = self
                .as_ref()
                .get_ref()
                .current_download_rows()?
                .into_iter()
                .find(|row| row.record.id == id.to_string())
                .ok_or_else(|| {
                    "This download is no longer available. Refresh Downloads and try again."
                        .to_owned()
                })?;
            if !row.file_ready {
                return Err("This file is not ready to open yet.".to_owned());
            }
            let record = row.record;
            let destination = PathBuf::from(record.destination);
            let metadata = fs::symlink_metadata(&destination)
                .map_err(|error| format!("download file is unavailable: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("completed download is not a regular file".to_owned());
            }
            let target = if reveal {
                destination
                    .parent()
                    .ok_or_else(|| "download has no containing directory".to_owned())?
                    .to_owned()
            } else {
                destination
            };
            if reveal
                && !fs::symlink_metadata(&target)
                    .map_err(|error| format!("download directory is unavailable: {error}"))?
                    .is_dir()
            {
                return Err("download containing path is not a directory".to_owned());
            }
            path_to_file_url(&target)
        })()
    }

    pub(super) fn download_desktop_action(
        mut self: Pin<&mut Self>,
        id: &QString,
        reveal: bool,
    ) -> bool {
        match self.as_mut().resolve_download_desktop_uri(id, reveal) {
            Ok(uri) => {
                self.as_mut().set_download_desktop_uri(QString::from(uri));
                true
            }
            Err(error) => {
                self.as_mut().set_download_desktop_uri(QString::default());
                self.set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn execute_download_control_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, super::PublicError> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err(format!("{} requires exactly one download ID", command.name).into());
        }
        let action = command
            .name
            .strip_prefix("download-")
            .ok_or_else(|| "invalid download control command".to_owned())?;
        let mut result = self
            .as_mut()
            .queue_download_action(&command.arguments[0], action)?;
        result["action"] = Value::String(command.name.clone());
        Ok(result)
    }

    pub(super) fn execute_permission_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        match command.name.as_str() {
            "permissions" => {
                if command.arguments.len() > 1 {
                    return Err("permissions accepts an optional exact origin".into());
                }
                let origin = command
                    .arguments
                    .first()
                    .map_or_else(QString::default, QString::from);
                let payload = self.as_mut().list_permissions(&origin).to_string();
                if payload.is_empty() {
                    return Err(self.as_ref().rust().status_text.to_string());
                }
                Ok(serde_json::from_str(&payload).map_err(|error| error.to_string())?)
            }
            "permission-reset" => {
                if command.arguments.len() != 2 {
                    return Err("permission-reset requires ORIGIN and PERMISSION".into());
                }
                if !self.as_mut().reset_permission(
                    &QString::from(command.arguments[0].as_str()),
                    &QString::from(command.arguments[1].as_str()),
                ) {
                    return Err(self.as_ref().rust().status_text.to_string());
                }
                Ok(serde_json::json!({"status": "reset"}))
            }
            _ => Err("unsupported permission command".into()),
        }
    }

    pub(super) fn queue_download_action(
        mut self: Pin<&mut Self>,
        id: &str,
        action: &str,
    ) -> Result<Value, super::PublicError> {
        let row = self
            .as_ref()
            .get_ref()
            .current_download_rows()?
            .into_iter()
            .find(|row| row.record.id == id)
            .ok_or_else(|| {
                crate::browser_ui_download_requests::download_request_error(
                    ferric_browser_ipc::ErrorCode::StaleTarget,
                    "This download is no longer available. Refresh Downloads and try again.",
                )
            })?;
        if matches!(action, "pause" | "resume" | "cancel") && !row.active {
            return Err("This download is no longer running.".into());
        }
        if row.finishing && matches!(action, "pause" | "resume") {
            return Err(crate::browser_ui_download_requests::download_request_error(
                ferric_browser_ipc::ErrorCode::Busy,
                "This file is being saved. You can cancel it until saving finishes.",
            ));
        }
        let record = row.record;
        let pending = match action {
            "cancel"
                if matches!(
                    record.state,
                    DownloadState::Offered
                        | DownloadState::SelectingDestination
                        | DownloadState::InProgress
                        | DownloadState::Paused
                ) =>
            {
                format!("download-cancel\t{id}")
            }
            "pause" if record.state == DownloadState::InProgress => {
                format!("download-pause\t{id}")
            }
            "resume" if record.state == DownloadState::Paused => {
                format!("download-resume\t{id}")
            }
            "retry"
                if matches!(
                    record.state,
                    DownloadState::Interrupted | DownloadState::Cancelled
                ) =>
            {
                if row.retry == ferric_browser_application::DownloadRetryState::Unavailable {
                    return Err(crate::browser_ui_download_requests::download_request_error(
                        ferric_browser_ipc::ErrorCode::InvalidArgument,
                        "This saved address can't be retried. Start the download again from its page.",
                    ));
                }
                let url = ValidatedUrl::parse(&record.source_url)
                    .map_err(|error| format!("download source is invalid: {error}"))?;
                let scheme = url.as_str().split_once(':').map(|(scheme, _)| scheme);
                if !matches!(scheme, Some("http" | "https")) {
                    return Err("download retry supports only HTTP(S) sources".into());
                }
                let target = super::current_target(
                    self.as_ref().rust().state.as_ref(),
                    self.as_ref().rust().tab,
                )
                .ok_or_else(|| {
                    "Open a page in this window before retrying the download.".to_owned()
                })?;
                let mut result = self.as_mut().queue_download_request_for(
                    target,
                    url.as_str(),
                    "Preparing download…",
                    Some(id),
                )?;
                result["download_id"] = Value::String(id.to_owned());
                result["action"] = Value::String(action.to_owned());
                return Ok(result);
            }
            "cancel" => return Err("only active downloads can be cancelled".into()),
            "pause" => return Err("only in-progress downloads can be paused".into()),
            "resume" => return Err("only paused downloads can be resumed".into()),
            "retry" => return Err("only interrupted or cancelled downloads can be retried".into()),
            _ => return Err("unknown download action".into()),
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(pending);
        self.set_status_text(QString::from(match action {
            "cancel" => "Download cancellation requested",
            "pause" => "Download pause requested",
            "resume" => "Download resume requested",
            _ => "Download retry requested",
        }));
        Ok(serde_json::json!({"status":"accepted", "download_id":id, "action":action}))
    }

    pub(super) fn request_download_action(
        mut self: Pin<&mut Self>,
        id: &QString,
        action: &QString,
    ) -> bool {
        match self
            .as_mut()
            .queue_download_action(&id.to_string(), &action.to_string())
        {
            Ok(_) => true,
            Err(error) => {
                self.set_status_text(QString::from(error.user_message()));
                false
            }
        }
    }

    pub(super) fn shares_permission_session(&self, other: &Self) -> bool {
        let left = self.rust();
        let right = other.rust();
        !left.profile_bootstrap_pending
            && !right.profile_bootstrap_pending
            && left
                .state
                .as_ref()
                .zip(right.state.as_ref())
                .is_some_and(|(left, right)| left.shares_permission_session(right))
    }

    pub(super) fn permission_decision(
        mut self: Pin<&mut Self>,
        origin: &QString,
        permission: &QString,
    ) -> QString {
        let resolution =
            self.as_ref().rust().state.as_ref().map(|state| {
                state.resolve_permission(&origin.to_string(), &permission.to_string())
            });
        let Some(resolution) = resolution else {
            return QString::from("deny");
        };
        if resolution.refresh_needed {
            let _ = self
                .as_mut()
                .request_permission_preferences(&origin.to_string());
        }
        if resolution.pending {
            return QString::from("pending");
        }
        QString::from(match resolution.decision {
            ferric_browser_config::PermissionDecision::Ask => "ask",
            ferric_browser_config::PermissionDecision::Allow => "allow",
            ferric_browser_config::PermissionDecision::Deny => "deny",
        })
    }

    pub(super) fn remember_permission(
        mut self: Pin<&mut Self>,
        origin: &QString,
        permission: &QString,
        decision: &QString,
        lifetime: &QString,
    ) -> bool {
        let origin = origin.to_string();
        let permission = permission.to_string();
        let decision = decision.to_string();
        let lifetime = lifetime.to_string();
        if !matches!(decision.as_str(), "allow" | "deny") {
            self.set_status_text(QString::from("Permission decision must be allow or deny"));
            return false;
        }
        if lifetime == "session" {
            let result = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.state
                    .as_mut()
                    .ok_or(ferric_browser_application::SessionPermissionError::ProfileUnavailable)
                    .and_then(|state| {
                        state.remember_session_permission(
                            &origin,
                            &permission,
                            &if decision == "allow" {
                                ferric_browser_config::PermissionDecision::Allow
                            } else {
                                ferric_browser_config::PermissionDecision::Deny
                            },
                        )
                    })
            };
            return match result {
                Ok(()) => {
                    self.set_status_text(QString::from("Permission session decision saved"));
                    true
                }
                Err(error) => {
                    self.set_status_text(QString::from(error.to_string()));
                    false
                }
            };
        }
        let result = if lifetime == "site" {
            if self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
            {
                Err(ferric_browser_storage::StoreError::PrivateNoDurableState)
            } else {
                let normalized = ferric_browser_storage::normalize_permission_origin(&origin);
                match normalized {
                    Ok(normalized) => {
                        let permissions = if permission == "camera-and-microphone" {
                            vec!["camera", "microphone"]
                        } else {
                            vec![permission.as_str()]
                        };
                        if permissions.iter().any(|permission| {
                            !ferric_browser_storage::is_rememberable_permission(permission)
                        }) {
                            Err(ferric_browser_storage::StoreError::InvalidInput(
                                "permission type is unsupported",
                            ))
                        } else {
                            let updated_at = unix_timestamp();
                            let rules = permissions
                                .into_iter()
                                .map(|permission| PermissionRule {
                                    origin: normalized.clone(),
                                    permission: permission.into(),
                                    decision: decision.clone(),
                                    expires_at: None,
                                    updated_at,
                                })
                                .collect::<Vec<_>>();
                            if self.as_mut().queue_permission_writes(rules) {
                                Ok(())
                            } else {
                                Err(ferric_browser_storage::StoreError::InvalidInput(
                                    "permission write queue is full",
                                ))
                            }
                        }
                    }
                    Err(error) => Err(error),
                }
            }
        } else {
            Err(ferric_browser_storage::StoreError::InvalidInput(
                "permission lifetime is unsupported",
            ))
        };
        match result {
            Ok(()) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                self.set_status_text(QString::from("Permission rule queued"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Permission rule rejected: {error}")));
                false
            }
        }
    }

    pub(super) fn list_permissions(mut self: Pin<&mut Self>, origin: &QString) -> QString {
        let origin = (!origin.to_string().is_empty()).then(|| origin.to_string());
        let private = self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage();
        if !private
            && (self.as_ref().rust().storage_library.is_none()
                || self.as_ref().rust().storage_library_dirty.get())
        {
            self.as_mut().request_storage_library();
            self.set_status_text(QString::from("Permission rules are still loading; retry"));
            return QString::from(
                serde_json::json!({
                    "status": "loading",
                    "private": false,
                    "ready": false,
                    "rules": []
                })
                .to_string(),
            );
        }
        let normalized_origin = match origin
            .as_deref()
            .map(ferric_browser_storage::normalize_permission_origin)
            .transpose()
        {
            Ok(origin) => origin,
            Err(error) => {
                self.set_status_text(QString::from(format!("Permissions unavailable: {error}")));
                return QString::default();
            }
        };
        let rules = self
            .as_ref()
            .rust()
            .storage_library
            .as_ref()
            .into_iter()
            .flat_map(|library| library.permissions.iter())
            .filter(|rule| {
                normalized_origin
                    .as_deref()
                    .is_none_or(|origin| rule.origin == origin)
            })
            .map(|rule| {
                serde_json::json!({
                    "origin": rule.origin,
                    "permission": rule.permission,
                    "decision": rule.decision,
                    "expires_at": rule.expires_at,
                    "updated_at": rule.updated_at
                })
            })
            .collect::<Vec<_>>();
        QString::from(
            serde_json::json!({
                "status": "listed",
                "private": private,
                "ready": true,
                "revision": self.as_ref().rust().storage_library_revision,
                "rules": rules
            })
            .to_string(),
        )
    }

    pub(super) fn reset_permission(
        mut self: Pin<&mut Self>,
        origin: &QString,
        permission: &QString,
    ) -> bool {
        let origin = origin.to_string();
        let permission = permission.to_string();
        let session_removed = if permission == "screen-capture" {
            false
        } else {
            let result = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.state
                    .as_mut()
                    .ok_or(ferric_browser_application::SessionPermissionError::ProfileUnavailable)
                    .and_then(|state| state.reset_session_permission(&origin, &permission))
            };
            match result {
                Ok(removed) => removed,
                Err(error) => {
                    self.set_status_text(QString::from(error.to_string()));
                    return false;
                }
            }
        };
        let normalized = match ferric_browser_storage::normalize_permission_origin(&origin) {
            Ok(origin) => origin,
            Err(error) => {
                self.as_mut()
                    .set_status_text(QString::from(format!("Permission reset rejected: {error}")));
                return false;
            }
        };
        if permission == "screen-capture" {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action =
                Some(format!("permission-reset\t{normalized}\t{permission}"));
            self.as_mut().set_status_text(QString::from(
                "Screen-capture consent reset; active captures will be stopped",
            ));
            return true;
        }
        let permissions = if permission == "camera-and-microphone" {
            vec!["camera".to_owned(), "microphone".to_owned()]
        } else if ferric_browser_storage::is_rememberable_permission(&permission) {
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
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action =
                Some(format!("permission-reset\t{normalized}\t{permission}"));
            self.as_mut().set_status_text(QString::from(
                "Permission reset; existing page access will be stopped",
            ));
            return true;
        }
        let request_result = Some(
            self.as_mut()
                .submit_storage(StorageRequest::PermissionReset {
                    origin: normalized.clone(),
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
                    Some(format!("permission-reset\t{normalized}\t{permission}"));
                drop(rust);
                self.as_mut()
                    .set_status_text(QString::from("Permission reset queued"));
                true
            }
            Some(Err(error)) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Permission reset unavailable; retry the command: {error}"
                )));
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
