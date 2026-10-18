//! Correlate requested downloads with an actual offer in the captured profile.

use super::{
    BrowserUiRust, CxxQtType, Duration, Instant, PendingDownload, Pin, PublicError, QString,
    Target, Uuid, ValidatedUrl, Value, captured_target_is_current, qobject, safe_ipc_url,
};

use ferric_browser_ipc::ErrorCode;

pub(super) fn download_request_error(code: ErrorCode, message: impl Into<String>) -> PublicError {
    let message = message.into();
    PublicError::new(code, message.clone(), message)
}

pub(super) fn finish_pending_download_operation(rust: &mut BrowserUiRust, status: &str) {
    if let Some(request) = rust.pending_download.take() {
        rust.operation_states
            .insert(request.operation_id, status.to_owned());
    }
}

impl qobject::BrowserUi {
    fn current_download_request(&self) -> Option<&PendingDownload> {
        let rust = self.rust();
        rust.pending_download.as_ref().filter(|request| {
            !rust.profile_bootstrap_pending
                && request.created_at.elapsed() < Duration::from_secs(180)
                && rust.state.as_ref().is_some_and(
                    ferric_browser_application::BrowserApplication::session_downloads_available,
                )
                && captured_target_is_current(rust.state.as_ref(), request.target)
        })
    }

    pub(super) fn download_request_source_index(&self, token: &QString) -> i32 {
        self.current_download_request()
            .filter(|request| request.token == token.to_string() && request.issued)
            .and_then(|request| {
                self.rust()
                    .tab_ids
                    .iter()
                    .position(|tab| *tab == request.target.tab)
            })
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    pub(super) fn queue_download_request(
        self: Pin<&mut Self>,
        target: Target,
        url: &str,
        status: &str,
    ) -> Result<Value, PublicError> {
        self.queue_download_request_for(target, url, status, None)
    }

    pub(super) fn queue_download_request_for(
        mut self: Pin<&mut Self>,
        target: Target,
        url: &str,
        status: &str,
        retry: Option<&str>,
    ) -> Result<Value, PublicError> {
        let url = ValidatedUrl::parse(url).map_err(|_| {
            download_request_error(
                ErrorCode::InvalidArgument,
                "Choose a valid HTTP or HTTPS download link.",
            )
        })?;
        if !url.as_str().starts_with("http://") && !url.as_str().starts_with("https://") {
            return Err(download_request_error(
                ErrorCode::InvalidArgument,
                "Downloads need an HTTP or HTTPS link.",
            ));
        }
        if self.as_ref().get_ref().current_download_request().is_some() {
            return Err(download_request_error(
                ErrorCode::Busy,
                "Another download is being prepared. Wait for it or cancel the request in Downloads.",
            ));
        }
        if !self.as_ref().get_ref().stream_download_available() {
            return Err(download_request_error(
                ErrorCode::Busy,
                "Downloads are busy in this window. Wait for a transfer to finish or stop, then retry.",
            ));
        }
        finish_pending_download_operation(self.as_mut().rust_mut().as_mut().get_mut(), "cancelled");
        let rust = self.as_ref().get_ref().rust();
        let state = rust
            .state
            .as_ref()
            .filter(|state| {
                state.session_downloads_available()
                    && captured_target_is_current(Some(state), target)
            })
            .ok_or_else(|| {
                download_request_error(
                    ErrorCode::StaleTarget,
                    "The page changed before the download started. Try again.",
                )
            })?;
        let retry = retry
            .map(|id| state.reserve_session_download_retry(id))
            .transpose()
            .map_err(|error| {
                use ferric_browser_application::DownloadSessionError;
                let code = match error {
                    DownloadSessionError::RetryPending
                    | DownloadSessionError::Capacity
                    | DownloadSessionError::ProfileUnavailable => ErrorCode::Busy,
                    _ => ErrorCode::Engine,
                };
                download_request_error(code, error.to_string())
            })?;
        let token = format!("download-{}", Uuid::new_v4());
        let operation_id = format!("op-{}", Uuid::new_v4());
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(operation_id.clone(), "download preparation pending".into());
        self.as_mut().rust_mut().as_mut().get_mut().pending_download = Some(PendingDownload {
            token: token.clone(),
            operation_id: operation_id.clone(),
            target,
            url: url.to_string(),
            issued: false,
            created_at: Instant::now(),
            retry,
            visited: std::collections::BTreeSet::new(),
            last_url: None,
        });
        self.set_status_text(QString::from(status));
        Ok(
            serde_json::json!({"status":"accepted", "token":token, "operation_id":operation_id, "url":safe_ipc_url(url.as_str())}),
        )
    }

    pub(super) fn queue_hint_download_request(
        mut self: Pin<&mut Self>,
        target: Target,
        url: &str,
    ) -> Value {
        match self
            .as_mut()
            .queue_download_request(target, url, "Preparing download…")
        {
            Ok(mut result) => {
                result["action"] = Value::String("download".into());
                result["action_id"] = Value::String("browser.link.download".into());
                result["kind"] = Value::String("link".into());
                result
            }
            Err(error) => {
                self.set_status_text(QString::from(error.user_message()));
                serde_json::json!({"error":"download-request-rejected"})
            }
        }
    }

    pub(super) fn take_download_request(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().set_download_request_token(QString::default());
        self.as_mut().set_download_request_url(QString::default());
        if self.as_ref().get_ref().current_download_request().is_none() {
            finish_pending_download_operation(
                self.as_mut().rust_mut().as_mut().get_mut(),
                "cancelled",
            );
            return false;
        }
        let mut rust = self.as_mut().rust_mut();
        let request = rust
            .as_mut()
            .get_mut()
            .pending_download
            .as_mut()
            .expect("validated request");
        if request.issued {
            return false;
        }
        request.issued = true;
        let (token, url) = (request.token.clone(), request.url.clone());
        drop(rust);
        self.as_mut()
            .set_download_request_token(QString::from(token));
        self.set_download_request_url(QString::from(url));
        true
    }

    pub(super) fn accept_download_request(
        mut self: Pin<&mut Self>,
        token: &QString,
        id: &QString,
    ) -> bool {
        if self.as_ref().get_ref().download_request_source_index(token) < 0
            || !self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .is_some_and(|state| state.owns_session_download_offer(&id.to_string()))
        {
            return false;
        }
        let Some(mut request) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_download
            .take()
        else {
            return false;
        };
        if let Some(retry) = request.retry.as_mut()
            && let Err(error) = retry.bind(&id.to_string())
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .operation_states
                .insert(
                    request.operation_id.clone(),
                    "failed (download offer refused)".into(),
                );
            self.set_status_text(QString::from(error.to_string()));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(
                request.operation_id.clone(),
                "completed: download offered".into(),
            );
        self.set_status_text(QString::from("Download requested"));
        true
    }

    fn finish_download_request(mut self: Pin<&mut Self>, token: &QString, cancelled: bool) -> bool {
        if self
            .as_ref()
            .rust()
            .pending_download
            .as_ref()
            .is_none_or(|request| request.token != token.to_string())
        {
            return false;
        }
        let current = self
            .as_ref()
            .rust()
            .pending_download
            .as_ref()
            .is_some_and(|request| {
                captured_target_is_current(self.as_ref().rust().state.as_ref(), request.target)
            });
        finish_pending_download_operation(
            self.as_mut().rust_mut().as_mut().get_mut(),
            if cancelled {
                "cancelled"
            } else {
                "failed (download could not start)"
            },
        );
        if current {
            self.set_status_text(QString::from(if cancelled {
                "Download request cancelled"
            } else {
                "The download couldn't start. Open the link to check access and the connection, then retry."
            }));
        }
        true
    }

    pub(super) fn discard_download_request(mut self: Pin<&mut Self>, token: &QString) -> bool {
        if self
            .as_ref()
            .rust()
            .pending_download
            .as_ref()
            .is_none_or(|request| request.token != token.to_string())
        {
            return false;
        }
        finish_pending_download_operation(
            self.as_mut().rust_mut().as_mut().get_mut(),
            "failed (download offer refused)",
        );
        true
    }

    pub(super) fn fail_download_request(self: Pin<&mut Self>, token: &QString) -> bool {
        self.finish_download_request(token, false)
    }

    pub(super) fn cancel_download_request(self: Pin<&mut Self>, token: &QString) -> bool {
        self.finish_download_request(token, true)
    }
}
