use super::{
    CollisionPolicy, CxxQtType, DownloadState, DownloadUpdate, ParsedCommand, PathBuf,
    PermissionRule, Pin, QString, StorageRequest, ValidatedUrl, Value, choose_download_path,
    cleanup_staged_download, configured_download_directory_path, finalize_staged_download, fs,
    is_safe_history_url, path_to_file_url, permission_session_key, qobject,
    sanitize_download_filename, stage_download_path, unix_timestamp,
};

impl qobject::BrowserUi {
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
        let source = ValidatedUrl::parse(source_url.to_string())
            .ok()
            .filter(|url| is_safe_history_url(url.as_str()))
            .map_or_else(|| "[redacted]".into(), |url| url.to_string());
        if self.as_ref().rust().profile_persistence.is_durable() {
            let result = self.as_mut().create_download_via_worker(
                id.to_string(),
                source,
                String::new(),
                DownloadState::Offered,
                unix_timestamp(),
            );
            if let Err(error) = result {
                self.set_status_text(QString::from(format!(
                    "Download index unavailable: {error}"
                )));
            }
        }
        QString::from(safe_name)
    }

    pub(super) fn accept_download(
        mut self: Pin<&mut Self>,
        id: &QString,
        directory: &QString,
        suggested_name: &QString,
    ) -> QString {
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
        if self.as_ref().rust().profile_persistence.is_durable() {
            if let Err(error) = self.as_mut().set_download_destination_via_worker(
                id.to_string(),
                path.to_string_lossy().into_owned(),
            ) {
                cleanup_staged_download(&staged);
                self.set_status_text(QString::from(format!(
                    "Download index update failed: {error}"
                )));
                return QString::default();
            }
            if !self.as_mut().queue_download_update(DownloadUpdate {
                id: id.to_string(),
                state: DownloadState::InProgress,
                bytes_received: 0,
                completed_at: None,
            }) {
                cleanup_staged_download(&staged);
                return QString::default();
            }
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
            if self.as_ref().rust().profile_persistence.is_durable() {
                if let Err(error) = self.as_mut().set_download_destination_via_worker(
                    id.to_string(),
                    path.to_string_lossy().into_owned(),
                ) {
                    cleanup_staged_download(&staged);
                    return Err(error);
                }
                if !self.as_mut().queue_download_update(DownloadUpdate {
                    id: id.to_string(),
                    state: DownloadState::InProgress,
                    bytes_received: 0,
                    completed_at: None,
                }) {
                    cleanup_staged_download(&staged);
                    return Err("download lifecycle update could not be queued".into());
                }
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

    pub(super) fn finalize_download(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id_string = id.to_string();
        let Some(staged) = self
            .as_ref()
            .rust()
            .staged_downloads
            .get(&id_string)
            .cloned()
        else {
            self.set_status_text(QString::from("Download staging record is unavailable"));
            return false;
        };
        match finalize_staged_download(&staged) {
            Ok(()) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .staged_downloads
                    .remove(&id_string);
                true
            }
            Err(error) => {
                cleanup_staged_download(&staged);
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .staged_downloads
                    .remove(&id_string);
                self.set_status_text(QString::from(format!(
                    "Download finalization failed: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn discard_download_staging(mut self: Pin<&mut Self>, id: &QString) {
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
        let update = DownloadUpdate {
            id: id.to_string(),
            state,
            bytes_received,
            completed_at,
        };
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
        {
            return true;
        }
        self.as_mut().queue_download_update(update)
    }

    pub(super) fn list_downloads(mut self: Pin<&mut Self>) -> QString {
        if self.as_ref().rust().storage_library.is_none()
            || self.as_ref().rust().storage_library_dirty.get()
        {
            self.as_mut().request_storage_library();
            if self.as_ref().rust().storage_library.is_none() {
                return QString::default();
            }
        }
        let binding = self.as_ref();
        let downloads = &binding
            .rust()
            .storage_library
            .as_ref()
            .expect("checked")
            .downloads;
        QString::from(
            downloads
                .iter()
                .map(|download| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        download.id,
                        download.state.as_str(),
                        download.bytes_received,
                        download.destination.replace(['\t', '\n', '\r'], " ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    pub(super) fn resolve_download_desktop_uri(
        mut self: Pin<&mut Self>,
        id: &QString,
        reveal: bool,
    ) -> Result<String, String> {
        if self.as_ref().rust().storage_library.is_none()
            || self.as_ref().rust().storage_library_dirty.get()
        {
            self.as_mut().request_storage_library();
        }
        (|| {
            let binding = self.as_ref();
            let rust = binding.rust();
            if rust.storage_library_dirty.get() {
                return Err("download metadata is refreshing; retry the action".to_owned());
            }
            let library = rust
                .storage_library
                .as_ref()
                .ok_or_else(|| "download metadata is still loading; retry the action".to_owned())?;
            let record = library
                .downloads
                .iter()
                .find(|record| record.id == id.to_string())
                .cloned()
                .ok_or_else(|| "download ID was not found".to_owned())?;
            if record.state != DownloadState::Completed {
                return Err("only completed downloads can be opened or revealed".to_owned());
            }
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
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err(format!("{} requires exactly one download ID", command.name));
        }
        let action = command
            .name
            .strip_prefix("download-")
            .ok_or_else(|| "invalid download control command".to_owned())?;
        self.as_mut()
            .queue_download_action(&command.arguments[0], action)?;
        Ok(serde_json::json!({
            "status": "accepted",
            "download_id": command.arguments[0],
            "action": command.name
        }))
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
    ) -> Result<(), String> {
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
        {
            return Err("download metadata is unavailable in a private session".to_owned());
        }
        if self.as_ref().rust().storage_library_dirty.get() {
            self.as_mut().request_storage_library();
            return Err("download metadata is refreshing; retry the action".to_owned());
        }
        let library = self.as_ref().rust().storage_library.clone();
        let Some(library) = library else {
            self.as_mut().request_storage_library();
            return Err("download metadata is still loading; retry the action".to_owned());
        };
        let record = library
            .downloads
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .ok_or_else(|| "download ID was not found".to_owned())?;
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
                let url = ValidatedUrl::parse(&record.source_url)
                    .map_err(|error| format!("download source is invalid: {error}"))?;
                let scheme = url.as_str().split_once(':').map(|(scheme, _)| scheme);
                if !matches!(scheme, Some("http" | "https")) {
                    return Err("download retry supports only HTTP(S) sources".into());
                }
                format!("download-retry\t{url}")
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
        Ok(())
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
            Ok(()) => true,
            Err(error) => {
                self.set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn permission_decision(
        mut self: Pin<&mut Self>,
        origin: &QString,
        permission: &QString,
    ) -> QString {
        let origin = origin.to_string();
        let permission = permission.to_string();
        // Screen/window capture is intentionally per-request.  Unlike the
        // other permission types, it must never inherit a configured allow or
        // a remembered decision: the portal source chooser is part of the
        // consent boundary for every capture request.
        if permission == "screen-capture" {
            return QString::from("ask");
        }
        let normalized_origin = ferric_browser_storage::normalize_permission_origin(&origin).ok();
        let normal_profile = self.as_ref().rust().profile_persistence.is_durable();
        let permission_scope_id = self
            .as_ref()
            .rust()
            .profile_id
            .unwrap_or(self.as_ref().rust().session_id);
        if normal_profile
            && let Some(normalized_origin) = normalized_origin.as_deref()
            && let Some(rule) = self
                .as_ref()
                .rust()
                .config
                .get("permission_rules")
                .and_then(Value::as_array)
                .and_then(|rules| {
                    rules.iter().find(|rule| {
                        rule.get("profile").and_then(Value::as_str)
                            == Some(self.as_ref().rust().profile_name.as_str())
                            && rule.get("permission").and_then(Value::as_str)
                                == Some(permission.as_str())
                            && rule
                                .get("origin")
                                .and_then(Value::as_str)
                                .and_then(|origin| {
                                    ferric_browser_storage::normalize_permission_origin(origin).ok()
                                })
                                .as_deref()
                                == Some(normalized_origin)
                    })
                })
            && let Some(decision) = rule.get("decision").and_then(Value::as_str)
            && matches!(decision, "allow" | "deny" | "ask")
        {
            return QString::from(decision);
        }
        if normal_profile
            && (self.as_ref().rust().storage_library.is_none()
                || self.as_ref().rust().storage_library_dirty.get())
        {
            self.as_mut().request_storage_library();
        }
        if normal_profile
            && let Some(normalized_origin) = normalized_origin.as_deref()
            && !self.as_ref().rust().storage_library_dirty.get()
        {
            let durable = self
                .as_ref()
                .rust()
                .storage_library
                .as_ref()
                .into_iter()
                .flat_map(|library| library.permissions.iter())
                .find(|rule| {
                    rule.origin == normalized_origin
                        && rule.permission == permission
                        && rule
                            .expires_at
                            .is_none_or(|expires_at| expires_at > unix_timestamp())
                })
                .map(|rule| rule.decision.clone());
            if let Some(decision) = durable {
                return QString::from(decision.as_str());
            }
        }
        if let Some(normalized_origin) = normalized_origin.as_deref()
            && let Some(decision) =
                self.as_ref()
                    .rust()
                    .session_permissions
                    .get(&permission_session_key(
                        permission_scope_id,
                        normalized_origin,
                        &permission,
                    ))
        {
            return QString::from(decision.as_str());
        }
        let decision = match permission.as_str() {
            "camera" | "microphone" | "screen-capture" | "notifications" | "geolocation"
            | "clipboard" | "local-fonts" => {
                let key = permission.replace('-', "_");
                self.as_ref()
                    .rust()
                    .config
                    .get("permissions")
                    .and_then(|permissions| permissions.get(&key))
                    .and_then(Value::as_str)
                    .filter(|value| matches!(*value, "ask" | "allow" | "deny"))
                    .unwrap_or("ask")
                    .to_owned()
            }
            _ => "deny".to_owned(),
        };
        QString::from(decision.as_str())
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
        let result = if lifetime == "session" {
            let normalized_origin =
                match ferric_browser_storage::normalize_permission_origin(&origin) {
                    Ok(origin) => origin,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Permission session decision rejected: {error}"
                        )));
                        return false;
                    }
                };
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let permission_scope_id = this.profile_id.unwrap_or(this.session_id);
            if permission == "camera-and-microphone" {
                this.session_permissions.insert(
                    permission_session_key(permission_scope_id, &normalized_origin, "camera"),
                    decision.clone(),
                );
                this.session_permissions.insert(
                    permission_session_key(permission_scope_id, &normalized_origin, "microphone"),
                    decision,
                );
            } else if matches!(
                permission.as_str(),
                "camera"
                    | "microphone"
                    | "notifications"
                    | "geolocation"
                    | "clipboard"
                    | "local-fonts"
            ) {
                this.session_permissions.insert(
                    permission_session_key(permission_scope_id, &normalized_origin, &permission),
                    decision,
                );
            } else {
                self.set_status_text(QString::from(
                    "Permission session decision is unsupported for this capability",
                ));
                return false;
            }
            Ok(())
        } else if lifetime == "site" {
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
                            !matches!(
                                *permission,
                                "camera"
                                    | "microphone"
                                    | "notifications"
                                    | "geolocation"
                                    | "clipboard"
                                    | "local-fonts"
                            )
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
                if lifetime != "session" {
                    self.as_ref().rust().storage_library_dirty.set(true);
                }
                self.set_status_text(QString::from(if lifetime == "session" {
                    "Permission session decision saved"
                } else {
                    "Permission rule queued"
                }));
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
