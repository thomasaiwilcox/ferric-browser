use super::{
    BrowserApplication, CxxQtType, NativeStashCommit, OperationId, OperationState, Pin, QString,
    QStringList, TabId, VersioningEffects, VersioningEngineEffect, qobject, stash_refusal_text,
    stash_unknown_text, versioning_status_text,
};
use ferric_browser_application::versioning::PageCaptureFailurePolicy;

impl qobject::BrowserUi {
    /// Returns the source-of-truth per-page MHTML byte ceiling as text so QML
    /// can cancel an oversized native save without duplicating the limit.
    // CXX-Qt exposes this as an invokable QObject method, so its receiver is
    // required by the bridge even though the constant has no instance data.
    #[allow(clippy::unused_self)]
    pub(crate) fn native_page_capture_archive_limit_bytes(self: Pin<&mut Self>) -> QString {
        QString::from(ferric_browser_core::versioning::limits::MAX_ARCHIVE_BYTES.to_string())
    }

    /// Exposes only routing IDs for one admitted, application-owned `Start`.
    /// QML must check the exact native source before acknowledging it.
    pub(crate) fn pending_native_stash_start(mut self: Pin<&mut Self>) -> QStringList {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .claim_native_stash_start()
            .map_or_else(QStringList::default, |(operation, target)| {
                [operation.to_string(), target.tab.to_string()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            })
    }

    /// Called only after the native host found the same visible, active source
    /// by the admitted tab ID. It advances validation, not freeze authority.
    pub(crate) fn acknowledge_native_stash_start(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        source_tab_id: &QString,
    ) -> bool {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some((operation, target)) = this.native_stash_bridge.start() else {
            return false;
        };
        operation.to_string() == operation_id.to_string()
            && target.tab.to_string() == source_tab_id.to_string()
            && this.advance_native_stash_start(operation, target)
    }

    /// Fail-before-mutation for a missing or stale native source. Effects
    /// return through the normal revision-aware application/Qt bridge.
    pub(crate) fn refuse_native_stash_start(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let (previous_revision, effects) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some((operation, _)) = this.native_stash_bridge.start() else {
                return false;
            };
            if operation.to_string() != operation_id.to_string() {
                return false;
            }
            let Some(previous_revision) =
                Option::as_ref(&this.state).map(BrowserApplication::revision)
            else {
                return false;
            };
            let Ok(effects) = this.reject_native_stash_start(operation) else {
                return false;
            };
            (previous_revision, effects)
        };
        self.as_mut()
            .accept_native_stash_effects(previous_revision, effects);
        self.set_status_text(QString::from(
            "Live stash refused: native source unavailable",
        ));
        true
    }

    /// Only the exact active preview and fixed application copy cross to QML.
    /// An operation ID is routing identity, never a consent permit.
    pub(crate) fn pending_native_stash_disclosure(mut self: Pin<&mut Self>) -> QStringList {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_stash_disclosure_for_display()
            .map_or_else(QStringList::default, |(operation, copy)| {
                [operation.to_string(), copy.to_owned()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            })
    }

    /// A direct native UI action may request the exact active-page archive
    /// disclosure. QML receives no URL, title, scope selector or permit.
    pub(crate) fn request_native_page_capture_consent(
        mut self: Pin<&mut Self>,
        editable: bool,
        selection: bool,
    ) -> bool {
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .request_native_page_capture_disclosure(editable, selection);
        let requested = match result {
            Ok((_, previous_revision, runtime_effects)) => {
                let qt_effects = self
                    .as_mut()
                    .accept_runtime_effects(previous_revision, runtime_effects);
                self.as_mut().set_pending_engine_action(&qt_effects);
                true
            }
            Err(mut failure) => {
                if let Some((previous_revision, runtime_effects)) =
                    failure.take_prepared_runtime_effects()
                {
                    let qt_effects = self
                        .as_mut()
                        .accept_runtime_effects(previous_revision, runtime_effects);
                    self.as_mut().set_pending_engine_action(&qt_effects);
                }
                false
            }
        };
        self.as_mut().set_status_text(QString::from(if requested {
            "Review the page archive disclosure"
        } else {
            "Page archive consent is unavailable for this page"
        }));
        requested
    }

    /// The native window action has a distinct strict/partial consent choice.
    /// Rust derives the window and page membership; QML supplies no page IDs.
    pub(crate) fn request_native_window_page_capture_consent(
        mut self: Pin<&mut Self>,
        editable: bool,
        selection: bool,
        allow_partial_pages: bool,
    ) -> bool {
        let policy = if allow_partial_pages {
            PageCaptureFailurePolicy::AllowPartialPages
        } else {
            PageCaptureFailurePolicy::Strict
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .request_native_window_page_capture_disclosure(editable, selection, policy);
        let requested = match result {
            Ok((_, previous_revision, runtime_effects)) => {
                let qt_effects = self
                    .as_mut()
                    .accept_runtime_effects(previous_revision, runtime_effects);
                self.as_mut().set_pending_engine_action(&qt_effects);
                true
            }
            Err(mut failure) => {
                if let Some((previous_revision, runtime_effects)) =
                    failure.take_prepared_runtime_effects()
                {
                    let qt_effects = self
                        .as_mut()
                        .accept_runtime_effects(previous_revision, runtime_effects);
                    self.as_mut().set_pending_engine_action(&qt_effects);
                }
                false
            }
        };
        self.as_mut().set_status_text(QString::from(if requested {
            "Review the window archive disclosure"
        } else {
            "Window archive consent is unavailable for these pages"
        }));
        requested
    }

    /// Only fixed-copy Rust projection crosses into the native modal.
    pub(crate) fn pending_native_page_capture_disclosure(mut self: Pin<&mut Self>) -> QStringList {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_page_capture_disclosure_for_display()
            .map_or_else(QStringList::default, |(operation, copy)| {
                [operation.to_string(), copy.to_owned()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            })
    }

    /// Consumes native acceptance exactly once and schedules the bounded
    /// storage reservation. No boolean supplied by QML becomes authority.
    pub(crate) fn accept_native_page_capture_disclosure(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        let accepted = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .accept_native_page_capture_disclosure(operation)
            .is_ok();
        self.as_mut().set_status_text(QString::from(if accepted {
            "Page archive consent accepted; preparing a private capture destination"
        } else {
            "Page archive consent expired or could not be reserved"
        }));
        accepted
    }

    pub(crate) fn decline_native_page_capture_disclosure(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        let declined = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .decline_native_page_capture_disclosure(operation)
            .is_ok();
        if declined {
            self.as_mut()
                .set_status_text(QString::from("Page archive request cancelled"));
        }
        declined
    }

    /// Emits a one-shot operation/tab route after the file worker has created
    /// and secured the exact capture destination.
    pub(crate) fn pending_native_page_capture_save(mut self: Pin<&mut Self>) -> QStringList {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_native_page_capture_save()
            .map_or_else(QStringList::default, |(operation, tab)| {
                [operation.to_string(), tab.to_string()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            })
    }

    /// Projects only the bounded one-use collector challenge. The challenge
    /// contains no owner, filesystem path, consent permit or replay authority.
    pub(crate) fn native_page_capture_sidecar_request(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        tab_id: &QString,
    ) -> QStringList {
        let (Ok(operation), Some(tab)) = (
            operation_id.to_string().parse::<OperationId>(),
            TabId::from_display(&tab_id.to_string()),
        ) else {
            return QStringList::default();
        };
        let mut rust = self.as_mut().rust_mut();
        let rust = rust.as_mut().get_mut();
        match rust.native_page_capture_sidecar_request(operation, tab) {
            Some(request) => [
                request.expected_origin,
                request.expected_route,
                request.route_fingerprint,
                request.capture_fingerprint,
                request.nonce,
                request.include_editable.to_string(),
                request.include_selection.to_string(),
                request.include_media.to_string(),
            ]
            .into_iter()
            .map(QString::from)
            .collect(),
            None if rust.media_fingerprint_key_is_pending() => {
                [QString::from("pending-media-key")].into_iter().collect()
            }
            None => QStringList::default(),
        }
    }

    /// Reports only whether profile-key loading is still pending, ready, or
    /// unavailable. No key material or profile identity crosses into QML.
    pub(crate) fn native_page_capture_media_key_status(mut self: Pin<&mut Self>) -> QString {
        let status = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .media_fingerprint_key_status();
        QString::from(status)
    }

    /// Feeds one helper result back into Rust's exact pending capture. Raw
    /// content is never reflected to QML, status text, logs or Debug output.
    pub(crate) fn complete_native_page_capture_sidecar(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        sidecar_json: &QString,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .complete_native_page_capture_sidecar(operation, sidecar_json.to_string())
    }

    /// Returns the exact private stage path only to the trusted QML save
    /// adapter after a one-use Rust-side route claim.
    pub(crate) fn claim_native_page_capture_save(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        tab_id: &QString,
        observed_url: &QString,
    ) -> QString {
        let operation_text = operation_id.to_string();
        let tab_text = tab_id.to_string();
        let (Ok(operation), Some(tab)) = (
            operation_text.parse::<OperationId>(),
            TabId::from_display(&tab_text),
        ) else {
            return QString::default();
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .claim_native_page_capture_save(operation, tab, &observed_url.to_string())
            .map_or_else(QString::default, QString::from)
    }

    pub(crate) fn cancel_native_page_capture_request(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .cancel_native_page_capture_request(operation)
    }

    pub(crate) fn cancel_pending_native_page_capture_for_shutdown(
        mut self: Pin<&mut Self>,
    ) -> bool {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .cancel_pending_native_page_capture_for_shutdown()
    }

    pub(crate) fn has_native_page_capture_request(mut self: Pin<&mut Self>) -> bool {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .has_native_page_capture_request()
    }

    pub(crate) fn note_native_page_capture_download_requested(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        download_id: &QString,
        observed_path: &QString,
        is_save_page: bool,
        is_mhtml: bool,
        permit_matches: bool,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        let download_id = download_id.to_string();
        let observed_path = observed_path.to_string();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .note_native_page_capture_download_requested(
                operation,
                &download_id,
                &observed_path,
                is_save_page,
                is_mhtml,
                permit_matches,
            )
    }

    pub(crate) fn finish_native_page_capture_download(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        download_facts_json: &QString,
    ) -> bool {
        let Ok(operation) = operation_id.to_string().parse::<OperationId>() else {
            return false;
        };
        let facts_json = download_facts_json.to_string();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .finish_native_page_capture_download_json(operation, &facts_json)
    }

    /// A native terminal event may advance only the exact indexed page of a
    /// separately consented partial-window capture. Unknown reason strings
    /// never become manifest data.
    pub(crate) fn fail_native_window_capture_page(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
        tab_id: &QString,
        reason: &QString,
        observed_path: &QString,
        download_id: &QString,
    ) -> bool {
        use ferric_browser_storage::versioning::ManifestPageArchiveFailure as Failure;

        let (Ok(operation), Some(tab)) = (
            operation_id.to_string().parse::<OperationId>(),
            TabId::from_display(&tab_id.to_string()),
        ) else {
            return false;
        };
        let reason = match reason.to_string().as_str() {
            "cancelled" => Failure::Cancelled,
            "interrupted" => Failure::Interrupted,
            "empty-archive" => Failure::EmptyArchive,
            "native-view-lost" => Failure::NativeViewLost,
            "document-changed" => Failure::DocumentChanged,
            "timeout" => Failure::Timeout,
            "storage-full" => Failure::StorageFull,
            "invalid-archive" => Failure::InvalidArchive,
            _ => return false,
        };
        let path = observed_path.to_string();
        let id = download_id.to_string();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .fail_native_window_capture_page(
                operation,
                tab,
                reason,
                (!path.is_empty()).then_some(path.as_str()),
                (!id.is_empty()).then_some(id.as_str()),
            )
    }

    /// A dismissal can only cancel the Rust-held preview for this operation.
    pub(crate) fn decline_native_stash_disclosure(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let (previous_revision, effects) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some((operation, _)) = this.native_stash_disclosure_for_display() else {
                return false;
            };
            if operation.to_string() != operation_id.to_string() {
                return false;
            }
            let Some(previous_revision) =
                Option::as_ref(&this.state).map(BrowserApplication::revision)
            else {
                return false;
            };
            let Ok(effects) = this.decline_native_stash_disclosure(operation) else {
                return false;
            };
            (previous_revision, effects)
        };
        self.as_mut()
            .accept_native_stash_effects(previous_revision, effects);
        true
    }

    /// Exact IDs for a logical reservation that still needs a native blank
    /// fallback. An empty list means there is no unacknowledged reservation.
    pub(crate) fn pending_native_stash_preparation(self: Pin<&mut Self>) -> QStringList {
        let pinned = self.as_ref();
        let this = pinned.rust();
        let Some(application) = this.state.as_ref() else {
            return QStringList::default();
        };
        let Some(operation) = this.native_stash_bridge.pending_operation() else {
            return QStringList::default();
        };
        if !application
            .versioning()
            .operation(operation)
            .is_some_and(|(_, state)| state == OperationState::Preparing)
        {
            return QStringList::default();
        }
        this.native_stash_bridge.pending_ids().map_or_else(
            QStringList::default,
            |(view, source, fallback)| {
                [view.to_string(), source.to_string(), fallback.to_string()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            },
        )
    }

    pub(crate) fn has_active_versioning_operations(self: Pin<&mut Self>) -> bool {
        let pinned = self.as_ref();
        let rust = pinned.rust();
        Option::as_ref(&rust.state)
            .is_some_and(|application| application.versioning().active_count() > 0)
    }

    pub(crate) fn has_pending_versioning_object_work(self: Pin<&mut Self>) -> bool {
        self.as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(BrowserApplication::has_pending_versioning_object_work)
    }

    /// QML calls this only after its read-only projection check found the
    /// exact same-profile blank fallback. It does not authorize a freeze.
    pub(crate) fn acknowledge_native_fallback_materialized(
        mut self: Pin<&mut Self>,
        view_id: &QString,
    ) -> bool {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(application) = this.state.as_ref() else {
            return false;
        };
        let Some((view, _, _)) = this.native_stash_bridge.pending_ids() else {
            return false;
        };
        view.to_string() == view_id.to_string()
            && this
                .native_stash_bridge
                .mark_materialized(application, view)
    }

    /// A failed native fallback or hold check cancels only this still-visible
    /// source reservation. QML invokes this before a successful native hold;
    /// any logical cleanup failure leaves the Rust authority for retry.
    pub(crate) fn abort_native_stash_preparation(
        mut self: Pin<&mut Self>,
        view_id: &QString,
    ) -> bool {
        let (previous_revision, effects) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.native_stash_bridge.pending.as_mut() else {
                return false;
            };
            if pending.preparation.view().to_string() != view_id.to_string() {
                return false;
            }
            if pending.native_held {
                return false;
            }
            // This callback is the host's failed-hold branch. Never route a
            // later terminal retry toward a native entry that was not made.
            pending.materialized = false;
            let Some(application) = Option::as_mut(&mut this.state) else {
                return false;
            };
            let previous_revision = application.revision();
            let Ok(effects) = application.abort_explicit_stash_preparation(pending.operation)
            else {
                return false;
            };
            this.native_stash_bridge.pending = None;
            (previous_revision, effects)
        };
        self.as_mut()
            .accept_native_stash_effects(previous_revision, effects);
        true
    }

    /// Called by the native host's bounded maintenance tick. The return ID
    /// is only a routing hint; the host still checks its exact `QObject` and
    /// the Rust ticket remains owned until actual Active acknowledgement.
    pub(crate) fn poll_native_stash_cleanup(mut self: Pin<&mut Self>) -> QString {
        let (previous_revision, effects, view) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(application) = Option::as_mut(&mut this.state) else {
                return QString::from("");
            };
            let previous_revision = application.revision();
            let effects = application.expire_versioning_operations();
            this.native_stash_bridge
                .clear_terminal_disclosure(application);
            this.native_stash_bridge.clear_terminal_preparation(
                application,
                &effects.cleanup_failed,
                &effects.native_cleanup_pending,
            );
            let view = this
                .native_stash_bridge
                .cleanup_view(application)
                .or_else(|| this.native_stash_bridge.pending_cleanup_view(application));
            (previous_revision, effects, view)
        };
        if !effects.is_empty() {
            self.as_mut()
                .accept_native_stash_effects(previous_revision, effects);
        }
        QString::from(view.map_or_else(String::new, |view| view.to_string()))
    }

    /// Result codes for the host's strict callback protocol: 1 means
    /// committed, 0 is a known refusal requiring native restoration, and -1
    /// means the host must keep ownership because the outcome is uncertain.
    pub(crate) fn confirm_native_frozen_stash(mut self: Pin<&mut Self>, view_id: &QString) -> i32 {
        let (previous_revision, outcome) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(view) = this.native_stash_bridge.view() else {
                return -1;
            };
            if view.to_string() != view_id.to_string() {
                return -1;
            }
            let Some(application) = Option::as_mut(&mut this.state) else {
                return -1;
            };
            let previous_revision = application.revision();
            (
                previous_revision,
                this.native_stash_bridge.confirm(application, view),
            )
        };
        match outcome {
            NativeStashCommit::Committed(effects) => {
                let needs_attention = self
                    .as_mut()
                    .accept_native_stash_effects(previous_revision, effects);
                self.set_status_text(QString::from(if needs_attention {
                    "Live stash frozen; operation cleanup needs attention"
                } else {
                    "Live stash frozen; work is volatile"
                }));
                1
            }
            NativeStashCommit::Refused(error) => {
                self.set_status_text(QString::from(stash_refusal_text(error)));
                0
            }
            NativeStashCommit::OutcomeUnknown(error) => {
                self.set_status_text(QString::from(stash_unknown_text(error)));
                -1
            }
        }
    }

    /// QML calls this only from `acknowledgeRestoredStash`, after its exact
    /// view reports Active again. Returning false keeps the native restoring
    /// record and this Rust ticket for another cleanup attempt.
    pub(crate) fn acknowledge_native_stash_restoration(
        mut self: Pin<&mut Self>,
        view_id: &QString,
    ) -> bool {
        let (previous_revision, result) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(application) = Option::as_mut(&mut this.state) else {
                return false;
            };
            let Some(view) = this
                .native_stash_bridge
                .view()
                .or_else(|| this.native_stash_bridge.pending_cleanup_view(application))
            else {
                return false;
            };
            if view.to_string() != view_id.to_string() {
                return false;
            }
            let previous_revision = application.revision();
            let result = if this.native_stash_bridge.stage.is_some() {
                this.native_stash_bridge
                    .acknowledge_restored(application, view)
            } else {
                this.native_stash_bridge
                    .acknowledge_held_restored(application, view)
            };
            (previous_revision, result)
        };
        let Ok(effects) = result else {
            self.set_status_text(QString::from("Live stash restoration remains pending"));
            return false;
        };
        let needs_attention = self
            .as_mut()
            .accept_native_stash_effects(previous_revision, effects);
        self.set_status_text(QString::from(if needs_attention {
            "Original view restored; operation cleanup needs attention"
        } else {
            "Original live view restored"
        }));
        true
    }

    pub(crate) fn accept_native_stash_effects(
        mut self: Pin<&mut Self>,
        previous_revision: u64,
        effects: VersioningEffects,
    ) -> bool {
        let VersioningEffects {
            runtime_effects,
            operation_effects,
            native_cleanup_pending,
            cleanup_failed,
        } = effects;
        let mut needs_attention = !native_cleanup_pending.is_empty() || !cleanup_failed.is_empty();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            needs_attention |= this.queue_native_operation_effects(operation_effects);
        }
        let qt_effects = self
            .as_mut()
            .accept_runtime_effects(previous_revision, runtime_effects);
        self.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&qt_effects);
        needs_attention
    }

    pub(crate) fn present_versioning_effect(
        mut self: Pin<&mut Self>,
        effect: &VersioningEngineEffect,
    ) {
        if let VersioningEngineEffect::SwitchResolved {
            branch,
            window,
            child,
        } = effect
        {
            let kind = if *child { "child" } else { "parent" };
            let target = if *child {
                branch.to_string()
            } else {
                window.to_string()
            };
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .pending_engine_actions
                .push_back(format!("versioning-branch-switch\t{kind}\t{target}"));
        }
        let read_request = match effect {
            VersioningEngineEffect::CommitListQueued { request_id }
            | VersioningEngineEffect::CommitInfoQueued { request_id }
            | VersioningEngineEffect::CommitDeletePreviewQueued { request_id }
            | VersioningEngineEffect::DiffQueued { request_id } => Some(*request_id),
            _ => None,
        };
        if let Some(request_id) = read_request {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().latest_versioning_read_request = Some(request_id);
        }
        self.as_mut()
            .set_status_text(QString::from(versioning_status_text(effect)));
    }
}
