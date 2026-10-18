use super::{
    CxxQtType, Duration, Instant, Pin, QString, QStringList, Value, diagnostic_report, diagnostics,
    diagnostics_presentation, qobject, query_empty_object_or_null,
};

const REPORT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DEFERRED_REPORTS: usize = 16;

pub(super) struct PendingDiagnostic {
    request: ferric_browser_ipc::PendingRequest,
    key: diagnostic_report::ReportKey,
    started: Instant,
}

impl qobject::BrowserUi {
    pub(super) fn sync_profile_startup_notice(mut self: Pin<&mut Self>) {
        let notice = QString::from(self.as_ref().rust().profile_startup.notice());
        self.as_mut().set_profile_startup_notice(notice);
    }

    pub(super) fn profile_startup_details(&self) -> QString {
        QString::from(self.rust().profile_startup.local_details())
    }

    // The QObject receiver is required by Qt's scalar invokable contract.
    #[allow(clippy::unused_self)]
    pub(super) fn engine_update_notice(&self) -> QStringList {
        let runtime = serde_json::json!({"value": super::qt_webengine_version()});
        let policy = diagnostics::engine_update_policy(&runtime);
        let (status, notice) =
            diagnostics_presentation::engine_notice(policy["status"].as_str().unwrap_or_default());
        [status, notice].into_iter().map(QString::from).collect()
    }

    pub(super) fn diagnostics_key(&self) -> diagnostic_report::ReportKey {
        diagnostic_report::ReportKey {
            owner: self.rust().session_id,
            configuration: self.rust().config_revision,
        }
    }

    pub(super) fn request_diagnostics(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().poll_diagnostics();
        if self.as_ref().rust().diagnostics_pending {
            return false;
        }
        self.as_mut().cancel_diagnostics();
        let input = self.as_ref().capture_diagnostic_input();
        let key = input.key;
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .diagnostics_collector
            .request(input, true);
        match result {
            Ok(ticket) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .diagnostics_review = Some((ticket, key, Instant::now()));
                self.as_mut().set_diagnostics_pending(true);
                self.runtime_work_available();
                true
            }
            Err(error) => {
                self.fail_diagnostics(&error);
                false
            }
        }
    }

    pub(super) fn cancel_diagnostics(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.diagnostics_review = None;
            this.diagnostics_preview_ready = false;
            this.diagnostics_preview_payload = None;
        }
        self.as_mut().set_diagnostics_available(false);
        self.as_mut().set_diagnostics_pending(false);
        self.as_mut().set_diagnostics_preview(QString::default());
        self.as_mut()
            .set_diagnostics_summary(QStringList::default());
        self.set_diagnostics_error(QString::default());
    }

    fn fail_diagnostics(mut self: Pin<&mut Self>, error: &str) {
        self.as_mut().cancel_diagnostics();
        self.set_diagnostics_error(QString::from(error.chars().take(4096).collect::<String>()));
    }

    pub(super) fn poll_diagnostics(mut self: Pin<&mut Self>) {
        let key = self.as_ref().diagnostics_key();
        let review = self.as_ref().rust().diagnostics_review;
        if let Some((_, owner, started)) = review {
            if owner != key {
                self.as_mut().fail_diagnostics("The profile or settings changed while preparing the report. Refresh to try again.");
            } else if started.elapsed() >= REPORT_TIMEOUT {
                self.as_mut().fail_diagnostics(
                    "Diagnostic collection did not finish within 15 seconds. Refresh to try again.",
                );
            }
        }
        let completion = {
            let mut rust = self.as_mut().rust_mut();
            let collector = &mut rust.as_mut().get_mut().diagnostics_collector;
            collector.select(key);
            collector.poll()
        };
        let Some(completion) = completion else {
            return;
        };
        let expected = self.as_ref().rust().diagnostics_review;
        if !expected.is_some_and(|(ticket, owner, _)| {
            ticket == completion.ticket && owner == completion.key
        }) {
            return;
        }
        match completion.result {
            Ok(report) => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.diagnostics_review = None;
                    this.diagnostics_preview_payload = Some(report.payload);
                    this.diagnostics_preview_ready = true;
                }
                self.as_mut()
                    .set_diagnostics_preview(QString::from(report.preview));
                self.as_mut()
                    .set_diagnostics_summary(report.summary.iter().map(QString::from).collect());
                self.as_mut().set_diagnostics_available(true);
                self.set_diagnostics_pending(false);
            }
            Err(error) => self.fail_diagnostics(&error),
        }
    }

    /// GUI IPC returns current in-memory facts plus the last completed probe
    /// snapshot. A first read reports pending facts and starts bounded collection.
    pub(super) fn ipc_diagnostics(self: Pin<&mut Self>, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "diagnostics.get")?;
        Ok(self.diagnostic_snapshot_value(true))
    }

    fn diagnostic_snapshot_value(mut self: Pin<&mut Self>, request: bool) -> Value {
        self.as_mut().poll_diagnostics();
        let input = self.as_ref().capture_diagnostic_input();
        let live = input.live.clone();
        let fallback = {
            let mut rust = self.as_mut().rust_mut();
            let collector = &mut rust.as_mut().get_mut().diagnostics_collector;
            collector.select(input.key);
            collector
                .cached()
                .is_none()
                .then(|| input.pending_snapshot())
        };
        let (cached, collecting, error) = {
            let mut rust = self.as_mut().rust_mut();
            let collector = &mut rust.as_mut().get_mut().diagnostics_collector;
            let failure = if request {
                collector.request(input, false).err()
            } else {
                None
            };
            (
                collector.cached().cloned(),
                collector.is_collecting(),
                collector.failure().map(str::to_owned).or(failure),
            )
        };
        let mut snapshot = cached
            .as_ref()
            .map(|report| report.snapshot.clone())
            .or(fallback)
            .unwrap_or(Value::Null);
        diagnostic_report::merge_live(&mut snapshot, &live);
        if let Some(error) = error {
            snapshot["collection"] = serde_json::json!({"status":"unavailable","reason":error});
        } else if collecting {
            snapshot["collection"]["status"] = serde_json::json!(if cached.is_some() {
                "refreshing"
            } else {
                "pending"
            });
        }
        snapshot["collection"]["live_captured_at"] =
            serde_json::json!(ferric_browser_storage::unix_timestamp());
        snapshot
    }

    pub(super) fn defer_diagnostic_request(
        mut self: Pin<&mut Self>,
        request: ferric_browser_ipc::PendingRequest,
    ) {
        if self.as_ref().rust().pending_diagnostic_requests.len() >= MAX_DEFERRED_REPORTS {
            let response = super::ipc_failure(
                &request.request().id,
                "E_BUSY",
                "Diagnostic collection is busy. Retry shortly.",
            );
            let _ = request.respond(&response);
            return;
        }
        let key = self.as_ref().diagnostics_key();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_diagnostic_requests
            .push(PendingDiagnostic {
                request,
                key,
                started: Instant::now(),
            });
    }

    pub(super) fn flush_diagnostic_requests(mut self: Pin<&mut Self>) {
        let key = self.as_ref().diagnostics_key();
        let pending = std::mem::take(
            &mut self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_diagnostic_requests,
        );
        let mut waiting = Vec::new();
        for entry in pending {
            let message = if entry.key != key {
                Some("The profile or settings changed. Retry diagnostics.")
            } else if entry.started.elapsed() >= REPORT_TIMEOUT {
                Some("Diagnostic collection timed out. Retry the request.")
            } else {
                None
            };
            if let Some(message) = message {
                let response = super::ipc_failure(&entry.request.request().id, "E_ENGINE", message);
                let _ = entry.request.respond(&response);
            } else {
                waiting.push(entry);
            }
        }
        if waiting.is_empty() {
            return;
        }
        let snapshot = self.as_mut().diagnostic_snapshot_value(false);
        let status = snapshot
            .pointer("/collection/status")
            .and_then(Value::as_str);
        if matches!(status, Some("pending" | "refreshing")) {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_diagnostic_requests = waiting;
            return;
        }
        for entry in waiting {
            let response = match status {
                Some("complete") => {
                    super::Response::success(entry.request.request().id.clone(), snapshot.clone())
                }
                _ => super::ipc_failure(
                    &entry.request.request().id,
                    "E_ENGINE",
                    "The diagnostic report couldn't be prepared. Retry the request.",
                ),
            };
            let _ = entry.request.respond(&response);
        }
    }
}
