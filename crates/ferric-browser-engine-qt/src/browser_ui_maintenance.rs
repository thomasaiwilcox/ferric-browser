use super::{
    Arc, AtomicBool, ConfigReloadWorker, ConfigWriteWorker, CxxQtType, EditorWriteWorker,
    HyprlandWorker, MAX_ACTION_AUDIT_RECORDS, NetworkPolicyWorker, Ordering, Pin,
    PortalProbeWorker, PrintWorker, ProfileDeleteWorker, ProfileListWorker, ProfilePreviewWorker,
    ReducedMotionProbeWorker, SystemFontScaleProbeWorker, Threading, action_audit_record,
    captured_target_is_current, elapsed_ms, has_pending_requests, install_pending_request_waker,
    ipc_failure, publish_ipc_event, qobject, remember_operation_stderr, subscribe_event_stream,
    take_pending_request,
};
use crate::{FailureKind, OperationUpdate};

impl qobject::BrowserUi {
    pub(super) fn refresh_operations(mut self: Pin<&mut Self>) {
        let completions = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.processes
                .spawn_completions
                .lock()
                .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let userscript_completions = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.processes
                .userscript_completions
                .lock()
                .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        for completion in completions {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .set_operation(completion.operation_id, completion.status);
        }
        for completion in userscript_completions {
            if self
                .as_ref()
                .rust()
                .operation(&completion.operation_id)
                .is_none_or(|record| record.state.is_terminal())
            {
                continue;
            }
            let mut status = completion.status;
            if let Some(action) = completion.action {
                status = if captured_target_is_current(
                    self.as_ref().rust().state.as_ref(),
                    completion.target,
                ) {
                    match self
                        .as_mut()
                        .execute_userscript_action(completion.target, action)
                    {
                        Ok(()) => OperationUpdate::completed("userscript action applied"),
                        Err(error) => OperationUpdate::failed(FailureKind::Engine, error),
                    }
                } else {
                    OperationUpdate::failed(FailureKind::StaleTarget, "userscript target changed")
                };
            }
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            remember_operation_stderr(this, &completion.operation_id, completion.stderr);
            this.set_operation(completion.operation_id, status);
        }
    }

    pub(super) fn record_action_audit(
        mut self: Pin<&mut Self>,
        action_id: &str,
        operation_id: &str,
        outcome: &str,
        category: Option<&str>,
    ) {
        let record = action_audit_record(action_id, operation_id, outcome, category);
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.action_audit.push(record.clone());
        if this.action_audit.len() > MAX_ACTION_AUDIT_RECORDS {
            let excess = this.action_audit.len() - MAX_ACTION_AUDIT_RECORDS;
            this.action_audit.drain(..excess);
        }
        let level = if matches!(outcome, "failed" | "rejected") {
            "error"
        } else {
            "info"
        };
        if let Some(sink) = this.structured_log.as_mut() {
            let _ = sink.record_action(level, action_id, operation_id, outcome, category);
        }
        publish_ipc_event(&mut this.ipc_sequence, "action.audit", record);
    }

    pub(super) fn activate_runtime_wake(mut self: Pin<&mut Self>) {
        self.as_mut().register_ipc_controller();
        let qt_thread = self.as_ref().get_ref().qt_thread();
        let wake_queued = Arc::new(AtomicBool::new(false));
        let callback_queued = Arc::clone(&wake_queued);
        install_pending_request_waker(Arc::new(move || {
            if callback_queued
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            let delivered = Arc::clone(&callback_queued);
            let failed = Arc::clone(&callback_queued);
            if qt_thread
                .queue(move |mut ui| {
                    delivered.store(false, Ordering::Release);
                    ui.as_mut().runtime_work_available();
                })
                .is_err()
            {
                failed.store(false, Ordering::Release);
            }
        }));
    }

    pub(super) fn maintenance_delay_ms(self: Pin<&mut Self>) -> i32 {
        let binding = self.as_ref();
        let rust = binding.rust();
        let worker_pending = rust
            .state
            .as_ref()
            .is_some_and(super::BrowserApplication::configuration_write_pending)
            || rust.configuration_io.pending_context_reload.is_some()
            || rust
                .configuration_io
                .context_reload_worker
                .as_ref()
                .is_some_and(super::ContextReloadWorker::is_pending)
            || rust
                .configuration_io
                .config_reload_worker
                .as_ref()
                .is_some_and(ConfigReloadWorker::is_pending)
            || rust
                .configuration_io
                .config_write_worker
                .as_ref()
                .is_some_and(ConfigWriteWorker::is_pending)
            || rust
                .print_worker
                .as_ref()
                .is_some_and(PrintWorker::is_pending)
            || rust
                .processes
                .editor_write_worker
                .as_ref()
                .is_some_and(EditorWriteWorker::is_pending)
            || rust
                .profile_delete_worker
                .as_ref()
                .is_some_and(ProfileDeleteWorker::is_pending)
            || rust.pending_profile_configuration.is_some()
            || rust
                .configuration_io
                .network_policy_worker
                .as_ref()
                .is_some_and(NetworkPolicyWorker::is_pending)
            || rust
                .hyprland_worker
                .as_ref()
                .is_some_and(HyprlandWorker::is_pending)
            || rust
                .portal_probe_worker
                .as_ref()
                .is_some_and(PortalProbeWorker::is_pending)
            || rust
                .reduced_motion_probe_worker
                .as_ref()
                .is_some_and(ReducedMotionProbeWorker::is_pending)
            || rust
                .font_scale_probe_worker
                .as_ref()
                .is_some_and(SystemFontScaleProbeWorker::is_pending)
            || rust
                .profile_list_worker
                .as_ref()
                .is_some_and(ProfileListWorker::is_pending)
            || rust
                .profile_preview_worker
                .as_ref()
                .is_some_and(ProfilePreviewWorker::is_pending)
            || rust
                .state
                .as_ref()
                .is_some_and(super::BrowserApplication::userscript_job_pending);
        let operation_pending = rust.has_active_operations()
            || rust.processes.pending_config_edit.is_some()
            || rust.processes.pending_editor.is_some()
            || rust.processes.pending_editor_write.is_some()
            || rust.library.switcher_library_index_building
            || !rust.library.session_restores.is_empty()
            || !rust.library.session_saves.is_empty()
            || rust.library.session_checkpoint_clear_pending
            || rust.library.storage_flush_pending;
        if worker_pending
            || operation_pending
            || rust.diagnostics_pending
            || !rust.diagnostic_work.pending_diagnostic_requests.is_empty()
        {
            return 50;
        }
        if rust.active_site_experiment.is_some()
            || rust.macro_key_started_ms.is_some()
            || !rust.binding_overlay.is_empty()
        {
            return 100;
        }
        // Private or unavailable profiles have no checkpoint destination.
        // Keep the draft dirty, but don't spin while there is nowhere to save.
        if rust.checkpoint.dirty && rust.session_path.is_some() {
            let elapsed = rust.checkpoint.dirty_since_ms.map_or(0, |since| {
                elapsed_ms(rust.binding_clock).saturating_sub(since)
            });
            return i32::try_from(500_u64.saturating_sub(elapsed).max(1)).unwrap_or(500);
        }
        if rust.configuration_io.config_watch.is_watching()
            || rust.configuration_io.theme_watch.is_watching()
            || rust.state.as_ref().is_some_and(|state| {
                state
                    .profile_snapshot()
                    .is_some_and(|profile| profile.durable)
            })
        {
            return 1_000;
        }
        -1
    }

    pub(super) fn poll_ipc(mut self: Pin<&mut Self>) {
        self.as_mut().flush_diagnostic_requests();
        self.as_mut().refresh_ipc_owner();
        for _ in 0..ferric_browser_ipc::MAX_PENDING_REQUESTS {
            let Some(pending) = take_pending_request() else {
                break;
            };
            self.as_mut().route_pending_ipc(pending);
        }
        if has_pending_requests() {
            self.as_mut().runtime_work_available();
        }
    }

    pub(super) fn deliver_pending_ipc(
        mut self: Pin<&mut Self>,
        pending: ferric_browser_ipc::PendingRequest,
        request: &super::Request,
    ) {
        let event_stream = if pending.request().method == "events.subscribe" {
            pending.clone_stream().map(Some)
        } else {
            Ok(None)
        };
        let event_types = if pending.request().method == "events.subscribe" {
            Self::ipc_event_filter(&pending.request().params)
        } else {
            Ok(None)
        };
        let mut response = self.as_mut().handle_ipc_request(request);
        self.as_mut().refresh_ipc_owner();
        if response.error.is_none()
            && let Ok(Some(stream)) = event_stream
            && let Ok(event_types) = event_types
            && let Err(error) = subscribe_event_stream(stream, event_types)
        {
            response = ipc_failure(&pending.request().id, "E_IO", error);
        }
        if pending.request().method == "diagnostics.get"
            && response.error.is_none()
            && response
                .result
                .as_ref()
                .and_then(|value| value.pointer("/collection/status"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|status| matches!(status, "pending" | "refreshing"))
        {
            self.as_mut().defer_diagnostic_request(pending);
            return;
        }
        let _ = pending.respond(&response);
    }
}
