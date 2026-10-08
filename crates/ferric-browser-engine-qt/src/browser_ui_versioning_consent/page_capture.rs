use super::super::parse_native_page_capture_download_facts;
use super::{
    BrowserUiRust, CaptureInterval, CaptureMonotonicInterval, CaptureStageTarget, CapturedOwner,
    InstalledObject, ManifestMutationEvidence, ManifestPageCapture, ManifestPageCategory,
    NativeProfileOwner, NativeStashReservation, OperationId, PageCaptureAuthorization,
    PageCaptureSidecarRequest, PageStateSidecar, PendingNativeDisclosure, RedactedPreview,
    StashError, StashObservation, StorageOperationError, StorageTicket, VersioningEffects,
    canonical_origin, capture_fingerprint_for_request, native_page_capture_unix_millis,
    page_route_for_url, route_fingerprint_for_url,
};
use ferric_browser_application::versioning::PageCaptureProvenance;
use ferric_browser_storage::versioning::ManifestPageArchiveFailure;

impl BrowserUiRust {
    /// Creates one Rust-held, capture-local challenge for the bounded QML
    /// collector. The projected strings are correlation inputs only; the
    /// application revalidates every returned sidecar against this exact
    /// challenge and current captured owner/document.
    pub(in crate::browser_ui_versioning) fn native_page_capture_sidecar_request(
        &mut self,
        operation: OperationId,
        tab: ferric_browser_core::TabId,
    ) -> Option<PageCaptureSidecarRequest> {
        if self.pending_native_page_capture_save() != Some((operation, tab))
            || self
                .native_stash_bridge
                .page_capture_collector_request
                .is_some()
            || self.native_stash_bridge.page_capture_sidecar_json.is_some()
        {
            return None;
        }
        let stored_page = &self
            .native_stash_bridge
            .page_capture_manifest
            .as_ref()?
            .windows
            .first()?
            .pages
            .get(self.native_stash_bridge.page_capture_page_index)?
            .safe_url_or_placeholder;
        // A POST result can be archived with consent while its durable
        // navigation descriptor must remain a non-replayable placeholder.
        // Use the reducer's exact current tab URL only as transient capture
        // correlation; never promote it into the stored manifest.
        let current_url = self
            .state
            .as_ref()?
            .runtime_view()
            .tabs()
            .get(&tab)?
            .url
            .as_deref()?;
        let expected_origin = canonical_origin(current_url)?;
        let expected_route = page_route_for_url(current_url)?;
        let route_fingerprint = route_fingerprint_for_url(current_url)?;
        if let ferric_browser_storage::versioning::ManifestSafeUrl::Replayable(stored_url) =
            stored_page
        {
            if route_fingerprint_for_url(stored_url).as_deref() != Some(route_fingerprint.as_str())
            {
                return None;
            }
        }
        let authorization = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()?;
        // Two tabs can share one route. Distinct window-member challenges
        // prevent their otherwise identical sidecars from being exchanged.
        let nonce = if authorization.scope().members().len() > 1 {
            uuid::Uuid::new_v4().simple().to_string()
        } else {
            operation.as_uuid().simple().to_string()
        };
        let capture_fingerprint = capture_fingerprint_for_request(&route_fingerprint, &nonce)?;
        let started_at_ms = native_page_capture_unix_millis()?;
        let include_editable = authorization.editable();
        let include_selection = authorization.selection();
        let media_authorized = authorization.media();
        let include_media = media_authorized && self.request_media_fingerprint_key(true);
        if media_authorized && !include_media && self.media_fingerprint_key_is_pending() {
            // Do not dispatch a collector that can observe media URLs while
            // profile-key loading is still in flight. Other sidecar categories
            // may proceed without media if no key can be loaded.
            return None;
        }
        self.native_stash_bridge.page_capture_collector_request =
            Some(super::super::PageCaptureCollectorRequest {
                expected_origin: expected_origin.clone(),
                route_fingerprint: route_fingerprint.clone(),
                capture_fingerprint: capture_fingerprint.clone(),
                nonce: nonce.clone(),
                started_at_ms,
                include_media,
            });
        self.native_stash_bridge.page_capture_monotonic_started_at =
            Some(std::time::Instant::now());
        Some(PageCaptureSidecarRequest {
            expected_origin,
            expected_route,
            route_fingerprint,
            capture_fingerprint,
            nonce,
            include_editable,
            include_selection,
            include_media,
        })
    }

    /// Accepts one bounded helper result only when it matches the Rust-issued
    /// nonce and the exact route fingerprint captured in the manifest. The
    /// result must arrive before archive save starts and stays one-shot after
    /// its bytes are handed to the file worker.
    /// Malformed/late results are omitted while archive capture may continue.
    pub fn complete_native_page_capture_sidecar(
        &mut self,
        operation: OperationId,
        json: String,
    ) -> bool {
        if !self.page_capture_route_is_current(operation)
            || self.native_stash_bridge.page_capture_save_started
            || self.native_stash_bridge.page_capture_sidecar_json.is_some()
            || self
                .native_stash_bridge
                .page_capture_sidecar_interval
                .is_some()
        {
            return false;
        }
        let Some(request) = self
            .native_stash_bridge
            .page_capture_collector_request
            .as_ref()
            .cloned()
        else {
            return false;
        };
        let Some(normalized) = self.normalize_media_page_state(&json, true) else {
            return false;
        };
        let bytes = normalized.json.into_bytes();
        if bytes.is_empty() || bytes.len() > ferric_browser_core::versioning::MAX_PAGE_SIDECAR_BYTES
        {
            return false;
        }
        let Ok(sidecar) = PageStateSidecar::parse_json(&bytes) else {
            return false;
        };
        if !sidecar.qualifies_for(
            &request.expected_origin,
            &request.route_fingerprint,
            &request.capture_fingerprint,
            &request.nonce,
        ) {
            return false;
        }
        let Some(finished_at_ms) =
            native_page_capture_unix_millis().filter(|finished| *finished >= request.started_at_ms)
        else {
            return false;
        };
        let Some(monotonic_started_at) = self.native_stash_bridge.page_capture_monotonic_started_at
        else {
            return false;
        };
        let Some(monotonic_finished_offset_ms) =
            page_capture_monotonic_offset_ms(monotonic_started_at)
        else {
            return false;
        };
        self.native_stash_bridge.page_capture_sidecar_interval = Some(CaptureInterval {
            started_at_ms: request.started_at_ms,
            finished_at_ms,
        });
        self.native_stash_bridge
            .page_capture_sidecar_monotonic_interval = Some(CaptureMonotonicInterval {
            started_offset_ms: 0,
            finished_offset_ms: monotonic_finished_offset_ms,
        });
        self.native_stash_bridge.page_capture_sidecar_summary = Some(sidecar.summary());
        self.native_stash_bridge.page_capture_sidecar_json = Some(bytes);
        true
    }

    /// Claims the exact staging path once. The path is returned only to the
    /// native adapter after rechecking owner, active tab and document token.
    pub fn claim_native_page_capture_save(
        &mut self,
        operation: OperationId,
        tab: ferric_browser_core::TabId,
        observed_url: &str,
    ) -> Option<String> {
        if self.native_stash_bridge.page_capture_save_started
            || self.pending_native_page_capture_save() != Some((operation, tab))
            || !self.page_capture_route_matches_captured(operation, observed_url)
        {
            return None;
        }
        let path = self
            .native_stash_bridge
            .page_capture_stage_target
            .as_ref()?
            .path()
            .to_str()?
            .to_owned();
        let started_at_ms = native_page_capture_unix_millis()?;
        // Page-state collection is optional: native archive capture must still
        // proceed when no sidecar request was issued or its evidence is absent.
        // In that case, anchor archive timing at the exact save claim.
        let monotonic_started_at = *self
            .native_stash_bridge
            .page_capture_monotonic_started_at
            .get_or_insert_with(std::time::Instant::now);
        let monotonic_started_offset_ms = page_capture_monotonic_offset_ms(monotonic_started_at)?;
        if monotonic_started_offset_ms > 5 * 60 * 1_000 {
            return None;
        }
        self.native_stash_bridge.page_capture_save_started = true;
        self.native_stash_bridge.page_capture_archive_started_at_ms = Some(started_at_ms);
        self.native_stash_bridge
            .page_capture_archive_monotonic_started_offset_ms = Some(monotonic_started_offset_ms);
        Some(path)
    }

    fn page_capture_route_matches_captured(
        &self,
        operation: OperationId,
        observed_url: &str,
    ) -> bool {
        if !self.page_capture_route_is_current(operation) {
            return false;
        }
        let Some(tab) = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .and_then(|authorization| {
                authorization
                    .scope()
                    .members()
                    .get(self.native_stash_bridge.page_capture_page_index)
            })
            .map(|captured| captured.target().tab)
        else {
            return false;
        };
        let current_url = self
            .state
            .as_ref()
            .and_then(|application| application.runtime_view().tabs().get(&tab))
            .and_then(|current| current.url.as_deref());
        let current_route = current_url.and_then(route_fingerprint_for_url);
        let observed_route = route_fingerprint_for_url(observed_url);
        if let Some(request) = self
            .native_stash_bridge
            .page_capture_collector_request
            .as_ref()
        {
            current_route.as_deref() == Some(request.route_fingerprint.as_str())
                && observed_route.as_deref() == Some(request.route_fingerprint.as_str())
        } else {
            // Sidecar collection is optional, but the archive callback still
            // has to match the exact route of the currently authorized page.
            current_route.is_some() && current_route == observed_route
        }
    }

    /// Records the one public `WebEngine` page-save event that matches the
    /// previously claimed one-use permit and exact staging path. Qt Quick's
    /// public request does not provide a safe per-view origin in every case;
    /// the unpredictable path is the correlation token for this serialized
    /// capture request.
    pub fn note_native_page_capture_download_requested(
        &mut self,
        operation: OperationId,
        download_id: &str,
        observed_path: &str,
        is_save_page: bool,
        is_mhtml: bool,
        permit_matches: bool,
    ) -> bool {
        let Some(expected_operation) = self
            .native_stash_bridge
            .page_capture_stage_target
            .as_ref()
            .map(CaptureStageTarget::operation)
        else {
            return false;
        };
        let Some(expected_path) = self
            .native_stash_bridge
            .page_capture_stage_target
            .as_ref()
            .and_then(|target| target.path().to_str())
        else {
            return false;
        };
        if operation != expected_operation
            || !self.native_stash_bridge.page_capture_save_started
            || !self.page_capture_route_is_current(operation)
            || download_id.is_empty()
            || download_id.len() > 20
            || download_id.parse::<u64>().is_err()
            || observed_path != expected_path
            || !is_save_page
            || !is_mhtml
            || !permit_matches
            || self.native_stash_bridge.page_capture_download_id.is_some()
        {
            return false;
        }
        self.native_stash_bridge.page_capture_download_id = Some(download_id.to_owned());
        true
    }

    /// Installs only the exact correlated completed save. Any cancellation,
    /// interrupted state, stale document, changed path, or empty output
    /// cancels the operation and releases its reservation through the app.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_native_page_capture_download(
        &mut self,
        operation: OperationId,
        download_id: &str,
        observed_path: &str,
        is_save_page: bool,
        is_mhtml: bool,
        view_matches: bool,
        observed_url: &str,
        terminal_state: &str,
        received_bytes: i64,
        mutation_result: &str,
    ) -> bool {
        let mutation_evidence = match mutation_result {
            "observed-stable" => ManifestMutationEvidence::ObservedStable,
            "observed-changed" => ManifestMutationEvidence::ObservedChanged,
            "not-observed" => ManifestMutationEvidence::NotObserved,
            // A captured document/frame was replaced after its sidecar read.
            // Do not publish a mixed-document archive/page commit.
            "document-changed" => {
                self.cancel_native_page_capture(operation);
                return false;
            }
            _ => {
                self.cancel_native_page_capture(operation);
                return false;
            }
        };
        let exact = self
            .pending_native_page_capture_save_for_download()
            .is_some_and(|(expected_operation, expected_id)| {
                operation == expected_operation && download_id == expected_id
            })
            && self
                .native_stash_bridge
                .page_capture_stage_target
                .as_ref()
                .and_then(|target| target.path().to_str())
                == Some(observed_path)
            && is_save_page
            && is_mhtml
            && view_matches
            && self.page_capture_route_matches_captured(operation, observed_url)
            && terminal_state == "completed"
            && received_bytes > 0
            && received_bytes
                <= i64::try_from(ferric_browser_core::versioning::limits::MAX_ARCHIVE_BYTES)
                    .unwrap_or(i64::MAX)
            && self.page_capture_route_is_current(operation);
        if !exact {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let Some(started_at_ms) = self.native_stash_bridge.page_capture_archive_started_at_ms
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(finished_at_ms) =
            native_page_capture_unix_millis().filter(|finished| *finished >= started_at_ms)
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(monotonic_started_at) = self.native_stash_bridge.page_capture_monotonic_started_at
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(monotonic_started_offset_ms) = self
            .native_stash_bridge
            .page_capture_archive_monotonic_started_offset_ms
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(monotonic_finished_offset_ms) =
            page_capture_monotonic_offset_ms(monotonic_started_at)
                .filter(|finished| *finished >= monotonic_started_offset_ms)
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        self.native_stash_bridge.page_capture_archive_finished_at_ms = Some(finished_at_ms);
        self.native_stash_bridge
            .page_capture_archive_monotonic_interval = Some(CaptureMonotonicInterval {
            started_offset_ms: monotonic_started_offset_ms,
            finished_offset_ms: monotonic_finished_offset_ms,
        });
        self.native_stash_bridge.page_capture_mutation_evidence = Some(mutation_evidence);
        let Some(authorization) = self.native_stash_bridge.page_capture_authorization.as_ref()
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let page_index = self.native_stash_bridge.page_capture_page_index;
        let result = self.state.as_mut().and_then(|application| {
            let expected_size = u64::try_from(received_bytes).ok();
            if authorization.scope().members().len() > 1 {
                application
                    .request_window_page_capture_object_install(
                        authorization,
                        page_index,
                        expected_size,
                        None,
                    )
                    .ok()
            } else {
                application
                    .request_page_capture_object_install(authorization, expected_size, None)
                    .ok()
            }
        });
        if let Some(ticket) = result {
            self.native_stash_bridge.page_capture_install_ticket = Some((operation, ticket));
            true
        } else {
            self.cancel_native_page_capture(operation);
            false
        }
    }

    /// Parses the closed native completion record and cancels only its exact
    /// active operation when the record is malformed. A rejected callback
    /// must not leave its staging reservation alive until the operation
    /// deadline.
    ///
    /// INVARIANT: malformed callback data may retire only the supplied active
    /// operation; a stale operation ID cannot release another capture's stage.
    pub(crate) fn finish_native_page_capture_download_json(
        &mut self,
        operation: OperationId,
        json: &str,
    ) -> bool {
        let Some(facts) = parse_native_page_capture_download_facts(json) else {
            self.cancel_native_page_capture_request(operation);
            return false;
        };
        self.finish_native_page_capture_download(
            operation,
            &facts.download_id,
            &facts.observed_path,
            facts.is_save_page,
            facts.is_mhtml,
            facts.view_matches,
            &facts.observed_url,
            &facts.terminal_state,
            facts.received_bytes,
            &facts.mutation_evidence,
        )
    }

    fn pending_native_page_capture_save_for_download(&self) -> Option<(OperationId, &str)> {
        let operation = self
            .native_stash_bridge
            .page_capture_stage_target
            .as_ref()?
            .operation();
        if !self.native_stash_bridge.page_capture_save_started
            || !self.page_capture_route_is_current(operation)
        {
            return None;
        }
        Some((
            operation,
            self.native_stash_bridge
                .page_capture_download_id
                .as_deref()?,
        ))
    }

    /// Routes the file-only object result into one profile-local atomic
    /// manifest publication. The digest and bytes stay on the Rust side.
    pub fn complete_native_page_capture_install(
        &mut self,
        ticket: StorageTicket,
        result: Result<InstalledObject, StorageOperationError>,
    ) -> bool {
        let Some((operation, expected_ticket)) =
            self.native_stash_bridge.page_capture_install_ticket
        else {
            return false;
        };
        if ticket != expected_ticket {
            return false;
        }
        self.native_stash_bridge.page_capture_install_ticket = None;
        let object = match result {
            Ok(object) => object,
            Err(error) => {
                if error.kind == ferric_browser_storage::StorageFailureKind::StorageFull {
                    let page_index = self.native_stash_bridge.page_capture_page_index;
                    let tab = self
                        .native_stash_bridge
                        .page_capture_authorization
                        .as_ref()
                        .and_then(|authorization| authorization.scope().members().get(page_index))
                        .map(|member| member.target().tab);
                    let path = self
                        .native_stash_bridge
                        .page_capture_stage_target
                        .as_ref()
                        .and_then(|target| target.path().to_str())
                        .map(str::to_owned);
                    let download_id = self.native_stash_bridge.page_capture_download_id.clone();
                    if let Some(tab) = tab {
                        return self.fail_native_window_capture_page(
                            operation,
                            tab,
                            ManifestPageArchiveFailure::StorageFull,
                            path.as_deref(),
                            download_id.as_deref(),
                        );
                    }
                }
                self.cancel_native_page_capture(operation);
                return false;
            }
        };
        if !self.page_capture_route_is_current(operation) {
            self.cancel_native_page_capture(operation);
            return false;
        }
        self.native_stash_bridge.page_capture_installed_object = Some(object);
        let sidecar_bytes = self.native_stash_bridge.page_capture_sidecar_json.take();
        let request = self
            .native_stash_bridge
            .page_capture_collector_request
            .clone();
        let sidecar_install = match (sidecar_bytes, request) {
            (Some(bytes), Some(request)) => {
                let Some(authorization) =
                    self.native_stash_bridge.page_capture_authorization.as_ref()
                else {
                    self.cancel_native_page_capture(operation);
                    return false;
                };
                let page_index = self.native_stash_bridge.page_capture_page_index;
                self.state.as_mut().and_then(|application| {
                    if authorization.scope().members().len() > 1 {
                        application.request_window_page_capture_sidecar_install(
                            authorization,
                            page_index,
                            bytes,
                            &request.expected_origin,
                            &request.route_fingerprint,
                            &request.capture_fingerprint,
                            &request.nonce,
                            request.include_media,
                        )
                    } else {
                        application.request_page_capture_sidecar_install(
                            authorization,
                            bytes,
                            &request.expected_origin,
                            &request.route_fingerprint,
                            &request.capture_fingerprint,
                            &request.nonce,
                            request.include_media,
                        )
                    }
                    .ok()
                })
            }
            _ => None,
        };
        if let Some((ticket, summary)) = sidecar_install {
            self.native_stash_bridge.page_capture_sidecar_summary = Some(summary);
            self.native_stash_bridge.page_capture_sidecar_install_ticket =
                Some((operation, ticket));
            return true;
        }
        if self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .is_some_and(|authorization| authorization.scope().members().len() > 1)
        {
            self.native_stash_bridge.page_capture_sidecar_summary = None;
            self.native_stash_bridge.page_capture_sidecar_interval = None;
            self.native_stash_bridge
                .page_capture_sidecar_monotonic_interval = None;
        }
        self.publish_native_page_capture(operation, None)
    }

    pub fn complete_native_page_capture_object_install(
        &mut self,
        ticket: StorageTicket,
        result: Result<InstalledObject, StorageOperationError>,
    ) -> bool {
        if self
            .native_stash_bridge
            .page_capture_install_ticket
            .is_some_and(|(_, expected)| expected == ticket)
        {
            self.complete_native_page_capture_install(ticket, result)
        } else {
            self.complete_native_page_capture_sidecar_install(ticket, result)
        }
    }

    /// Completes an optional sidecar install. Its failure only lowers page
    /// fidelity; it never discards the independently validated MHTML object.
    pub fn complete_native_page_capture_sidecar_install(
        &mut self,
        ticket: StorageTicket,
        result: Result<InstalledObject, StorageOperationError>,
    ) -> bool {
        let Some((operation, expected_ticket)) =
            self.native_stash_bridge.page_capture_sidecar_install_ticket
        else {
            return false;
        };
        if ticket != expected_ticket {
            return false;
        }
        self.native_stash_bridge.page_capture_sidecar_install_ticket = None;
        if !self.page_capture_route_is_current(operation) {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let sidecar = result.ok();
        if sidecar.is_none()
            && self
                .native_stash_bridge
                .page_capture_authorization
                .as_ref()
                .is_some_and(|authorization| authorization.scope().members().len() > 1)
        {
            self.native_stash_bridge.page_capture_sidecar_summary = None;
            self.native_stash_bridge.page_capture_sidecar_interval = None;
            self.native_stash_bridge
                .page_capture_sidecar_monotonic_interval = None;
        }
        self.native_stash_bridge
            .page_capture_sidecar_installed_object = sidecar;
        self.publish_native_page_capture(operation, sidecar)
    }

    fn publish_native_page_capture(
        &mut self,
        operation: OperationId,
        sidecar: Option<InstalledObject>,
    ) -> bool {
        if !self.page_capture_route_is_current(operation) {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let (Some(started_at_ms), Some(finished_at_ms)) = (
            self.native_stash_bridge.page_capture_archive_started_at_ms,
            self.native_stash_bridge.page_capture_archive_finished_at_ms,
        ) else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(archive) = self.native_stash_bridge.page_capture_installed_object else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let Some(manifest) = self.native_stash_bridge.page_capture_manifest.as_ref() else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        if started_at_ms == 0 || finished_at_ms < started_at_ms {
            self.cancel_native_page_capture(operation);
            return false;
        }
        if manifest
            .windows
            .first()
            .and_then(|window| {
                window
                    .pages
                    .get(self.native_stash_bridge.page_capture_page_index)
            })
            .is_none()
        {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let categories = self
            .native_stash_bridge
            .page_capture_sidecar_summary
            .as_ref()
            .map_or_else(Vec::new, |summary| {
                summary
                    .coverage
                    .iter()
                    .map(|coverage| {
                        use ferric_browser_core::versioning::SidecarCategory;
                        let category = match coverage.category() {
                            SidecarCategory::Geometry => ManifestPageCategory::Geometry,
                            SidecarCategory::Controls => ManifestPageCategory::Controls,
                            SidecarCategory::Editable => ManifestPageCategory::Editable,
                            SidecarCategory::Selection => ManifestPageCategory::Selection,
                            SidecarCategory::Media => ManifestPageCategory::Media,
                        };
                        ferric_browser_storage::versioning::ManifestCategoryCoverage {
                            category,
                            requested: coverage.requested(),
                            captured: coverage.captured(),
                            truncated: coverage.truncated(),
                        }
                    })
                    .collect()
            });
        let page_capture = ManifestPageCapture {
            archive_interval: Some(CaptureInterval {
                started_at_ms,
                finished_at_ms,
            }),
            sidecar_interval: self.native_stash_bridge.page_capture_sidecar_interval,
            archive_monotonic_interval: self
                .native_stash_bridge
                .page_capture_archive_monotonic_interval,
            sidecar_monotonic_interval: self
                .native_stash_bridge
                .page_capture_sidecar_monotonic_interval,
            // Mutation observation is a warning signal, never an atomicity
            // certificate. NotObserved remains distinct from ObservedStable.
            mutation_evidence: self
                .native_stash_bridge
                .page_capture_mutation_evidence
                .unwrap_or(ManifestMutationEvidence::NotObserved),
            categories,
        };
        let helper_version = self
            .native_stash_bridge
            .page_capture_sidecar_summary
            .as_ref()
            .map(|summary| summary.helper_version.clone());
        let engine_fingerprint = page_capture_engine_fingerprint();
        let Some(authorization) = self.native_stash_bridge.page_capture_authorization.as_ref()
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        if authorization.scope().members().len() > 1 {
            return self.advance_native_window_capture(
                operation,
                Some(page_capture),
                engine_fingerprint.as_deref(),
            );
        }
        let Some(manifest) = self.native_stash_bridge.page_capture_manifest.take() else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let publish_ticket = self.state.as_mut().and_then(|application| {
            application
                .request_page_capture_object_publish(
                    authorization,
                    manifest,
                    archive,
                    sidecar,
                    page_capture,
                    PageCaptureProvenance::new(
                        helper_version.as_deref(),
                        engine_fingerprint.as_deref(),
                    ),
                )
                .ok()
        });
        let Some(publish_ticket) = publish_ticket else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        self.native_stash_bridge.page_capture_publish_ticket = Some((operation, publish_ticket));
        true
    }

    fn advance_native_window_capture(
        &mut self,
        operation: OperationId,
        page_capture: Option<ManifestPageCapture>,
        engine_fingerprint: Option<&str>,
    ) -> bool {
        let Some(authorization) = self.native_stash_bridge.page_capture_authorization.as_ref()
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let member_count = authorization.scope().members().len();
        let page_index = self.native_stash_bridge.page_capture_page_index;
        let Some(slot) = self
            .native_stash_bridge
            .page_capture_window_evidence
            .get_mut(page_index)
            .filter(|slot| slot.is_none())
        else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        *slot = page_capture;
        let next_index = page_index + 1;
        if next_index < member_count {
            self.clear_native_page_capture_page_state();
            self.native_stash_bridge.page_capture_page_index = next_index;
            let authorization = self.native_stash_bridge.page_capture_authorization.as_ref();
            let next_ticket = self.state.as_mut().and_then(|application| {
                application
                    .request_window_page_capture_stage(authorization?, next_index)
                    .ok()
            });
            let Some(next_ticket) = next_ticket else {
                self.cancel_native_page_capture(operation);
                return false;
            };
            self.native_stash_bridge.page_capture_stage_ticket = Some((operation, next_ticket));
            return true;
        }
        let Some(manifest) = self.native_stash_bridge.page_capture_manifest.take() else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        let page_captures =
            std::mem::take(&mut self.native_stash_bridge.page_capture_window_evidence);
        let publish_ticket = self.state.as_mut().and_then(|application| {
            application
                .request_window_page_capture_object_publish(
                    authorization,
                    manifest,
                    page_captures,
                    PageCaptureProvenance::new(None, engine_fingerprint),
                )
                .ok()
        });
        let Some(publish_ticket) = publish_ticket else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        self.native_stash_bridge.page_capture_publish_ticket = Some((operation, publish_ticket));
        true
    }

    /// Converts an exactly owned failed native page into one payload-free
    /// window result. A strict capture or an uncorrelated callback still
    /// cancels the whole operation; no failure string becomes authority.
    pub fn fail_native_window_capture_page(
        &mut self,
        operation: OperationId,
        tab: ferric_browser_core::TabId,
        reason: ManifestPageArchiveFailure,
        observed_path: Option<&str>,
        download_id: Option<&str>,
    ) -> bool {
        let page_index = self.native_stash_bridge.page_capture_page_index;
        let eligible = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .is_some_and(|authorization| {
                authorization.operation() == operation
                    && authorization.scope().members().len() > 1
                    && authorization.failure_policy()
                        == ferric_browser_application::versioning::PageCaptureFailurePolicy::AllowPartialPages
                    && authorization
                        .scope()
                        .members()
                        .get(page_index)
                        .is_some_and(|member| member.target().tab == tab)
            })
            && self
                .native_stash_bridge
                .page_capture_stage_target
                .as_ref()
                .is_some_and(|target| target.operation() == operation)
            && self.native_stash_bridge.page_capture_install_ticket.is_none()
            && self.native_stash_bridge.page_capture_publish_ticket.is_none()
            && (reason == ManifestPageArchiveFailure::DocumentChanged
                || self.page_capture_route_is_current(operation))
            && if self.native_stash_bridge.page_capture_save_started {
                self.native_stash_bridge
                    .page_capture_stage_target
                    .as_ref()
                    .and_then(|target| target.path().to_str())
                    == observed_path
                    && self.native_stash_bridge.page_capture_download_id.as_deref()
                        == download_id
            } else {
                observed_path.is_none() && download_id.is_none()
            };
        if !eligible {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let recorded = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .and_then(|authorization| {
                self.state
                    .as_mut()?
                    .record_window_page_capture_failure(authorization, page_index, reason)
                    .ok()
            })
            .is_some();
        if !recorded {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let engine_fingerprint = page_capture_engine_fingerprint();
        self.advance_native_window_capture(operation, None, engine_fingerprint.as_deref())
    }

    /// A durable publication consumes the operation. A failed publication
    /// cancels it so the application's abort path releases the reservation.
    pub fn complete_native_page_capture_publish(
        &mut self,
        ticket: StorageTicket,
        result: Result<
            ferric_browser_storage::VersioningObjectPublishReceipt,
            StorageOperationError,
        >,
    ) -> bool {
        let Some((operation, expected_ticket)) =
            self.native_stash_bridge.page_capture_publish_ticket
        else {
            return false;
        };
        if ticket != expected_ticket {
            return false;
        }
        self.native_stash_bridge.page_capture_publish_ticket = None;
        if result.is_err() {
            self.cancel_native_page_capture(operation);
            return false;
        }
        let operation_effects = self
            .state
            .as_mut()
            .and_then(|application| application.complete_versioning_operation(operation).ok());
        let Some(operation_effects) = operation_effects else {
            self.cancel_native_page_capture(operation);
            return false;
        };
        self.queue_native_operation_effects(operation_effects);
        self.clear_native_page_capture_state();
        true
    }

    fn clear_native_page_capture_state(&mut self) {
        self.native_stash_bridge.page_capture_disclosure = None;
        self.native_stash_bridge.page_capture_authorization = None;
        self.native_stash_bridge.page_capture_manifest = None;
        self.native_stash_bridge.page_capture_stage_ticket = None;
        self.native_stash_bridge
            .page_capture_window_evidence
            .clear();
        self.native_stash_bridge.page_capture_page_index = 0;
        self.clear_native_page_capture_page_state();
    }

    fn clear_native_page_capture_page_state(&mut self) {
        self.native_stash_bridge.page_capture_stage_target = None;
        self.native_stash_bridge.page_capture_save_started = false;
        self.native_stash_bridge.page_capture_collector_request = None;
        self.native_stash_bridge.page_capture_sidecar_json = None;
        self.native_stash_bridge.page_capture_sidecar_summary = None;
        self.native_stash_bridge.page_capture_sidecar_interval = None;
        self.native_stash_bridge.page_capture_monotonic_started_at = None;
        self.native_stash_bridge
            .page_capture_sidecar_monotonic_interval = None;
        self.native_stash_bridge.page_capture_sidecar_install_ticket = None;
        self.native_stash_bridge
            .page_capture_sidecar_installed_object = None;
        self.native_stash_bridge.page_capture_archive_started_at_ms = None;
        self.native_stash_bridge.page_capture_archive_finished_at_ms = None;
        self.native_stash_bridge
            .page_capture_archive_monotonic_started_offset_ms = None;
        self.native_stash_bridge
            .page_capture_archive_monotonic_interval = None;
        self.native_stash_bridge.page_capture_mutation_evidence = None;
        self.native_stash_bridge.page_capture_download_id = None;
        self.native_stash_bridge.page_capture_install_ticket = None;
        self.native_stash_bridge.page_capture_publish_ticket = None;
        self.native_stash_bridge.page_capture_installed_object = None;
    }

    pub(crate) fn page_capture_route_is_current(&self, operation: OperationId) -> bool {
        let Some(authorization) = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .filter(|authorization| authorization.operation() == operation)
        else {
            return false;
        };
        let Some(application) = self.state.as_ref() else {
            return false;
        };
        let Some((envelope, state)) = application.versioning().operation(operation) else {
            return false;
        };
        let members = authorization.scope().members();
        let Some(captured) = members.get(self.native_stash_bridge.page_capture_page_index) else {
            return false;
        };
        let Some((profile, instance)) = self.native_versioning_profile else {
            return false;
        };
        let Some(window) = self.window else {
            return false;
        };
        let owner = NativeProfileOwner::new(application.application_owner(), profile, instance);
        let target = captured.target();
        state == ferric_browser_application::versioning::OperationState::Executing
            && envelope.owner() == owner
            && envelope.scope() == authorization.scope()
            && (members.len() > 1
                || envelope.expected_logical_revision() == application.runtime_view().revision())
            && captured.window() == window
            && captured.native() == owner
            && application.versioning().owner(profile) == Some(owner)
            && application.capture_target(target.tab) == Some(target)
            && application.windows().get(&window).is_some_and(|current| {
                current.profile == profile
                    && (members.len() > 1 || current.active_tab == Some(target.tab))
            })
            && if members.len() > 1 {
                self.tab_ids.contains(&target.tab)
                    && (authorization.failure_policy()
                        == ferric_browser_application::versioning::PageCaptureFailurePolicy::AllowPartialPages
                        || members.iter().all(|member| {
                            application.capture_target(member.target().tab)
                                == Some(member.target())
                        }))
            } else {
                self.tab_for_index(self.active_tab_index) == Some(target.tab)
            }
    }

    pub(crate) fn cancel_native_page_capture(&mut self, operation: OperationId) {
        let operation_effects = self
            .state
            .as_mut()
            .and_then(|application| application.cancel_versioning_operation(operation).ok())
            .map(|effects| effects.operation_effects);
        if let Some(effects) = operation_effects {
            self.queue_native_operation_effects(effects);
        }
        self.clear_native_page_capture_state();
    }

    pub fn cancel_native_page_capture_request(&mut self, operation: OperationId) -> bool {
        let active = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .is_some_and(|authorization| authorization.operation() == operation)
            || self
                .native_stash_bridge
                .page_capture_disclosure
                .as_ref()
                .is_some_and(|pending| pending.preview.operation() == operation);
        if active {
            self.cancel_native_page_capture(operation);
        }
        active
    }

    /// Cancels the one Rust-owned page-capture request during orderly shutdown,
    /// including the interval before QML receives its exact native-save route.
    pub(crate) fn cancel_pending_native_page_capture_for_shutdown(&mut self) -> bool {
        self.clear_terminal_native_page_capture();
        let operation = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .map(PageCaptureAuthorization::operation)
            .or_else(|| {
                self.native_stash_bridge
                    .page_capture_disclosure
                    .as_ref()
                    .map(|pending| pending.preview.operation())
            });
        let Some(operation) = operation else {
            return false;
        };
        self.cancel_native_page_capture(operation);
        true
    }

    pub fn has_native_page_capture_request(&mut self) -> bool {
        self.clear_terminal_native_page_capture();
        self.native_stash_bridge.page_capture_disclosure.is_some()
            || self
                .native_stash_bridge
                .page_capture_authorization
                .is_some()
    }

    pub(crate) fn clear_terminal_native_page_capture(&mut self) {
        let operation = self
            .native_stash_bridge
            .page_capture_authorization
            .as_ref()
            .map(PageCaptureAuthorization::operation)
            .or_else(|| {
                self.native_stash_bridge
                    .page_capture_disclosure
                    .as_ref()
                    .map(|pending| pending.preview.operation())
            });
        let terminal = operation.is_some_and(|operation| {
            self.state
                .as_ref()
                .and_then(|application| application.versioning().operation(operation))
                .is_none_or(|(_, state)| state.terminal())
        });
        if terminal {
            self.clear_native_page_capture_state();
        }
    }

    /// Holds the exact application preview and the sole native decision sink
    /// in Rust. Only a redacted, fixed-copy projection may reach QML. This
    /// requires a trusted categorical observation; no QML boolean can create
    /// a preview or confirmation permit.
    ///
    /// # Errors
    /// Refuses a stale native owner, occupied bridge, unavailable decision
    /// sink, or an application eligibility/preview failure.
    pub fn register_native_stash_disclosure(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<RedactedPreview, StashError> {
        if !self.native_stash_bridge.can_claim_start() {
            return Err(StashError::WrongStage);
        }
        let window = self.window.ok_or(StashError::OwnerChanged)?;
        let (profile, instance) = self
            .native_versioning_profile
            .ok_or(StashError::OwnerChanged)?;
        let application = Option::as_mut(&mut self.state).ok_or(StashError::OwnerChanged)?;
        let owner = NativeProfileOwner::new(application.application_owner(), profile, instance);
        if application.versioning().owner(profile) != Some(owner)
            || application
                .windows()
                .get(&window)
                .is_none_or(|current| current.profile != profile)
        {
            return Err(StashError::OwnerChanged);
        }
        let (envelope, _) = application
            .versioning()
            .operation(operation)
            .ok_or(StashError::UnknownOperation)?;
        let [captured] = envelope.scope().members() else {
            return Err(StashError::WrongScope);
        };
        if captured.window() != window || captured.native() != owner {
            return Err(StashError::OwnerChanged);
        }
        if self.native_stash_bridge.decision_sink.is_none() {
            self.native_stash_bridge.decision_sink =
                application.take_native_versioning_decision_sink();
        }
        if self.native_stash_bridge.decision_sink.is_none() {
            return Err(StashError::InvalidAuthorization);
        }
        let preview = application.prepare_explicit_stash_disclosure(operation, observation)?;
        let redacted = preview.redacted();
        self.native_stash_bridge.disclosure = Some(PendingNativeDisclosure {
            preview,
            accept_attempted: false,
        });
        Ok(redacted)
    }

    /// Consumes an exact displayed preview only after a trusted native UI
    /// acceptance callback. The application rechecks a fresh categorical
    /// observation before accepting the one-use permit.
    ///
    /// # Errors
    /// Refuses an unrelated operation, replay, expired preview or changed
    /// owner/document/activity. A failed acceptance cannot be retried, but
    /// retains the exact operation for native dismissal/cleanup.
    pub fn accept_native_stash_disclosure(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<CapturedOwner, StashError> {
        if self.native_stash_disclosure_for_display().map(|(id, _)| id) != Some(operation) {
            return Err(StashError::WrongStage);
        }
        if self.native_stash_bridge.pending_disclosure_operation() != Some(operation) {
            return Err(StashError::WrongStage);
        }
        let pending = self
            .native_stash_bridge
            .disclosure
            .as_mut()
            .ok_or(StashError::WrongStage)?;
        if pending.accept_attempted {
            return Err(StashError::WrongStage);
        }
        pending.accept_attempted = true;
        let sink = self
            .native_stash_bridge
            .decision_sink
            .as_ref()
            .ok_or(StashError::InvalidAuthorization)?;
        let application = Option::as_mut(&mut self.state).ok_or(StashError::OwnerChanged)?;
        let permit = application
            .capture_native_versioning_decision(operation, sink.accept(&pending.preview))
            .map_err(StashError::Preview)?;
        let owner = application.authorize_explicit_stash(operation, observation, permit)?;
        self.native_stash_bridge.disclosure = None;
        Ok(owner)
    }

    /// Consumes the exact Rust-held consent and reserves one logical blank
    /// fallback. The original native view is still visible; the returned
    /// effects must be presented before any materialization acknowledgement.
    ///
    /// # Errors
    /// Refuses stale or replayed consent without allocating a fallback. If
    /// rollback after an accepted operation fails, maintenance must retry it.
    pub fn accept_and_prepare_native_stash_disclosure(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<NativeStashReservation, StashError> {
        let previous_revision = self
            .state
            .as_ref()
            .ok_or(StashError::OwnerChanged)?
            .revision();
        self.accept_native_stash_disclosure(operation, observation)?;
        let prepared = Option::as_mut(&mut self.state)
            .ok_or(StashError::OwnerChanged)?
            .prepare_explicit_stash_fallback(operation, observation);
        let reason = match prepared {
            Ok(preparation) => match self.register_native_stash_preparation(operation, preparation)
            {
                Ok(runtime_effects) => {
                    return Ok(NativeStashReservation::Prepared {
                        previous_revision,
                        runtime_effects,
                    });
                }
                Err(_) => StashError::StalePreparation,
            },
            Err(error) => error,
        };
        let effects = Option::as_mut(&mut self.state)
            .ok_or(StashError::OwnerChanged)?
            .cancel_versioning_operation(operation)
            .map_err(|_| StashError::RollbackFailed)?;
        Ok(NativeStashReservation::Cancelled {
            previous_revision,
            reason,
            effects,
        })
    }

    /// A native dismissal cancels only the operation whose preview was shown.
    /// No QML-provided boolean is ever treated as a permit.
    ///
    /// # Errors
    /// Refuses an unrelated operation or failed logical cancellation.
    pub fn decline_native_stash_disclosure(
        &mut self,
        operation: OperationId,
    ) -> Result<VersioningEffects, ferric_browser_application::versioning::OperationError> {
        if self.native_stash_bridge.pending_disclosure_operation() != Some(operation) {
            return Err(ferric_browser_application::versioning::OperationError::UnknownOperation);
        }
        let application = Option::as_mut(&mut self.state)
            .ok_or(ferric_browser_application::versioning::OperationError::UnknownOperation)?;
        let effects = application.cancel_versioning_operation(operation)?;
        self.native_stash_bridge.disclosure = None;
        Ok(effects)
    }
}

fn page_capture_monotonic_offset_ms(started_at: std::time::Instant) -> Option<u64> {
    u64::try_from(started_at.elapsed().as_millis()).ok()
}

fn page_capture_engine_fingerprint() -> Option<String> {
    // INVARIANT: backend metadata comes from Qt's public runtime APIs, never
    // from the renderer, QML result or archive payload.
    let webengine = crate::qt_webengine_version();
    let chromium = crate::qt_chromium_version();
    let security_patch = crate::qt_chromium_security_patch_version();
    let platform = crate::qt_platform_name();
    if !runtime_fingerprint_component(&webengine, 48)
        || !runtime_fingerprint_component(&chromium, 48)
        || !runtime_fingerprint_component(&security_patch, 48)
        || !runtime_fingerprint_component(&platform, 32)
    {
        return None;
    }
    let fingerprint = format!(
        "qtwebengine={webengine};chromium={chromium};security-patch={security_patch};platform={platform}"
    );
    (fingerprint.len() <= 256).then_some(fingerprint)
}

fn runtime_fingerprint_component(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod callback_integration_tests {
    use super::*;
    use ferric_browser_application::{
        BrowserApplication, ProfileOpenRequest, ProfileStorage, StorageRequest,
    };
    use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
    use ferric_browser_core::{CommitId, Event, LogicalPageId, PrivacyKind, ValidatedUrl};
    use ferric_browser_runtime::RuntimeInput;
    use ferric_browser_storage::{
        DownloadState, ProfileStore, RootSpec, StorageCompletion, StorageOperation,
        StorageOperationError, StorageRoots, StoreMode,
    };
    use serde_json::json;
    use std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    const SOURCE_URL: &str = "https://example.test/article";

    fn wait_for(ui: &mut BrowserUiRust, ticket: StorageTicket) -> StorageCompletion {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(completion) = ui
                .state
                .as_mut()
                .expect("application")
                .poll_storage_effects()
                .into_iter()
                .find_map(|effect| (effect.ticket == ticket).then_some(effect.completion))
            {
                return completion;
            }
            assert!(Instant::now() < deadline, "storage ticket did not complete");
            thread::yield_now();
        }
    }

    fn prepared_capture() -> (BrowserUiRust, PathBuf, Uuid, OperationId, CommitId) {
        prepared_capture_with_safe_get(true)
    }

    fn prepared_capture_with_safe_get(
        safe_get: bool,
    ) -> (BrowserUiRust, PathBuf, Uuid, OperationId, CommitId) {
        let base = std::env::temp_dir().join(format!("ferric-page-callback-{}", Uuid::new_v4()));
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("normal application");
        application
            .request_profile_open(ProfileOpenRequest {
                name: "default".into(),
                label: "Default".into(),
                privacy: PrivacyKind::Normal,
                storage: ProfileStorage::Durable(RootSpec::Base(base.clone())),
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: RuntimeOverrides::default(),
                config_path: None,
                contexts: ContextsConfig::default(),
                context_configuration_issue: false,
            })
            .expect("durable profile request");
        let deadline = Instant::now() + Duration::from_secs(5);
        let profile = loop {
            if let Some(result) = application.poll_profile_open() {
                break result.expect("durable profile").profile;
            }
            assert!(Instant::now() < deadline, "profile open timed out");
            thread::yield_now();
        };
        let profile_uuid = profile.id.expect("persistent profile identity");
        let target = application.capture_target(tab).expect("initial target");
        let url = ValidatedUrl::parse(SOURCE_URL).expect("safe fixture URL");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: url.clone(),
            }))
            .expect("start fixture navigation");
        let target = application.capture_target(tab).expect("loading target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target,
                url,
                title: "Fixture page".into(),
            }))
            .expect("commit fixture navigation");
        if safe_get {
            assert!(application.note_typed_safe_get_navigation(tab, SOURCE_URL));
        }

        let mut ui = BrowserUiRust {
            state: Some(application),
            window: Some(window),
            tab: Some(tab),
            tab_ids: vec![tab],
            ..BrowserUiRust::default()
        };
        assert!(ui.bind_native_versioning_profile_owner());
        ui.request_native_page_capture_disclosure(false, false)
            .expect("one-use disclosure");
        let operation = ui
            .native_page_capture_disclosure_for_display()
            .expect("fixed disclosure")
            .0;
        let commit = ui
            .native_stash_bridge
            .page_capture_disclosure
            .as_ref()
            .expect("pending disclosure")
            .manifest
            .id
            .parse::<CommitId>()
            .expect("commit id");
        let ticket = ui
            .accept_native_page_capture_disclosure(operation)
            .expect("explicit one-use acceptance");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) = wait_for(&mut ui, ticket) else {
            panic!("exact stage completion expected");
        };
        assert!(ui.complete_native_page_capture_stage(ticket, Ok(stage)));
        (ui, base, profile_uuid, operation, commit)
    }

    fn prepared_window_capture() -> (
        BrowserUiRust,
        PathBuf,
        Uuid,
        OperationId,
        CommitId,
        ferric_browser_core::TabId,
        ferric_browser_core::TabId,
    ) {
        use ferric_browser_application::versioning::{PageCaptureFailurePolicy, PreviewKind};

        let (mut ui, base, profile_uuid, previous_operation, _) = prepared_capture();
        assert!(ui.cancel_native_page_capture_request(previous_operation));
        let window = ui.window.expect("native window");
        let first_tab = ui.tab.expect("first tab");
        ui.state
            .as_mut()
            .expect("application")
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second tab");
        let second_tab = ui.state.as_ref().expect("application").windows()[&window]
            .active_tab
            .expect("second tab active");
        let target = ui
            .state
            .as_ref()
            .expect("application")
            .capture_target(second_tab)
            .expect("second tab target");
        let url = ValidatedUrl::parse(SOURCE_URL).expect("safe second-page URL");
        ui.state
            .as_mut()
            .expect("application")
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: url.clone(),
            }))
            .expect("start second-page navigation");
        let target = ui
            .state
            .as_ref()
            .expect("application")
            .capture_target(second_tab)
            .expect("loading second-page target");
        ui.state
            .as_mut()
            .expect("application")
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target,
                url,
                title: "Second fixture page".into(),
            }))
            .expect("commit second-page navigation");
        assert!(
            ui.state
                .as_mut()
                .expect("application")
                .note_typed_safe_get_navigation(second_tab, SOURCE_URL)
        );
        ui.tab_ids = vec![first_tab, second_tab];
        ui.active_tab_index = 1;
        ui.tab = Some(second_tab);
        let (redacted, _, _) = ui
            .request_native_window_page_capture_disclosure(
                false,
                false,
                PageCaptureFailurePolicy::AllowPartialPages,
            )
            .expect("distinct native window disclosure");
        assert_eq!(redacted.kind, PreviewKind::PageArchivePartial);
        let operation = ui
            .native_page_capture_disclosure_for_display()
            .expect("fixed two-page disclosure")
            .0;
        let commit = ui
            .native_stash_bridge
            .page_capture_disclosure
            .as_ref()
            .expect("pending window disclosure")
            .manifest
            .id
            .parse::<CommitId>()
            .expect("window commit id");
        let ticket = ui
            .accept_native_page_capture_disclosure(operation)
            .expect("one-use window acceptance");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) = wait_for(&mut ui, ticket) else {
            panic!("first window stage completion expected");
        };
        assert!(ui.complete_native_page_capture_stage(ticket, Ok(stage)));
        (
            ui,
            base,
            profile_uuid,
            operation,
            commit,
            first_tab,
            second_tab,
        )
    }

    #[test]
    fn shutdown_cancels_rust_owned_capture_before_qml_save_owner_exists() {
        let (mut ui, base, _, operation, _) = prepared_capture();
        assert!(ui.has_native_page_capture_request());
        assert_eq!(
            ui.pending_native_page_capture_save(),
            Some((operation, ui.tab.expect("captured page")))
        );

        assert!(ui.cancel_pending_native_page_capture_for_shutdown());
        assert!(!ui.has_native_page_capture_request());
        assert!(ui.pending_native_page_capture_save().is_none());
        assert!(!ui.cancel_pending_native_page_capture_for_shutdown());
        finish(&mut ui, base);
    }

    #[test]
    fn native_window_disclosure_binds_two_pages_and_projects_the_first_stage() {
        let (mut ui, base, _, operation, _, first_tab, _) = prepared_window_capture();
        assert_eq!(
            ui.pending_native_page_capture_save(),
            Some((operation, first_tab)),
            "first captured member remains bound even while another tab is active"
        );
        assert!(ui.cancel_native_page_capture_request(operation));
        finish(&mut ui, base);
    }

    #[test]
    fn native_page_capture_rejects_reported_size_over_the_archive_limit() {
        let (mut ui, base, _, operation, _) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("capture-local route challenge");
        assert!(ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge),));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save target");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "508", &path, true, true, true,
        ));

        let above_limit = ferric_browser_core::versioning::limits::MAX_ARCHIVE_BYTES + 1;
        assert!(!ui.finish_native_page_capture_download(
            operation,
            "508",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(above_limit).expect("archive limit fits signed bytes"),
            "not-observed",
        ));
        assert!(ui.pending_native_page_capture_save().is_none());
        assert!(ui.native_stash_bridge.page_capture_install_ticket.is_none());
        finish(&mut ui, base);
    }

    #[test]
    fn malformed_native_capture_completion_cancels_only_its_exact_operation() {
        let (mut ui, base, _, operation, _) = prepared_capture();
        let tab = ui.tab.expect("captured page");
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save target");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "509", &path, true, true, true,
        ));
        assert!(ui.has_native_page_capture_request());

        assert!(!ui.finish_native_page_capture_download_json(
            OperationId::fresh(),
            "{malformed completion}"
        ));
        assert!(
            ui.has_native_page_capture_request(),
            "a stale malformed callback cannot cancel the live reservation"
        );
        assert!(ui.native_stash_bridge.page_capture_save_started);
        assert_eq!(
            ui.native_stash_bridge.page_capture_download_id.as_deref(),
            Some("509"),
            "a stale malformed callback preserves the claimed request"
        );

        assert!(!ui.finish_native_page_capture_download_json(operation, "{malformed completion}"));
        assert!(!ui.has_native_page_capture_request());
        assert!(ui.pending_native_page_capture_save().is_none());
        finish(&mut ui, base);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One regression follows both native saves through one atomic publication.
    fn native_window_stages_two_archives_before_one_manifest_publication() {
        let (mut ui, base, profile_uuid, operation, commit, first_tab, second_tab) =
            prepared_window_capture();
        let mut challenges = Vec::new();
        let mut first_page_sidecar = None;
        for (index, tab) in [first_tab, second_tab].into_iter().enumerate() {
            assert_eq!(
                ui.pending_native_page_capture_save(),
                Some((operation, tab))
            );
            let challenge = ui
                .native_page_capture_sidecar_request(operation, tab)
                .expect("capture-local route challenge");
            challenges.push(challenge.nonce.clone());
            let sidecar_json = synthetic_sidecar(challenge);
            if index == 0 {
                first_page_sidecar = Some(sidecar_json.clone());
            } else {
                assert!(
                    !ui.complete_native_page_capture_sidecar(
                        operation,
                        first_page_sidecar.take().expect("first page sidecar"),
                    ),
                    "the same-route first page must not qualify for the second challenge"
                );
            }
            assert!(ui.complete_native_page_capture_sidecar(operation, sidecar_json));
            let path = ui
                .claim_native_page_capture_save(operation, tab, SOURCE_URL)
                .expect("exact indexed native save target");
            let bytes = if index == 0 {
                b"first window archive".as_slice()
            } else {
                b"second window archive".as_slice()
            };
            fs::write(&path, bytes).expect("synthetic native page save");
            let download_id = if index == 0 { "501" } else { "502" };
            assert!(ui.note_native_page_capture_download_requested(
                operation,
                download_id,
                &path,
                true,
                true,
                true
            ));
            assert!(ui.finish_native_page_capture_download(
                operation,
                download_id,
                &path,
                true,
                true,
                true,
                SOURCE_URL,
                "completed",
                i64::try_from(bytes.len()).expect("bounded archive size"),
                "not-observed",
            ));
            let (_, install_ticket) = ui
                .native_stash_bridge
                .page_capture_install_ticket
                .expect("indexed archive install ticket");
            let StorageCompletion::VersioningObjectInstall(Ok(object)) =
                wait_for(&mut ui, install_ticket)
            else {
                panic!("indexed archive installation expected");
            };
            assert!(
                ui.page_capture_route_is_current(operation),
                "route at page {index}"
            );
            assert!(
                ui.complete_native_page_capture_install(install_ticket, Ok(object)),
                "archive completion at page {index}"
            );
            let (_, sidecar_ticket) = ui
                .native_stash_bridge
                .page_capture_sidecar_install_ticket
                .expect("sidecar install is bound to the indexed archive");
            let StorageCompletion::VersioningObjectInstall(Ok(sidecar)) =
                wait_for(&mut ui, sidecar_ticket)
            else {
                panic!("indexed sidecar installation expected");
            };
            assert!(ui.complete_native_page_capture_sidecar_install(sidecar_ticket, Ok(sidecar)));
            if index == 0 {
                assert!(ui.native_stash_bridge.page_capture_publish_ticket.is_none());
                let (_, next_ticket) = ui
                    .native_stash_bridge
                    .page_capture_stage_ticket
                    .expect("next member stage ticket");
                let StorageCompletion::VersioningCaptureStage(Ok(stage)) =
                    wait_for(&mut ui, next_ticket)
                else {
                    panic!("next page stage expected");
                };
                assert!(ui.complete_native_page_capture_stage(next_ticket, Ok(stage)));
            }
        }
        assert_ne!(challenges[0], challenges[1]);
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("one aggregate publish ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest = stored_manifest(&mut ui, profile_uuid, commit).expect("durable window");
        assert_eq!(manifest.windows[0].pages.len(), 2);
        assert!(
            manifest.windows[0]
                .pages
                .iter()
                .all(|page| page.archive_object.is_some()
                    && page.sidecar_object.is_some()
                    && page.page_capture.as_ref().is_some_and(|capture| {
                        capture.sidecar_interval.is_some()
                            && capture.sidecar_monotonic_interval.is_some()
                            && !capture.categories.is_empty()
                    })
                    && page.archive_failure.is_none())
        );
        assert_ne!(
            manifest.windows[0].pages[0]
                .sidecar_object
                .as_ref()
                .expect("first sidecar")
                .sha256,
            manifest.windows[0].pages[1]
                .sidecar_object
                .as_ref()
                .expect("second sidecar")
                .sha256,
            "same-route tabs must retain distinct challenged sidecars"
        );
        assert_eq!(
            manifest.helper_bundle_version,
            ferric_browser_core::versioning::PAGE_STATE_HELPER_VERSION
        );
        finish(&mut ui, base);
    }

    #[test]
    fn native_partial_window_publishes_one_timeout_and_one_verified_archive() {
        let (mut ui, base, profile_uuid, operation, commit, first_tab, second_tab) =
            prepared_window_capture();
        assert!(ui.fail_native_window_capture_page(
            operation,
            first_tab,
            ManifestPageArchiveFailure::Timeout,
            None,
            None,
        ));
        let (_, next_ticket) = ui
            .native_stash_bridge
            .page_capture_stage_ticket
            .expect("second page stage after consented timeout");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) = wait_for(&mut ui, next_ticket)
        else {
            panic!("second page stage expected");
        };
        assert!(ui.complete_native_page_capture_stage(next_ticket, Ok(stage)));
        assert_eq!(
            ui.pending_native_page_capture_save(),
            Some((operation, second_tab))
        );
        ui.native_page_capture_sidecar_request(operation, second_tab)
            .expect("second-page route challenge");
        let path = ui
            .claim_native_page_capture_save(operation, second_tab, SOURCE_URL)
            .expect("second-page exact stage path");
        let bytes = b"second window archive";
        fs::write(&path, bytes).expect("synthetic second-page save");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "602", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "602",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(bytes.len()).expect("bounded archive size"),
            "not-observed",
        ));
        let (_, install_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("second-page archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(object)) =
            wait_for(&mut ui, install_ticket)
        else {
            panic!("second-page immutable object expected");
        };
        assert!(ui.complete_native_page_capture_install(install_ticket, Ok(object)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("one partial-window publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest = stored_manifest(&mut ui, profile_uuid, commit).expect("durable partial");
        assert_eq!(manifest.windows[0].pages.len(), 2);
        assert_eq!(
            manifest.windows[0].pages[0].archive_failure,
            Some(ManifestPageArchiveFailure::Timeout)
        );
        assert!(manifest.windows[0].pages[0].archive_object.is_none());
        assert!(manifest.windows[0].pages[1].archive_object.is_some());
        finish(&mut ui, base);
    }

    #[test]
    fn native_partial_window_cancelled_started_save_advances_to_next_page() {
        let (mut ui, base, _, operation, _, first_tab, second_tab) = prepared_window_capture();
        ui.native_page_capture_sidecar_request(operation, first_tab)
            .expect("first-page challenge");
        let path = ui
            .claim_native_page_capture_save(operation, first_tab, SOURCE_URL)
            .expect("first-page exact stage path");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "701", &path, true, true, true
        ));
        assert!(ui.fail_native_window_capture_page(
            operation,
            first_tab,
            ManifestPageArchiveFailure::Cancelled,
            Some(&path),
            Some("701"),
        ));
        let (_, next_ticket) = ui
            .native_stash_bridge
            .page_capture_stage_ticket
            .expect("next page stage follows a correlated cancellation");
        let StorageCompletion::VersioningCaptureStage(Ok(stage)) = wait_for(&mut ui, next_ticket)
        else {
            panic!("second-page stage expected");
        };
        assert!(ui.complete_native_page_capture_stage(next_ticket, Ok(stage)));
        assert_eq!(
            ui.pending_native_page_capture_save(),
            Some((operation, second_tab))
        );
        ui.cancel_native_page_capture(operation);
        finish(&mut ui, base);
    }

    fn stored_manifest(
        ui: &mut BrowserUiRust,
        profile_uuid: Uuid,
        commit: CommitId,
    ) -> Option<ferric_browser_storage::versioning::VersionedCaptureManifest> {
        let ticket = ui
            .state
            .as_mut()
            .expect("application")
            .submit_storage(StorageRequest::VersioningManifest {
                profile_uuid,
                id: commit,
            })
            .expect("manifest query admitted");
        let StorageCompletion::VersioningManifest(Ok(manifest)) = wait_for(ui, ticket) else {
            panic!("manifest query completion expected");
        };
        manifest
    }

    fn finish(ui: &mut BrowserUiRust, base: PathBuf) {
        ui.state.as_mut().expect("application").close_profile();
        fs::remove_dir_all(base).expect("remove exact disposable profile");
    }

    fn synthetic_sidecar(challenge: PageCaptureSidecarRequest) -> String {
        let mut categories = vec!["geometry", "controls"];
        let mut coverage = vec![
            json!({"category": "geometry", "requested": 1, "captured": 0, "truncated": false}),
            json!({"category": "controls", "requested": 1, "captured": 0, "truncated": false}),
        ];
        if challenge.include_media {
            categories.push("media");
            coverage.push(
                json!({"category": "media", "requested": 1, "captured": 0, "truncated": false}),
            );
        }
        json!({
            "schema_version": 1,
            "helper_version": "ferric-page-state-5",
            "nonce": challenge.nonce,
            "capture_fingerprint": challenge.capture_fingerprint,
            "requested_categories": categories,
            "frames": [{
                "id": 0,
                "parent": null,
                "origin": challenge.expected_origin,
                "route_fingerprint": challenge.route_fingerprint
            }],
            "candidates": [],
            "positions": [],
            "controls": [],
            "coverage": coverage,
            "exclusions": [],
            "sensitive_editable": []
        })
        .to_string()
    }

    #[test]
    fn native_sidecar_rejects_nested_frame_authority_before_publication() {
        let (mut ui, base, _profile_uuid, operation, _commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        let clean = synthetic_sidecar(challenge);
        let mut injected: serde_json::Value =
            serde_json::from_str(&clean).expect("synthetic sidecar JSON");
        injected["frames"][0]["native_command"] = json!("open-file");
        injected["frames"][0]["url"] = json!("https://example.test/private-frame-url");
        assert!(!ui.complete_native_page_capture_sidecar(operation, injected.to_string()));
        assert!(ui.native_stash_bridge.page_capture_sidecar_json.is_none());
        assert!(
            ui.native_stash_bridge
                .page_capture_sidecar_summary
                .is_none()
        );
        assert!(
            ui.native_stash_bridge
                .page_capture_sidecar_interval
                .is_none()
        );
        assert!(ui.complete_native_page_capture_sidecar(operation, clean));
        finish(&mut ui, base);
    }

    #[test]
    fn successful_sidecar_and_archive_publish_one_durable_commit() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        let sidecar_json = synthetic_sidecar(challenge);
        assert!(ui.complete_native_page_capture_sidecar(operation, sidecar_json.clone()));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        let archive_bytes = b"synthetic bounded MHTML archive";
        fs::write(&path, archive_bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "106", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "106",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(archive_bytes.len()).expect("bounded archive size"),
            "observed-stable"
        ));
        let (_, archive_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, archive_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(archive_ticket, Ok(archive)));
        assert!(
            !ui.complete_native_page_capture_sidecar(operation, sidecar_json.clone()),
            "a duplicate collector callback cannot replace an in-flight sidecar"
        );
        let (_, sidecar_ticket) = ui
            .native_stash_bridge
            .page_capture_sidecar_install_ticket
            .expect("sidecar install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(sidecar)) =
            wait_for(&mut ui, sidecar_ticket)
        else {
            panic!("sidecar installation expected");
        };
        assert!(ui.complete_native_page_capture_sidecar_install(sidecar_ticket, Ok(sidecar)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("manifest publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));

        let manifest = stored_manifest(&mut ui, profile_uuid, commit).expect("durable manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_some());
        let capture = page.page_capture.as_ref().expect("capture intervals");
        assert!(capture.archive_interval.is_some());
        assert!(capture.sidecar_interval.is_some());
        assert_eq!(
            capture.mutation_evidence,
            ManifestMutationEvidence::ObservedStable
        );
        let list_ticket = ui
            .state
            .as_mut()
            .expect("application")
            .submit_storage(StorageRequest::VersioningList {
                profile_uuid,
                limit: 10,
                offset: 0,
            })
            .expect("commit list admitted");
        let StorageCompletion::VersioningList(Ok(commits)) = wait_for(&mut ui, list_ticket) else {
            panic!("commit list expected");
        };
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].id, commit);
        let logical_page = page
            .logical_page_id
            .parse::<LogicalPageId>()
            .expect("typed logical page");
        let state_root = StorageRoots::resolve(RootSpec::Base(base.clone()))
            .expect("disposable roots")
            .state;
        let read_ticket = ui
            .state
            .as_mut()
            .expect("application")
            .submit_storage(StorageRequest::VersioningSidecarRead {
                profile_uuid,
                state_root,
                id: commit,
                logical_page,
            })
            .expect("committed sidecar read admitted");
        let StorageCompletion::VersioningSidecarRead(Ok(Some(read_bytes))) =
            wait_for(&mut ui, read_ticket)
        else {
            panic!("committed sidecar read expected");
        };
        assert!(
            read_bytes.into_bytes() == sidecar_json.as_bytes(),
            "committed sidecar bytes differ"
        );
        finish(&mut ui, base);
    }

    #[test]
    fn same_document_mutation_between_sidecar_and_archive_is_partial_not_atomic() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        assert!(ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge)));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        let archive_bytes = b"archive after same-document mutation";
        fs::write(&path, archive_bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "107", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "107",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(archive_bytes.len()).expect("bounded archive size"),
            "observed-changed"
        ));
        let (_, archive_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, archive_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(archive_ticket, Ok(archive)));
        let (_, sidecar_ticket) = ui
            .native_stash_bridge
            .page_capture_sidecar_install_ticket
            .expect("sidecar install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(sidecar)) =
            wait_for(&mut ui, sidecar_ticket)
        else {
            panic!("sidecar installation expected");
        };
        assert!(ui.complete_native_page_capture_sidecar_install(sidecar_ticket, Ok(sidecar)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("manifest publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest = stored_manifest(&mut ui, profile_uuid, commit).expect("durable manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_some());
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Partial
        );
        assert_eq!(
            page.page_capture
                .as_ref()
                .expect("capture intervals")
                .mutation_evidence,
            ManifestMutationEvidence::ObservedChanged
        );
        finish(&mut ui, base);
    }

    #[test]
    fn navigation_between_sidecar_and_archive_cancels_without_mixed_document_commit() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        assert!(ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge)));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        let archive_bytes = b"archive after navigation";
        fs::write(&path, archive_bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "108", &path, true, true, true
        ));
        let application = ui.state.as_mut().expect("application");
        let target = application.capture_target(tab).expect("current target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: ValidatedUrl::parse("https://example.test/next").expect("next route"),
            }))
            .expect("start replacement navigation");
        assert!(!ui.finish_native_page_capture_download(
            operation,
            "108",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(archive_bytes.len()).expect("bounded archive size"),
            "observed-stable"
        ));
        assert!(ui.native_stash_bridge.page_capture_authorization.is_none());
        assert!(stored_manifest(&mut ui, profile_uuid, commit).is_none());
        finish(&mut ui, base);
    }

    #[test]
    fn late_collector_after_archive_start_is_omitted_without_losing_archive() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        assert!(
            !ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge)),
            "collector result cannot trail archive start"
        );
        let archive_bytes = b"archive without late sidecar";
        fs::write(&path, archive_bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "109", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "109",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(archive_bytes.len()).expect("bounded archive size"),
            "not-observed"
        ));
        let (_, archive_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, archive_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(archive_ticket, Ok(archive)));
        assert!(
            ui.native_stash_bridge
                .page_capture_sidecar_install_ticket
                .is_none()
        );
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("archive-only publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest = stored_manifest(&mut ui, profile_uuid, commit).expect("durable manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_none());
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Unavailable
        );
        assert!(
            page.page_capture
                .as_ref()
                .expect("archive interval")
                .sidecar_interval
                .is_none()
        );
        finish(&mut ui, base);
    }

    #[test]
    fn failed_collector_keeps_successful_archive_and_unavailable_page_fidelity() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let _challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        let bytes = b"synthetic bounded MHTML archive";
        fs::write(&path, bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "101", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "101",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(bytes.len()).expect("bounded archive size"),
            "not-observed"
        ));
        let (_, install_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, install_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(install_ticket, Ok(archive)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("manifest publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest =
            stored_manifest(&mut ui, profile_uuid, commit).expect("durable archive-only manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_none());
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Unavailable
        );
        finish(&mut ui, base);
    }

    #[test]
    fn nonreplayable_page_keeps_placeholder_but_publishes_consented_archive() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture_with_safe_get(false);
        let tab = ui.tab.expect("captured tab");
        assert!(matches!(
            ui.native_stash_bridge
                .page_capture_manifest
                .as_ref()
                .expect("prepared manifest")
                .windows[0]
                .pages[0]
                .safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
        ));
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("transient route for the exact live page");
        assert_eq!(challenge.expected_origin, "https://example.test");
        assert!(!ui.complete_native_page_capture_sidecar(operation, "{malformed".into()));
        assert!(
            ui.claim_native_page_capture_save(operation, tab, "https://example.test/other")
                .is_none(),
            "a different route must not consume the one-use save claim"
        );
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("consented archive path despite nonreplayable descriptor");
        let bytes = b"synthetic POST-result MHTML archive";
        fs::write(&path, bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "105", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "105",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(bytes.len()).expect("bounded archive size"),
            "not-observed"
        ));
        let (_, install_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, install_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(install_ticket, Ok(archive)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("archive-only publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest =
            stored_manifest(&mut ui, profile_uuid, commit).expect("durable archive-only manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(matches!(
            page.safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
        ));
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_none());
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Unavailable
        );
        finish(&mut ui, base);
    }

    // xtask supplies an actual QtWebEngine POST-result MHTML output after the
    // complete native archive probe passes. This is deliberately not part of ordinary
    // Rust-only test runs, which cannot manufacture a Qt page-save callback.
    fn publish_native_archive_fixture(
        fixture: &Path,
    ) -> (BrowserUiRust, PathBuf, Uuid, CommitId, u64) {
        let source_len = fs::metadata(fixture)
            .expect("native archive metadata")
            .len();
        assert!(source_len > 0, "Qt archive must be nonempty");

        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture_with_safe_get(false);
        let tab = ui.tab.expect("captured tab");
        // This native fixture exercises archive-only publication. A page-state
        // collector is optional, and the archive claim anchors its own timing
        // when no collector (or media fingerprint key) is available.
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        assert_eq!(
            fs::copy(fixture, &path).expect("copy verified Qt output to claimed path"),
            source_len
        );
        assert!(ui.note_native_page_capture_download_requested(
            operation, "104", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "104",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(source_len).expect("bounded Qt archive size"),
            "not-observed"
        ));
        let (_, install_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, install_ticket)
        else {
            panic!("Qt archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(install_ticket, Ok(archive)));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("manifest publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        (ui, base, profile_uuid, commit, source_len)
    }

    #[test]
    #[ignore = "requires verified native Qt archive output from cargo xtask native archive"]
    fn native_qt_mhtml_file_commits_through_durable_adapter() {
        let fixture = PathBuf::from(
            std::env::var_os("FERRIC_NATIVE_ARCHIVE_FIXTURE")
                .expect("xtask must provide the verified native archive"),
        );
        let (mut ui, base, profile_uuid, commit, source_len) =
            publish_native_archive_fixture(&fixture);
        let manifest =
            stored_manifest(&mut ui, profile_uuid, commit).expect("committed Qt archive manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(matches!(
            page.safe_url_or_placeholder,
            ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
        ));
        assert_eq!(
            page.archive_object
                .as_ref()
                .expect("Qt archive object")
                .bytes,
            source_len
        );
        assert!(page.sidecar_object.is_none());
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Unavailable
        );
        finish(&mut ui, base);
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum NativeCaptureScenario {
        None,
        WindowStrict,
        WindowStrictViewLoss,
        WindowPartialCancel,
        WindowPartialSecondCancel,
        WindowPartialSecondTimeout,
        WindowPartialNavigate,
        WindowPartialSecondNavigate,
        WindowPartialTimeout,
        WindowPartialViewLoss,
        WindowPartialSecondViewLoss,
        WindowPartialCloseSavedPage,
        #[allow(dead_code)] // Constructed only by the native archive fault-test feature.
        WindowPartialCloseDuringInstall,
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        WindowPartialStorageFull,
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        WindowPartialSecondStorageFull,
        RestoreGeometry,
        RestoreMultiPageGeometry,
        RestoreEditableRefill,
        RestoreMediaPosition,
        SelectionEditor,
        PostForm,
        PostRedirect303,
        PostRedirect307,
        SecondaryPostForm,
        Navigate,
        RemoveFrameDuringSave,
        ReloadFrameDuringSave,
        CancelDownload,
        DeclineThenCapture,
        CollectorTimeout,
        #[cfg(feature = "native-archive-fault-test")]
        ArchiveInstallPause,
        #[cfg(feature = "native-archive-fault-test")]
        ArchiveInstallShutdown,
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        ArchiveInstallStorageFull,
        #[cfg(feature = "native-archive-fault-test")]
        SidecarWorkerRefusal,
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        SidecarFilesystemPermissionDenied,
    }

    impl NativeCaptureScenario {
        fn is_window(self) -> bool {
            match self {
                Self::WindowStrict
                | Self::WindowStrictViewLoss
                | Self::WindowPartialCancel
                | Self::WindowPartialSecondCancel
                | Self::WindowPartialSecondTimeout
                | Self::WindowPartialNavigate
                | Self::WindowPartialSecondNavigate
                | Self::WindowPartialTimeout
                | Self::WindowPartialViewLoss
                | Self::WindowPartialSecondViewLoss
                | Self::WindowPartialCloseSavedPage
                | Self::WindowPartialCloseDuringInstall
                | Self::RestoreMultiPageGeometry => true,
                #[cfg(all(feature = "native-archive-fault-test", unix))]
                Self::WindowPartialStorageFull | Self::WindowPartialSecondStorageFull => true,
                _ => false,
            }
        }

        fn is_partial_window(self) -> bool {
            match self {
                Self::WindowPartialCancel
                | Self::WindowPartialSecondCancel
                | Self::WindowPartialSecondTimeout
                | Self::WindowPartialNavigate
                | Self::WindowPartialSecondNavigate
                | Self::WindowPartialTimeout
                | Self::WindowPartialViewLoss
                | Self::WindowPartialSecondViewLoss
                | Self::WindowPartialCloseSavedPage
                | Self::WindowPartialCloseDuringInstall => true,
                #[cfg(all(feature = "native-archive-fault-test", unix))]
                Self::WindowPartialStorageFull | Self::WindowPartialSecondStorageFull => true,
                _ => false,
            }
        }

        fn is_second_page_failure(self) -> bool {
            let common = matches!(
                self,
                Self::WindowPartialSecondCancel
                    | Self::WindowPartialSecondViewLoss
                    | Self::WindowPartialSecondTimeout
                    | Self::WindowPartialSecondNavigate
            );
            #[cfg(all(feature = "native-archive-fault-test", unix))]
            let common = common || self == Self::WindowPartialSecondStorageFull;
            common
        }

        fn expects_no_commit(self) -> bool {
            match self {
                Self::Navigate
                | Self::RemoveFrameDuringSave
                | Self::ReloadFrameDuringSave
                | Self::CancelDownload => true,
                Self::WindowStrictViewLoss => true,
                #[cfg(all(feature = "native-archive-fault-test", unix))]
                Self::ArchiveInstallStorageFull => true,
                #[cfg(feature = "native-archive-fault-test")]
                Self::ArchiveInstallShutdown => true,
                _ => false,
            }
        }

        fn is_shutdown_drain(self) -> bool {
            #[cfg(feature = "native-archive-fault-test")]
            if self == Self::ArchiveInstallShutdown {
                return true;
            }
            false
        }
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    struct StagePermissionsGuard {
        path: std::path::PathBuf,
        original: fs::Permissions,
        restored: bool,
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    impl StagePermissionsGuard {
        fn restore(&mut self) {
            fs::set_permissions(&self.path, self.original.clone())
                .expect("restore exact disposable capture staging permissions");
            self.restored = true;
        }
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    impl Drop for StagePermissionsGuard {
        fn drop(&mut self) {
            if !self.restored {
                let _ = fs::set_permissions(&self.path, self.original.clone());
            }
        }
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_page_collects_saves_and_publishes_through_production_window() {
        native_qt_capture_journey("/versioning/document", false, NativeCaptureScenario::None);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_viewless_extension_download_completes_in_profile_ledger() {
        use cxx_qt_lib::{
            QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl,
            QVariant,
        };
        use ferric_browser_config::DownloadDirectory;

        let download_url = std::env::var("FERRIC_NATIVE_VIEWLESS_DOWNLOAD_URL")
            .expect("xtask must provide the exact loopback download fixture");
        assert!(
            download_url.starts_with("http://127.0.0.1:")
                && download_url.ends_with("/versioning/archive-download"),
            "viewless download must use the exact local fixture route"
        );
        let base = std::env::temp_dir().join(format!(
            "ferric-viewless-download-ledger-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base).expect("disposable normal-profile root");
        let download_directory = base.join("downloads");
        let mut native_config = Config::default();
        native_config.downloads.directory =
            DownloadDirectory::Path(download_directory.to_string_lossy().into_owned());
        native_config.downloads.ask_destination = false;

        crate::register_internal_scheme();
        cxx_qt::init_crate!(ferric_browser_engine_qt);
        cxx_qt::init_qml_module!("io.github.ferricbrowser");
        let app = QGuiApplication::new();
        assert!(app.as_ref().is_some(), "Qt GUI application");
        let mut engine = QQmlApplicationEngine::new();
        let config = serde_json::to_string(&native_config).expect("test download config");
        let contexts = serde_json::to_string(&ContextsConfig::default()).expect("contexts");
        let overrides =
            serde_json::to_string(&RuntimeOverrides::default()).expect("default overrides");
        let mut properties: QMap<QMapPair_QString_QVariant> = QMap::default();
        for (name, value) in [
            ("startupConfigJson", config.as_str()),
            ("startupConfigBaseJson", config.as_str()),
            ("startupContextsJson", contexts.as_str()),
            ("startupCliOverridesJson", overrides.as_str()),
            ("startupProfileOverridesJson", overrides.as_str()),
            (
                "storageBasePath",
                base.to_str().expect("UTF-8 disposable path"),
            ),
        ] {
            properties.insert(QString::from(name), QVariant::from(&QString::from(value)));
        }
        properties.insert(
            QString::from("instanceRequestOwner"),
            QVariant::from(&false),
        );
        engine
            .as_mut()
            .expect("QML engine")
            .set_initial_properties(&properties);
        engine.as_mut().expect("QML engine").load(&QUrl::from(
            "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
        ));
        let state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
            let json = crate::qobject::ferric_browser_test_page_capture_state(
                engine.as_mut().expect("QML engine"),
            );
            serde_json::from_str::<serde_json::Value>(&json.to_string())
                .expect("bounded native download state")
        };
        let ready_deadline = Instant::now() + Duration::from_secs(25);
        let mut bootstrap_started = false;
        loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            bootstrap_started |= current["profile_bootstrap_pending"] == true;
            let profile_ready = current["status"]
                .as_str()
                .is_some_and(|status| status.starts_with("Profile ready"))
                || bootstrap_started && current["profile_bootstrap_pending"] == false;
            if current["ui_ready"] == true
                && profile_ready
                && current["view_ready"] == true
                && current["view_url"] == "about:blank"
                && current["load_state"] == "complete"
            {
                break;
            }
            assert!(
                Instant::now() < ready_deadline,
                "durable profile did not become ready: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        }

        let roots = StorageRoots::resolve(RootSpec::Base(base.clone()))
            .expect("disposable Ferric storage roots");
        let profile_databases = roots.data.join("profiles");
        let database_deadline = Instant::now() + Duration::from_secs(15);
        let database = loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let found = fs::read_dir(&profile_databases)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("browser.sqlite"))
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            if found.len() == 1 {
                break found
                    .into_iter()
                    .next()
                    .expect("one durable profile database");
            }
            assert!(
                Instant::now() < database_deadline,
                "exactly one durable test profile database must exist"
            );
            thread::sleep(Duration::from_millis(10));
        };
        let store = ProfileStore::open(&database, StoreMode::Normal)
            .expect("open the exact disposable profile ledger");

        let extension_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/native/viewless_download_extension");
        assert!(
            crate::qobject::ferric_browser_test_load_viewless_download_extension(
                engine.as_mut().expect("QML engine"),
                &QString::from(extension_path.to_string_lossy().as_ref()),
            ),
            "public MV3 extension manager must load on the exact production profile"
        );
        assert!(
            crate::qobject::ferric_browser_test_observe_viewless_download(
                engine.as_mut().expect("QML engine"),
                &QString::from(download_url.as_str()),
            ),
            "native observer must attach to the exact production profile"
        );
        assert!(
            crate::qobject::ferric_browser_test_open_viewless_download_extension_popup(
                engine.as_mut().expect("QML engine"),
                &QString::from(download_url.as_str()),
            ),
            "test extension popup must start its profile-owned download"
        );
        let completion_deadline = Instant::now() + Duration::from_secs(30);
        let completed = loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let downloads = store.downloads().expect("read durable download ledger");
            if let [download] = downloads.as_slice()
                && download.state == DownloadState::Completed
                && download.bytes_received > 0
            {
                break download.clone();
            }
            assert!(
                Instant::now() < completion_deadline,
                "profile download did not reach one durable completed row"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            crate::qobject::ferric_browser_test_viewless_download_was_observed(
                engine.as_mut().expect("QML engine")
            ),
            "the exact production-profile request must be unique and have page() == nullptr"
        );
        assert_eq!(completed.source_url, download_url);
        assert_eq!(
            completed.bytes_received,
            b"synthetic archive download".len() as i64
        );
        assert!(completed.completed_at.is_some());
        let destination = PathBuf::from(&completed.destination);
        assert!(destination.starts_with(&download_directory));
        assert_eq!(
            fs::read(&destination).expect("completed download destination"),
            b"synthetic archive download"
        );

        drop(store);
        drop(engine);
        drop(app);
        fs::remove_dir_all(&base).expect("remove disposable profile and download files");
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_two_tabs_publish_one_strict_window_capture() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowStrict,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_strict_window_publishes_nothing_after_first_tab_closes() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowStrictViewLoss,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_one_page_after_one_cancelled_save() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialCancel,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_first_page_when_second_save_is_cancelled() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialSecondCancel,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_first_page_when_second_save_times_out() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialSecondTimeout,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_one_page_after_first_page_navigation() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialNavigate,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_first_page_after_second_page_navigation() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialSecondNavigate,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_one_page_after_first_save_deadline() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialTimeout,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_one_page_after_first_tab_closes() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialViewLoss,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_first_page_after_second_tab_closes_during_save() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialSecondViewLoss,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_saved_page_after_its_tab_closes() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialCloseSavedPage,
        );
    }

    #[cfg(feature = "native-archive-fault-test")]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_defers_close_until_archive_install_finishes() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialCloseDuringInstall,
        );
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_one_page_after_archive_copy_storage_full() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialStorageFull,
        );
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_partial_window_retains_first_page_after_second_archive_copy_storage_full() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::WindowPartialSecondStorageFull,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_committed_geometry_applies_in_pristine_restore_window() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::RestoreGeometry,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_committed_multi_page_geometry_keeps_inactive_restore_lazy() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::RestoreMultiPageGeometry,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_committed_draft_requires_review_before_live_refill() {
        native_qt_capture_journey(
            "/versioning/editor-refill",
            false,
            NativeCaptureScenario::RestoreEditableRefill,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_mutating_page_publishes_partial_fidelity() {
        native_qt_capture_journey("/versioning/mutation", true, NativeCaptureScenario::None);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_post_result_publishes_nonreplayable_archive() {
        native_qt_capture_journey("/versioning/form", false, NativeCaptureScenario::PostForm);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_post_redirect_303_publishes_result_without_replay() {
        native_qt_capture_journey(
            "/versioning/redirect-303",
            false,
            NativeCaptureScenario::PostRedirect303,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_post_redirect_307_preserves_body_and_publishes() {
        native_qt_capture_journey(
            "/versioning/redirect-307",
            false,
            NativeCaptureScenario::PostRedirect307,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_secondary_post_result_does_not_replay_as_get() {
        native_qt_capture_journey(
            "/versioning/form",
            false,
            NativeCaptureScenario::SecondaryPostForm,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_frame_tree_excludes_cross_origin_and_publishes() {
        native_qt_capture_journey("/versioning/frames", false, NativeCaptureScenario::None);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_editor_opt_in_publishes_bounded_text_draft() {
        native_qt_capture_journey("/versioning/editor", false, NativeCaptureScenario::None);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_selection_opt_in_publishes_offsets_without_selected_text() {
        native_qt_capture_journey(
            "/versioning/editor",
            false,
            NativeCaptureScenario::SelectionEditor,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_media_metadata_publishes_without_playback() {
        native_qt_capture_journey("/versioning/media", false, NativeCaptureScenario::None);
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_committed_paused_media_position_restores_without_playback() {
        native_qt_capture_journey(
            "/versioning/media",
            false,
            NativeCaptureScenario::RestoreMediaPosition,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_navigation_during_capture_does_not_publish_mixed_document() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::Navigate,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_removed_child_frame_during_save_does_not_publish_mixed_document() {
        native_qt_capture_journey(
            "/versioning/frames",
            false,
            NativeCaptureScenario::RemoveFrameDuringSave,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_same_url_child_reload_during_save_does_not_publish_mixed_document() {
        native_qt_capture_journey(
            "/versioning/frames",
            false,
            NativeCaptureScenario::ReloadFrameDuringSave,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_cancelled_archive_after_collector_does_not_publish() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::CancelDownload,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_declined_capture_requires_new_one_use_disclosure() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::DeclineThenCapture,
        );
    }

    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_collector_timeout_keeps_successful_archive_only() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::CollectorTimeout,
        );
    }

    #[cfg(feature = "native-archive-fault-test")]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_ui_responds_while_archive_worker_copy_is_paused() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::ArchiveInstallPause,
        );
    }

    #[cfg(feature = "native-archive-fault-test")]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_cancel_quit_waits_for_archive_worker_drain() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::ArchiveInstallShutdown,
        );
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_archive_install_storage_full_publishes_no_commit() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::ArchiveInstallStorageFull,
        );
    }

    #[cfg(feature = "native-archive-fault-test")]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_sidecar_worker_refusal_keeps_successful_archive_only() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::SidecarWorkerRefusal,
        );
    }

    #[cfg(all(feature = "native-archive-fault-test", unix))]
    #[test]
    #[ignore = "requires disposable loopback fixture and native Qt GUI runtime"]
    fn native_qt_sidecar_filesystem_denial_keeps_successful_archive_only() {
        native_qt_capture_journey(
            "/versioning/document",
            false,
            NativeCaptureScenario::SidecarFilesystemPermissionDenied,
        );
    }

    fn native_qt_capture_journey(
        expected_path: &str,
        expect_mutation: bool,
        interruption: NativeCaptureScenario,
    ) {
        use cxx_qt_lib::{
            QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl,
            QVariant,
        };

        let mut source_url = std::env::var("FERRIC_NATIVE_CAPTURE_URL")
            .expect("xtask must provide loopback page fixture");
        assert!(
            source_url.starts_with("http://127.0.0.1:") && source_url.ends_with(expected_path),
            "native capture fixture must match the exact loopback route"
        );
        let fixture_origin = source_url
            .strip_suffix(expected_path)
            .expect("exact loopback fixture origin")
            .to_owned();
        let base =
            std::env::temp_dir().join(format!("ferric-live-page-capture-{}", Uuid::new_v4()));
        fs::create_dir(&base).expect("disposable normal-profile root");
        crate::register_internal_scheme();
        cxx_qt::init_crate!(ferric_browser_engine_qt);
        cxx_qt::init_qml_module!("io.github.ferricbrowser");
        let app = QGuiApplication::new();
        assert!(app.as_ref().is_some(), "Qt GUI application");
        let mut engine = QQmlApplicationEngine::new();
        let config = serde_json::to_string(&Config::default()).expect("default config");
        let contexts = serde_json::to_string(&ContextsConfig::default()).expect("contexts");
        let overrides =
            serde_json::to_string(&RuntimeOverrides::default()).expect("default overrides");
        let mut properties: QMap<QMapPair_QString_QVariant> = QMap::default();
        for (name, value) in [
            ("startupConfigJson", config.as_str()),
            ("startupConfigBaseJson", config.as_str()),
            ("startupContextsJson", contexts.as_str()),
            ("startupCliOverridesJson", overrides.as_str()),
            ("startupProfileOverridesJson", overrides.as_str()),
            (
                "storageBasePath",
                base.to_str().expect("UTF-8 disposable path"),
            ),
        ] {
            properties.insert(QString::from(name), QVariant::from(&QString::from(value)));
        }
        properties.insert(
            QString::from("instanceRequestOwner"),
            QVariant::from(&false),
        );
        engine
            .as_mut()
            .expect("QML engine")
            .set_initial_properties(&properties);
        engine.as_mut().expect("QML engine").load(&QUrl::from(
            "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
        ));
        let state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
            let json = crate::qobject::ferric_browser_test_page_capture_state(
                engine.as_mut().expect("QML engine"),
            );
            serde_json::from_str::<serde_json::Value>(&json.to_string())
                .expect("bounded native capture state")
        };
        let ready_deadline = Instant::now() + Duration::from_secs(25);
        let mut bootstrap_started = false;
        let mut profile_ready = false;
        loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            bootstrap_started |= current["profile_bootstrap_pending"] == true;
            profile_ready |= current["status"]
                .as_str()
                .is_some_and(|status| status.starts_with("Profile ready"))
                || bootstrap_started && current["profile_bootstrap_pending"] == false;
            if current["ui_ready"] == true
                && profile_ready
                && current["view_ready"] == true
                && current["view_url"] == "about:blank"
                && current["load_state"] == "complete"
            {
                break;
            }
            assert!(
                Instant::now() < ready_deadline,
                "profile not ready: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            crate::qobject::ferric_browser_test_navigate_page_capture_source(
                engine.as_mut().expect("QML engine"),
                &QString::from(source_url.as_str()),
            )
        );
        let requested = state(&mut engine);
        assert_eq!(
            requested["current_url"], source_url,
            "navigation was refused: {requested}"
        );
        let load_deadline = Instant::now() + Duration::from_secs(25);
        loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            if current["load_state"] == "complete"
                && current["current_url"] == source_url
                && current["view_url"] == source_url
            {
                break;
            }
            assert!(
                Instant::now() < load_deadline,
                "source did not load: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        }
        if interruption.is_window() {
            let second_url = format!("{fixture_origin}/versioning/controlled");
            assert!(
                crate::qobject::ferric_browser_test_open_second_page_capture_tab(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(second_url.as_str()),
                ),
                "second tab must use the production tab and navigation routes"
            );
            let second_load_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["tab_count"] == 2
                    && current["active_tab_index"] == 1
                    && current["load_state"] == "complete"
                    && current["current_url"] == second_url
                    && current["view_url"] == second_url
                {
                    break;
                }
                assert!(
                    Instant::now() < second_load_deadline,
                    "second live tab did not load: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::RestoreMultiPageGeometry {
            assert!(crate::qobject::ferric_browser_test_select_page_capture_tab(
                engine.as_mut().expect("QML engine"),
                0,
            ));
            let selected_deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["active_tab_index"] == 0
                    && current["current_url"] == source_url
                    && current["view_url"] == source_url
                    && current["load_state"] == "complete"
                {
                    break;
                }
                assert!(
                    Instant::now() < selected_deadline,
                    "first geometry page did not become selected: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::SecondaryPostForm {
            let startup = QString::from(source_url.as_str());
            assert!(
                crate::qobject::ferric_browser_test_open_secondary_page_capture_form(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                ),
                "same-profile secondary window must open"
            );
            let secondary_state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
                let json = crate::qobject::ferric_browser_test_secondary_page_capture_state(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                );
                serde_json::from_str::<serde_json::Value>(&json.to_string())
                    .expect("bounded secondary native state")
            };
            let secondary_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = secondary_state(&mut engine);
                if current["view_url"] == source_url
                    && current["current_url"] == source_url
                    && current["load_state"] == "complete"
                {
                    break;
                }
                assert!(
                    Instant::now() < secondary_deadline,
                    "secondary form did not load: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                crate::qobject::ferric_browser_test_submit_secondary_page_capture_form(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                )
            );
            let result_url = format!("{fixture_origin}/versioning/form/result");
            let post_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = secondary_state(&mut engine);
                if current["view_url"] == result_url
                    && current["current_url"] == result_url
                    && current["load_state"] == "complete"
                    && current["url_binding_suspended"] == true
                {
                    break;
                }
                assert!(
                    Instant::now() < post_deadline,
                    "secondary POST result did not remain loaded: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            let replacement_url = format!("{fixture_origin}/versioning/controlled");
            assert!(
                crate::qobject::ferric_browser_test_navigate_secondary_page_capture(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                    &QString::from(replacement_url.as_str()),
                )
            );
            let navigation_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = secondary_state(&mut engine);
                if current["view_url"] == replacement_url
                    && current["current_url"] == replacement_url
                    && current["load_state"] == "complete"
                    && current["url_binding_suspended"] == false
                {
                    break;
                }
                assert!(
                    Instant::now() < navigation_deadline,
                    "secondary navigation did not rearm after POST: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                crate::qobject::ferric_browser_test_request_secondary_page_capture_close(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                )
            );
            assert_eq!(
                secondary_state(&mut engine)["close_confirmation_visible"],
                true,
                "ordinary close must request confirmation"
            );
            assert!(
                crate::qobject::ferric_browser_test_continue_secondary_page_capture_close(
                    engine.as_mut().expect("QML engine"),
                    &startup,
                )
            );
            let child_shutdown_deadline = Instant::now() + Duration::from_secs(20);
            let mut page_close_confirmed = false;
            let mut last_storage_retry: Option<Instant> = None;
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = secondary_state(&mut engine);
                if current["window_ready"] == false {
                    break;
                }
                if current["page_prompt_visible"] == true && !page_close_confirmed {
                    assert!(
                        crate::qobject::ferric_browser_test_confirm_secondary_page_capture_close(
                            engine.as_mut().expect("QML engine"),
                            &startup,
                        ),
                        "page-state close confirmation must be accepted"
                    );
                    page_close_confirmed = true;
                }
                if current["storage_prompt_visible"] == true
                    && last_storage_retry
                        .is_none_or(|last| last.elapsed() >= Duration::from_millis(100))
                {
                    assert!(
                        crate::qobject::ferric_browser_test_retry_secondary_page_capture_close(
                            engine.as_mut().expect("QML engine"),
                            &startup,
                        ),
                        "visible durable-flush prompt must offer an explicit retry"
                    );
                    last_storage_retry = Some(Instant::now());
                }
                assert_eq!(
                    current["shutdown_prompt_visible"], false,
                    "secondary unexpectedly reported active work: {current}"
                );
                assert!(
                    Instant::now() < child_shutdown_deadline,
                    "secondary normal close did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                engine.as_mut().expect("QML engine"),
            ));
            let shutdown_deadline = Instant::now() + Duration::from_secs(20);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["shutdown_approved"] == true || current["root_ready"] == false {
                    break;
                }
                assert!(
                    Instant::now() < shutdown_deadline,
                    "secondary shutdown did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(engine);
            drop(app);
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        if matches!(
            interruption,
            NativeCaptureScenario::PostForm
                | NativeCaptureScenario::PostRedirect303
                | NativeCaptureScenario::PostRedirect307
        ) {
            assert!(
                crate::qobject::ferric_browser_test_submit_page_capture_form(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
            let result_path = match interruption {
                NativeCaptureScenario::PostRedirect303 => "/versioning/redirect-303/result",
                NativeCaptureScenario::PostRedirect307 => "/versioning/redirect-307/result",
                _ => "/versioning/form/result",
            };
            let result_url = format!("{fixture_origin}{result_path}");
            let post_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["load_state"] == "complete"
                    && current["current_url"] == result_url
                    && current["view_url"] == result_url
                {
                    break;
                }
                assert!(
                    Instant::now() < post_deadline,
                    "POST result did not load: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            source_url = result_url;
        }
        if expected_path == "/versioning/media" {
            let media_deadline = Instant::now() + Duration::from_secs(15);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["view_title"] == "Versioning media ready" {
                    break;
                }
                assert!(
                    Instant::now() < media_deadline,
                    "local media metadata did not load: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::RestoreMediaPosition {
            assert!(
                crate::qobject::ferric_browser_test_prepare_page_capture_media_position(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
            let prepared_deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["view_title"] == "Versioning media prepared" {
                    break;
                }
                assert!(
                    Instant::now() < prepared_deadline,
                    "paused media position setup failed: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::CollectorTimeout {
            assert!(
                crate::qobject::ferric_browser_test_delay_page_capture_renderer(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
        }
        if matches!(
            interruption,
            NativeCaptureScenario::RestoreGeometry
                | NativeCaptureScenario::RestoreMultiPageGeometry
        ) {
            assert!(
                crate::qobject::ferric_browser_test_prepare_page_capture_geometry(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
            let prepared_deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["view_title"] == "Versioning geometry prepared" {
                    break;
                }
                assert!(
                    Instant::now() < prepared_deadline,
                    "geometry setup failed: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::SelectionEditor {
            assert!(
                crate::qobject::ferric_browser_test_prepare_page_capture_selection(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
            let prepared_deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["view_title"] == "Versioning selection prepared" {
                    break;
                }
                assert!(
                    Instant::now() < prepared_deadline,
                    "selection setup failed: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::RestoreEditableRefill {
            assert!(
                crate::qobject::ferric_browser_test_prepare_page_capture_refill_draft(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(source_url.as_str()),
                )
            );
            let prepared_deadline = Instant::now() + Duration::from_secs(10);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["view_title"] == "Versioning refill prepared" {
                    break;
                }
                assert!(
                    Instant::now() < prepared_deadline,
                    "refill draft setup failed: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if interruption == NativeCaptureScenario::DeclineThenCapture {
            assert!(crate::qobject::ferric_browser_test_request_page_capture(
                engine.as_mut().expect("QML engine"),
                false,
                false,
            ));
            let declined = crate::qobject::ferric_browser_test_page_capture_disclosure_operation(
                engine.as_mut().expect("QML engine"),
            );
            assert!(!declined.is_empty(), "first native disclosure must exist");
            assert!(crate::qobject::ferric_browser_test_decline_page_capture(
                engine.as_mut().expect("QML engine"),
                &declined,
            ));
            assert!(
                !crate::qobject::ferric_browser_test_accept_page_capture(
                    engine.as_mut().expect("QML engine"),
                    &declined,
                ),
                "a declined operation cannot be accepted later"
            );
            assert!(
                crate::qobject::ferric_browser_test_page_capture_disclosure_operation(
                    engine.as_mut().expect("QML engine"),
                )
                .is_empty(),
                "decline must retire the exact pending disclosure"
            );
        }
        #[cfg(feature = "native-archive-fault-test")]
        if interruption == NativeCaptureScenario::SidecarWorkerRefusal {
            ferric_browser_storage::versioning::arm_one_sidecar_install_refusal();
        }
        #[cfg(feature = "native-archive-fault-test")]
        if matches!(
            interruption,
            NativeCaptureScenario::ArchiveInstallPause
                | NativeCaptureScenario::ArchiveInstallShutdown
                | NativeCaptureScenario::WindowPartialCloseDuringInstall
        ) {
            ferric_browser_storage::versioning::arm_one_archive_install_pause();
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if matches!(
            interruption,
            NativeCaptureScenario::ArchiveInstallStorageFull
                | NativeCaptureScenario::WindowPartialStorageFull
        ) {
            ferric_browser_storage::versioning::arm_one_archive_install_storage_full();
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if interruption == NativeCaptureScenario::WindowPartialSecondStorageFull {
            ferric_browser_storage::versioning::arm_archive_install_storage_full_after(1);
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if interruption == NativeCaptureScenario::SidecarFilesystemPermissionDenied {
            ferric_browser_storage::versioning::arm_one_sidecar_install_pause();
        }
        let consent_requested = if interruption.is_window() {
            crate::qobject::ferric_browser_test_request_window_page_capture(
                engine.as_mut().expect("QML engine"),
                false,
                false,
                interruption.is_partial_window(),
            )
        } else {
            crate::qobject::ferric_browser_test_request_page_capture(
                engine.as_mut().expect("QML engine"),
                matches!(
                    expected_path,
                    "/versioning/editor" | "/versioning/editor-refill"
                ) && interruption != NativeCaptureScenario::SelectionEditor,
                interruption == NativeCaptureScenario::SelectionEditor,
            )
        };
        assert!(consent_requested);
        let operation = crate::qobject::ferric_browser_test_page_capture_disclosure_operation(
            engine.as_mut().expect("QML engine"),
        );
        assert!(!operation.is_empty(), "fixed native disclosure must exist");
        assert!(crate::qobject::ferric_browser_test_accept_page_capture(
            engine.as_mut().expect("QML engine"),
            &operation,
        ));
        // Reloading a child frame can leave Qt's page-save callback pending.
        // The production host must retire that save at its 90-second native
        // deadline; do not fail this negative journey before that boundary.
        let capture_deadline = Instant::now()
            + if interruption == NativeCaptureScenario::ReloadFrameDuringSave {
                Duration::from_secs(105)
            } else {
                Duration::from_secs(45)
            };
        let mut capture_observed = false;
        let mut last_capture_state = serde_json::Value::Null;
        let mut sidecar_accepted = false;
        let mut frame_public_projections_checked = false;
        let mut collector_timed_out = false;
        let mut interruption_triggered = false;
        let mut captured_view_close_completed = false;
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        let mut stage_permissions: Option<StagePermissionsGuard> = None;
        let replacement_url = format!("{fixture_origin}/versioning/controlled");
        let later_page_url = format!("{replacement_url}?capture-race=second");
        loop {
            #[cfg(all(feature = "native-archive-fault-test", unix))]
            if interruption == NativeCaptureScenario::SidecarFilesystemPermissionDenied
                && ferric_browser_storage::versioning::sidecar_permission_denials() == 1
            {
                if let Some(mut guard) = stage_permissions.take() {
                    guard.restore();
                }
            }
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            capture_observed |= current["native_capture_active"] == true;
            if current["native_capture_active"] == true {
                last_capture_state = current.clone();
            }
            sidecar_accepted |= current["sidecar_accepted"] == true;
            collector_timed_out |= current["collector_timed_out"] == true;
            #[cfg(all(feature = "native-archive-fault-test", unix))]
            if interruption == NativeCaptureScenario::WindowPartialStorageFull
                || interruption == NativeCaptureScenario::WindowPartialSecondStorageFull
            {
                if ferric_browser_storage::versioning::injected_archive_install_storage_fulls() == 1
                {
                    interruption_triggered = true;
                }
            }
            #[cfg(feature = "native-archive-fault-test")]
            if matches!(
                interruption,
                NativeCaptureScenario::ArchiveInstallPause
                    | NativeCaptureScenario::ArchiveInstallShutdown
                    | NativeCaptureScenario::WindowPartialCloseDuringInstall
            ) && !interruption_triggered
                && ferric_browser_storage::versioning::archive_install_paused()
            {
                assert_eq!(current["save_requested"], true);
                let expected_active_url =
                    if interruption == NativeCaptureScenario::WindowPartialCloseDuringInstall {
                        replacement_url.as_str()
                    } else {
                        source_url.as_str()
                    };
                assert_eq!(current["view_url"], expected_active_url);
                if interruption == NativeCaptureScenario::ArchiveInstallShutdown {
                    assert_eq!(current["download_finished"], true);
                    assert!(
                        crate::qobject::ferric_browser_test_cancel_quit_and_drain_archive_worker(
                            engine.as_mut().expect("QML engine"),
                        ),
                        "cancel-and-quit must keep waiting for the paused archive worker"
                    );
                    let waiting = state(&mut engine);
                    assert_eq!(waiting["shutdown_waiting_for_work"], true);
                    assert_eq!(waiting["shutdown_prompt_visible"], true);
                    assert_eq!(waiting["native_capture_active"], false);
                    assert_eq!(waiting["pending_versioning_object_work"], true);
                    assert_eq!(waiting["capture_request_active"], false);
                    assert_eq!(waiting["shutdown_in_progress"], false);
                    assert!(
                        ferric_browser_storage::versioning::archive_install_paused(),
                        "the injected archive worker must remain paused while shutdown waits"
                    );
                }
                if interruption == NativeCaptureScenario::WindowPartialCloseDuringInstall {
                    assert_eq!(current["download_finished"], true);
                    assert!(
                        crate::qobject::ferric_browser_test_close_during_capture_install_is_deferred(
                            engine.as_mut().expect("QML engine"),
                            &operation,
                        ),
                        "the view must remain live until archive installation settles: {current}"
                    );
                }
                app.as_ref().expect("Qt GUI application").process_events();
                let responsive = state(&mut engine);
                assert_eq!(responsive["view_url"], expected_active_url);
                assert!(
                    ferric_browser_storage::versioning::archive_install_paused(),
                    "the UI probe must complete before the worker resumes"
                );
                ferric_browser_storage::versioning::resume_archive_install();
                interruption_triggered = true;
            }
            if expected_path == "/versioning/frames"
                && interruption == NativeCaptureScenario::None
                && current["sidecar_accepted"] == true
                && !frame_public_projections_checked
            {
                let raw = crate::qobject::ferric_browser_test_page_capture_public_projections(
                    engine.as_mut().expect("QML engine"),
                )
                .to_string();
                let public: serde_json::Value =
                    serde_json::from_str(&raw).expect("live diagnostics and command projection");
                let diagnostics = public["diagnostics"]
                    .as_str()
                    .expect("public diagnostics JSON");
                assert!(serde_json::from_str::<serde_json::Value>(diagnostics).is_ok());
                assert_eq!(public["command_history_previous"], "");
                let child_url = format!("{fixture_origin}/versioning/frame-child?kind=same");
                let child_route = route_fingerprint_for_url(&child_url)
                    .expect("same-origin child route fingerprint");
                for marker in [
                    "/versioning/frame-child",
                    "Synthetic child frame",
                    child_route.as_str(),
                ] {
                    assert!(
                        !current.to_string().contains(marker) && !raw.contains(marker),
                        "child frame metadata leaked into ordinary projection: {marker}"
                    );
                }
                frame_public_projections_checked = true;
            }
            #[cfg(all(feature = "native-archive-fault-test", unix))]
            if interruption == NativeCaptureScenario::SidecarFilesystemPermissionDenied
                && !interruption_triggered
                && ferric_browser_storage::versioning::sidecar_install_paused()
            {
                use std::os::unix::fs::PermissionsExt;

                let state_root = StorageRoots::resolve(RootSpec::Base(base.clone()))
                    .expect("disposable storage roots")
                    .state;
                let mut profiles = fs::read_dir(state_root.join("versioning"))
                    .expect("disposable versioning profiles")
                    .map(|entry| entry.expect("profile entry").path())
                    .collect::<Vec<_>>();
                assert_eq!(profiles.len(), 1, "one disposable profile only");
                let profile_root = profiles.pop().expect("disposable profile");
                assert!(
                    profile_root
                        .file_name()
                        .and_then(|name| name.to_str())
                        .and_then(|name| Uuid::parse_str(name).ok())
                        .is_some(),
                    "fault target must be a UUID-owned profile"
                );
                let operation_id = operation
                    .to_string()
                    .parse::<OperationId>()
                    .expect("typed native capture operation");
                let capture_stage_directory = profile_root
                    .join("staging")
                    .join(operation_id.as_uuid().hyphenated().to_string());
                let metadata =
                    fs::symlink_metadata(&capture_stage_directory).expect("exact capture stage");
                assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
                let original = metadata.permissions();
                assert_eq!(original.mode() & 0o777, 0o700);
                assert!(!capture_stage_directory.join("sidecar.tmp").exists());
                fs::set_permissions(&capture_stage_directory, fs::Permissions::from_mode(0o500))
                    .expect("remove write access from disposable capture stage");
                stage_permissions = Some(StagePermissionsGuard {
                    path: capture_stage_directory,
                    original,
                    restored: false,
                });
                ferric_browser_storage::versioning::resume_sidecar_install();
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::Navigate
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
            {
                assert!(
                    crate::qobject::ferric_browser_test_navigate_page_capture_source(
                        engine.as_mut().expect("QML engine"),
                        &QString::from(replacement_url.as_str()),
                    )
                );
                let requested = state(&mut engine);
                assert_eq!(
                    requested["current_url"], replacement_url,
                    "replacement navigation was refused: {requested}"
                );
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::WindowPartialNavigate
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
            {
                assert!(
                    crate::qobject::ferric_browser_test_navigate_first_page_capture_tab(
                        engine.as_mut().expect("QML engine"),
                        &QString::from(replacement_url.as_str()),
                    ),
                    "the captured first tab must navigate through the production route"
                );
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::WindowPartialSecondNavigate
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
                && current["capture_tab_index"] == 1
            {
                assert!(
                    crate::qobject::ferric_browser_test_navigate_second_page_capture_tab(
                        engine.as_mut().expect("QML engine"),
                        &QString::from(later_page_url.as_str()),
                    ),
                    "the captured second tab must navigate through the production route"
                );
                interruption_triggered = true;
            }
            if matches!(
                interruption,
                NativeCaptureScenario::CancelDownload
                    | NativeCaptureScenario::WindowPartialCancel
                    | NativeCaptureScenario::WindowPartialSecondCancel
            ) && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["download_started"] == true
                && (interruption != NativeCaptureScenario::WindowPartialSecondCancel
                    || current["capture_tab_index"] == 1)
            {
                assert!(
                    crate::qobject::ferric_browser_test_cancel_page_capture_download(
                        engine.as_mut().expect("QML engine"),
                        &operation,
                    )
                );
                interruption_triggered = true;
            }
            if matches!(
                interruption,
                NativeCaptureScenario::WindowPartialTimeout
                    | NativeCaptureScenario::WindowPartialSecondTimeout
            ) && !interruption_triggered
                && current["save_requested"] == true
                && current["download_finished"] == false
                && (interruption != NativeCaptureScenario::WindowPartialSecondTimeout
                    || current["capture_tab_index"] == 1)
            {
                assert!(
                    crate::qobject::ferric_browser_test_expire_page_capture_save(
                        engine.as_mut().expect("QML engine"),
                        &operation,
                    ),
                    "the first page must still own its native save deadline: {current}"
                );
                interruption_triggered = true;
            }
            if matches!(
                interruption,
                NativeCaptureScenario::WindowPartialViewLoss
                    | NativeCaptureScenario::WindowPartialSecondViewLoss
                    | NativeCaptureScenario::WindowStrictViewLoss
            ) && !interruption_triggered
                && current["save_requested"] == true
                && current["download_finished"] == false
                && (interruption != NativeCaptureScenario::WindowPartialSecondViewLoss
                    || current["capture_tab_index"] == 1)
            {
                let closed = if interruption == NativeCaptureScenario::WindowPartialSecondViewLoss {
                    crate::qobject::ferric_browser_test_close_second_page_capture_tab(
                        engine.as_mut().expect("QML engine"),
                        &operation,
                    )
                } else {
                    crate::qobject::ferric_browser_test_close_first_page_capture_tab(
                        engine.as_mut().expect("QML engine"),
                        &operation,
                    )
                };
                assert!(
                    closed,
                    "the exact captured tab must close through the production route: {current}"
                );
                let after_close_request = state(&mut engine);
                if current["download_started"] == true {
                    assert_eq!(after_close_request["tab_count"], 1);
                } else {
                    assert_eq!(after_close_request["tab_count"], 2);
                    assert_eq!(
                        after_close_request["download_started"], false,
                        "the view must remain alive until Qt registers the queued profile request"
                    );
                    assert_eq!(
                        after_close_request["status"],
                        "Tab close is waiting for native archive request registration"
                    );
                }
                interruption_triggered = true;
            }
            if matches!(
                interruption,
                NativeCaptureScenario::WindowPartialViewLoss
                    | NativeCaptureScenario::WindowPartialSecondViewLoss
                    | NativeCaptureScenario::WindowStrictViewLoss
            ) && interruption_triggered
                && current["tab_count"] == 1
            {
                captured_view_close_completed = true;
            }
            if interruption == NativeCaptureScenario::WindowPartialCloseSavedPage
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
                && current["download_finished"] == false
                && crate::qobject::ferric_browser_test_close_completed_page_capture_tab(
                    engine.as_mut().expect("QML engine"),
                    &operation,
                )
            {
                assert_eq!(state(&mut engine)["tab_count"], 1);
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::RemoveFrameDuringSave
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
            {
                assert!(
                    crate::qobject::ferric_browser_test_remove_page_capture_frame(
                        engine.as_mut().expect("QML engine"),
                        &QString::from(source_url.as_str()),
                    )
                );
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::ReloadFrameDuringSave
                && !interruption_triggered
                && current["sidecar_accepted"] == true
                && current["save_requested"] == true
            {
                assert!(
                    crate::qobject::ferric_browser_test_reload_page_capture_frame(
                        engine.as_mut().expect("QML engine"),
                        &QString::from(source_url.as_str()),
                    )
                );
                interruption_triggered = true;
            }
            if interruption == NativeCaptureScenario::Navigate
                && interruption_triggered
                && current["native_capture_active"] == false
                && current["view_url"] == replacement_url
                && current["load_state"] == "complete"
            {
                break;
            }
            if interruption == NativeCaptureScenario::CancelDownload
                && interruption_triggered
                && current["native_capture_active"] == false
                && current["capture_request_active"] == false
            {
                break;
            }
            if interruption == NativeCaptureScenario::RemoveFrameDuringSave
                && interruption_triggered
                && current["native_capture_active"] == false
                && current["capture_request_active"] == false
                && current["view_title"] == "Versioning frame removed"
            {
                break;
            }
            if interruption == NativeCaptureScenario::ReloadFrameDuringSave
                && interruption_triggered
                && current["native_capture_active"] == false
                && current["capture_request_active"] == false
                && current["view_title"] == "Versioning frame reloaded"
            {
                break;
            }
            if current["status"] == "Page capture saved; refresh the versioning list to inspect it"
                && (!matches!(
                    interruption,
                    NativeCaptureScenario::WindowPartialViewLoss
                        | NativeCaptureScenario::WindowPartialSecondViewLoss
                        | NativeCaptureScenario::WindowStrictViewLoss
                ) || current["tab_count"] == 1)
            {
                assert!(
                    !interruption.expects_no_commit(),
                    "capture published before requested interruption: {current}"
                );
                break;
            }
            if interruption.is_shutdown_drain()
                && interruption_triggered
                && (current["shutdown_approved"] == true || current["root_ready"] == false)
            {
                break;
            }
            if capture_observed
                && current["native_capture_active"] == false
                && current["capture_request_active"] == false
                && (!matches!(
                    interruption,
                    NativeCaptureScenario::WindowPartialViewLoss
                        | NativeCaptureScenario::WindowPartialSecondViewLoss
                        | NativeCaptureScenario::WindowStrictViewLoss
                ) || current["tab_count"] == 1)
                && !matches!(
                    interruption,
                    NativeCaptureScenario::Navigate
                        | NativeCaptureScenario::RemoveFrameDuringSave
                        | NativeCaptureScenario::ReloadFrameDuringSave
                        | NativeCaptureScenario::CancelDownload
                        | NativeCaptureScenario::WindowPartialCloseSavedPage
                        | NativeCaptureScenario::WindowPartialCloseDuringInstall
                )
                && !interruption.is_shutdown_drain()
            {
                // Status can be overwritten by a later session checkpoint. The reopened
                // profile assertions below prove whether publication actually completed.
                break;
            }
            assert!(
                Instant::now() < capture_deadline,
                "production capture did not publish: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            sidecar_accepted,
            interruption != NativeCaptureScenario::CollectorTimeout,
            "collector outcome did not match the native scenario; last active capture state: {last_capture_state}"
        );
        if expected_path == "/versioning/frames" && interruption == NativeCaptureScenario::None {
            assert!(
                frame_public_projections_checked,
                "accepted child sidecar never reached the public projection probe"
            );
        }
        assert_eq!(
            collector_timed_out,
            interruption == NativeCaptureScenario::CollectorTimeout,
            "collector deadline did not match the native scenario"
        );
        #[cfg(feature = "native-archive-fault-test")]
        if interruption == NativeCaptureScenario::ArchiveInstallPause {
            assert!(
                interruption_triggered,
                "archive worker pause was not observed"
            );
        }
        #[cfg(feature = "native-archive-fault-test")]
        if interruption == NativeCaptureScenario::ArchiveInstallShutdown {
            assert!(
                interruption_triggered,
                "archive worker shutdown was not exercised"
            );
            let settled = state(&mut engine);
            assert!(
                settled["shutdown_approved"] == true || settled["root_ready"] == false,
                "cancel-and-quit did not settle after the worker drained: {settled}"
            );
            if settled["root_ready"] == true {
                assert_eq!(settled["shutdown_waiting_for_work"], false);
                assert_eq!(settled["pending_versioning_object_work"], false);
            }
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if matches!(
            interruption,
            NativeCaptureScenario::ArchiveInstallStorageFull
                | NativeCaptureScenario::WindowPartialStorageFull
                | NativeCaptureScenario::WindowPartialSecondStorageFull
        ) {
            assert_eq!(
                ferric_browser_storage::versioning::injected_archive_install_storage_fulls(),
                1,
                "the archive object copy must encounter one injected ENOSPC"
            );
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if interruption == NativeCaptureScenario::SidecarFilesystemPermissionDenied {
            assert!(
                interruption_triggered,
                "worker never reached the filesystem fault"
            );
            assert_eq!(
                ferric_browser_storage::versioning::sidecar_permission_denials(),
                1,
                "the operating system must deny exactly one sidecar write"
            );
            assert!(
                stage_permissions.is_none(),
                "staging permissions must be restored"
            );
        }
        if matches!(
            interruption,
            NativeCaptureScenario::Navigate
                | NativeCaptureScenario::RemoveFrameDuringSave
                | NativeCaptureScenario::ReloadFrameDuringSave
                | NativeCaptureScenario::CancelDownload
                | NativeCaptureScenario::WindowPartialCancel
                | NativeCaptureScenario::WindowPartialSecondCancel
                | NativeCaptureScenario::WindowPartialSecondTimeout
                | NativeCaptureScenario::WindowPartialNavigate
                | NativeCaptureScenario::WindowPartialSecondNavigate
                | NativeCaptureScenario::WindowPartialViewLoss
                | NativeCaptureScenario::WindowPartialSecondViewLoss
                | NativeCaptureScenario::WindowPartialCloseSavedPage
                | NativeCaptureScenario::WindowPartialCloseDuringInstall
                | NativeCaptureScenario::WindowStrictViewLoss
        ) {
            assert!(
                interruption_triggered,
                "native save did not expose the requested interruption interval"
            );
        }
        let settled_capture_state = state(&mut engine);
        if matches!(
            interruption,
            NativeCaptureScenario::WindowPartialViewLoss
                | NativeCaptureScenario::WindowPartialSecondViewLoss
                | NativeCaptureScenario::WindowStrictViewLoss
        ) {
            assert!(
                captured_view_close_completed || settled_capture_state["tab_count"] == 1,
                "a queued captured-tab close must complete only after the profile callback: {settled_capture_state}"
            );
        }
        if matches!(
            interruption,
            NativeCaptureScenario::PostForm
                | NativeCaptureScenario::PostRedirect303
                | NativeCaptureScenario::PostRedirect307
        ) {
            assert_eq!(
                state(&mut engine)["url_binding_suspended"],
                true,
                "completed POST result must remain under native view ownership"
            );
            assert!(
                crate::qobject::ferric_browser_test_navigate_page_capture_source(
                    engine.as_mut().expect("QML engine"),
                    &QString::from(replacement_url.as_str()),
                ),
                "normal navigation after POST must be accepted"
            );
            let navigate_deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["current_url"] == replacement_url
                    && current["view_url"] == replacement_url
                    && current["load_state"] == "complete"
                    && current["url_binding_suspended"] == false
                {
                    break;
                }
                assert!(
                    Instant::now() < navigate_deadline,
                    "ordinary navigation did not resume after POST capture: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        if !interruption.is_shutdown_drain() {
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                engine.as_mut().expect("QML engine"),
            ));
            let shutdown_deadline = Instant::now() + Duration::from_secs(20);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["shutdown_approved"] == true || current["root_ready"] == false {
                    break;
                }
                assert!(
                    Instant::now() < shutdown_deadline,
                    "shutdown did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        drop(engine);

        let (mut application, _, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("reopen application");
        application
            .request_profile_open(ProfileOpenRequest {
                name: "default".into(),
                label: "Default".into(),
                privacy: PrivacyKind::Normal,
                storage: ProfileStorage::Durable(RootSpec::Base(base.clone())),
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: RuntimeOverrides::default(),
                config_path: None,
                contexts: ContextsConfig::default(),
                context_configuration_issue: false,
            })
            .expect("reopen durable profile request");
        let reopen_deadline = Instant::now() + Duration::from_secs(10);
        let profile = loop {
            if let Some(result) = application.poll_profile_open() {
                break result.expect("durable profile").profile;
            }
            assert!(Instant::now() < reopen_deadline, "profile reopen timed out");
            thread::yield_now();
        };
        let profile_uuid = profile.id.expect("durable profile identity");
        let ticket = application
            .submit_storage(StorageRequest::VersioningList {
                profile_uuid,
                limit: 10,
                offset: 0,
            })
            .expect("native capture commit list admitted");
        let list_deadline = Instant::now() + Duration::from_secs(10);
        let commits = loop {
            if let Some(effect) = application
                .poll_storage_effects()
                .into_iter()
                .find(|effect| effect.ticket == ticket)
            {
                let StorageCompletion::VersioningList(Ok(commits)) = effect.completion else {
                    panic!("native capture commit list expected");
                };
                break commits;
            }
            assert!(Instant::now() < list_deadline, "commit list timed out");
            thread::yield_now();
        };
        if interruption.expects_no_commit() {
            assert!(
                commits.is_empty(),
                "failed or interrupted capture must not publish a commit"
            );
            application.close_profile();
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        assert_eq!(
            commits.len(),
            1,
            "one consented native capture expected; settled state={settled_capture_state}"
        );
        let ticket = application
            .submit_storage(StorageRequest::VersioningManifest {
                profile_uuid,
                id: commits[0].id,
            })
            .expect("native capture manifest admitted");
        let manifest_deadline = Instant::now() + Duration::from_secs(10);
        let manifest = loop {
            if let Some(effect) = application
                .poll_storage_effects()
                .into_iter()
                .find(|effect| effect.ticket == ticket)
            {
                let StorageCompletion::VersioningManifest(Ok(Some(manifest))) = effect.completion
                else {
                    panic!("native capture manifest expected");
                };
                break manifest;
            }
            assert!(
                Instant::now() < manifest_deadline,
                "manifest read timed out"
            );
            thread::yield_now();
        };
        if interruption.is_window() {
            assert_eq!(manifest.windows.len(), 1);
            assert_eq!(manifest.windows[0].pages.len(), 2);
            assert_eq!(
                manifest.requested_capture.allow_partial_pages,
                interruption.is_partial_window()
            );
            let any_sidecar = manifest.windows[0]
                .pages
                .iter()
                .any(|page| page.sidecar_object.is_some());
            assert_eq!(
                manifest.helper_bundle_version,
                if any_sidecar {
                    ferric_browser_core::versioning::PAGE_STATE_HELPER_VERSION
                } else {
                    "none"
                }
            );
            let urls = [source_url.as_str(), replacement_url.as_str()];
            let mut digests = Vec::new();
            let mut failed_pages = 0;
            for (page_index, (page, expected_url)) in
                manifest.windows[0].pages.iter().zip(urls).enumerate()
            {
                assert!(matches!(
                    &page.safe_url_or_placeholder,
                    ferric_browser_storage::versioning::ManifestSafeUrl::Replayable(url)
                        if url == expected_url
                ));
                if interruption.is_partial_window() && page.archive_failure.is_some() {
                    assert_eq!(
                        page.archive_failure,
                        Some(match interruption {
                            NativeCaptureScenario::WindowPartialNavigate => {
                                ManifestPageArchiveFailure::DocumentChanged
                            }
                            NativeCaptureScenario::WindowPartialSecondNavigate => {
                                ManifestPageArchiveFailure::DocumentChanged
                            }
                            NativeCaptureScenario::WindowPartialSecondCancel => {
                                ManifestPageArchiveFailure::Cancelled
                            }
                            NativeCaptureScenario::WindowPartialTimeout => {
                                ManifestPageArchiveFailure::Timeout
                            }
                            NativeCaptureScenario::WindowPartialSecondTimeout => {
                                ManifestPageArchiveFailure::Timeout
                            }
                            NativeCaptureScenario::WindowPartialViewLoss => {
                                ManifestPageArchiveFailure::NativeViewLost
                            }
                            NativeCaptureScenario::WindowPartialSecondViewLoss => {
                                ManifestPageArchiveFailure::NativeViewLost
                            }
                            #[cfg(all(feature = "native-archive-fault-test", unix))]
                            NativeCaptureScenario::WindowPartialStorageFull
                            | NativeCaptureScenario::WindowPartialSecondStorageFull => {
                                ManifestPageArchiveFailure::StorageFull
                            }
                            _ => ManifestPageArchiveFailure::Cancelled,
                        })
                    );
                    if interruption.is_second_page_failure() {
                        assert_eq!(page_index, 1, "only the second page should fail");
                    }
                    assert!(page.archive_object.is_none());
                    assert!(page.sidecar_object.is_none());
                    assert!(page.page_capture.is_none());
                    failed_pages += 1;
                    continue;
                }
                if interruption.is_second_page_failure() {
                    assert_eq!(page_index, 0, "the first page should remain archived");
                    assert!(page.sidecar_object.is_some());
                }
                assert!(page.archive_failure.is_none());
                let archive = page.archive_object.as_ref().expect("real Qt page archive");
                assert!(archive.bytes > 0);
                assert_eq!(
                    archive.archive_backend.as_deref(),
                    Some(ferric_browser_storage::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1)
                );
                let capture = page.page_capture.as_ref().expect("indexed page interval");
                assert!(capture.archive_interval.is_some());
                assert!(capture.archive_monotonic_interval.is_some());
                if let Some(sidecar) = &page.sidecar_object {
                    assert!(sidecar.bytes > 0);
                    assert!(capture.sidecar_interval.is_some());
                    assert!(capture.sidecar_monotonic_interval.is_some());
                    assert!(!capture.categories.is_empty());
                } else {
                    assert!(capture.sidecar_interval.is_none());
                    assert!(capture.sidecar_monotonic_interval.is_none());
                    assert!(capture.categories.is_empty());
                }
                digests.push(archive.sha256.as_str());
            }
            if matches!(
                interruption,
                NativeCaptureScenario::WindowStrict
                    | NativeCaptureScenario::RestoreMultiPageGeometry
            ) {
                assert_eq!(failed_pages, 0);
                assert!(
                    manifest.windows[0]
                        .pages
                        .iter()
                        .all(|page| page.sidecar_object.is_some()),
                    "both successful strict pages must retain their indexed collectors"
                );
                assert_ne!(
                    digests[0], digests[1],
                    "distinct tabs need distinct archives"
                );
            } else if matches!(
                interruption,
                NativeCaptureScenario::WindowPartialCloseSavedPage
                    | NativeCaptureScenario::WindowPartialCloseDuringInstall
            ) {
                assert!(
                    interruption_triggered,
                    "the requested native close lifecycle was not exercised"
                );
                assert_eq!(failed_pages, 0);
                assert_eq!(digests.len(), 2);
            } else {
                assert!(
                    interruption_triggered,
                    "partial window never exercised its requested failure"
                );
                assert_eq!(failed_pages, 1, "one failed page must be explicit");
                assert_eq!(
                    digests.len(),
                    1,
                    "one unaffected page must retain its archive"
                );
            }
            application.close_profile();
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        let page = &manifest.windows[0].pages[0];
        let capture = page
            .page_capture
            .as_ref()
            .expect("native capture interval evidence");
        assert_eq!(
            manifest.engine_fingerprint,
            page_capture_engine_fingerprint().unwrap_or_else(|| "unavailable".into()),
            "committed backend provenance must come from the native runtime"
        );
        assert_eq!(
            manifest.helper_bundle_version,
            if capture.sidecar_interval.is_some() {
                ferric_browser_core::versioning::PAGE_STATE_HELPER_VERSION
            } else {
                "none"
            },
            "committed helper provenance must reflect the collector interval"
        );
        assert!(capture.archive_interval.is_some());
        assert!(capture.archive_monotonic_interval.is_some());
        assert_eq!(
            capture.sidecar_interval.is_some(),
            capture.sidecar_monotonic_interval.is_some()
        );
        assert_eq!(
            page.archive_object
                .as_ref()
                .and_then(|archive| archive.archive_backend.as_deref()),
            Some(ferric_browser_storage::versioning::ARCHIVE_BACKEND_QT_WEBENGINE_MHTML_V1)
        );
        if capture.sidecar_interval.is_some() {
            assert!(capture.categories.iter().any(|coverage| {
                coverage.category == ManifestPageCategory::Geometry && coverage.requested > 0
            }));
            assert!(capture.categories.iter().any(|coverage| {
                coverage.category == ManifestPageCategory::Controls && coverage.requested > 0
            }));
        } else {
            assert!(capture.categories.is_empty());
        }
        if expected_path == "/versioning/document" && interruption == NativeCaptureScenario::None {
            assert_eq!(
                capture.mutation_evidence,
                ManifestMutationEvidence::ObservedStable
            );
            assert!(
                capture
                    .categories
                    .iter()
                    .all(|coverage| !coverage.truncated)
            );
            assert_eq!(
                page.fidelity.page,
                ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
            );
        }
        if matches!(
            interruption,
            NativeCaptureScenario::PostForm | NativeCaptureScenario::PostRedirect307
        ) {
            assert!(
                matches!(
                    page.safe_url_or_placeholder,
                    ferric_browser_storage::versioning::ManifestSafeUrl::Placeholder(_)
                ),
                "POST result must not persist a replayable navigation URL"
            );
        }
        assert!(
            page.archive_object.is_some(),
            "real Qt save must be installed"
        );
        if interruption == NativeCaptureScenario::CollectorTimeout {
            assert!(
                page.sidecar_object.is_none(),
                "timed-out collector must not publish"
            );
            assert_eq!(
                page.fidelity.archive,
                ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
            );
            assert_eq!(
                page.fidelity.page,
                ferric_browser_storage::versioning::ManifestCoverage::Unavailable
            );
            assert!(page.page_capture.as_ref().is_some_and(|capture| {
                capture.archive_interval.is_some() && capture.sidecar_interval.is_none()
            }));
            application.close_profile();
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        #[cfg(feature = "native-archive-fault-test")]
        if interruption == NativeCaptureScenario::SidecarWorkerRefusal {
            assert_eq!(
                ferric_browser_storage::versioning::injected_sidecar_install_refusals(),
                1,
                "one refusal must cross the actual sidecar worker"
            );
            assert!(
                page.sidecar_object.is_none(),
                "refused sidecar object must not publish"
            );
            assert_eq!(
                page.fidelity.archive,
                ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
            );
            assert_eq!(
                page.fidelity.page,
                ferric_browser_storage::versioning::ManifestCoverage::Unavailable
            );
            assert!(page.page_capture.as_ref().is_some_and(|capture| {
                capture.archive_interval.is_some() && capture.sidecar_interval.is_some()
            }));
            application.close_profile();
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        #[cfg(all(feature = "native-archive-fault-test", unix))]
        if interruption == NativeCaptureScenario::SidecarFilesystemPermissionDenied {
            assert!(
                page.sidecar_object.is_none(),
                "denied sidecar must not publish"
            );
            assert_eq!(
                page.fidelity.archive,
                ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
            );
            assert_eq!(
                page.fidelity.page,
                ferric_browser_storage::versioning::ManifestCoverage::Unavailable
            );
            assert!(page.page_capture.as_ref().is_some_and(|capture| {
                capture.archive_interval.is_some() && capture.sidecar_interval.is_some()
            }));
            application.close_profile();
            fs::remove_dir_all(base).expect("remove exact disposable profile");
            return;
        }
        assert!(
            page.sidecar_object.is_some(),
            "real Qt collector must be installed"
        );
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert!(page.page_capture.as_ref().is_some_and(|capture| {
            capture.archive_interval.is_some() && capture.sidecar_interval.is_some()
        }));
        if expect_mutation {
            assert_eq!(
                page.page_capture
                    .as_ref()
                    .expect("native capture intervals")
                    .mutation_evidence,
                ManifestMutationEvidence::ObservedChanged
            );
            assert_eq!(
                page.fidelity.page,
                ferric_browser_storage::versioning::ManifestCoverage::Partial
            );
        }
        let logical_page = page
            .logical_page_id
            .parse::<LogicalPageId>()
            .expect("typed committed page");
        let state_root = StorageRoots::resolve(RootSpec::Base(base.clone()))
            .expect("disposable storage roots")
            .state;
        let ticket = application
            .submit_storage(StorageRequest::VersioningSidecarRead {
                profile_uuid,
                state_root,
                id: commits[0].id,
                logical_page,
            })
            .expect("committed native sidecar read admitted");
        let sidecar_deadline = Instant::now() + Duration::from_secs(10);
        let sidecar_bytes = loop {
            if let Some(effect) = application
                .poll_storage_effects()
                .into_iter()
                .find(|effect| effect.ticket == ticket)
            {
                let StorageCompletion::VersioningSidecarRead(Ok(Some(bytes))) = effect.completion
                else {
                    panic!("committed native sidecar read expected");
                };
                break bytes.into_bytes();
            }
            assert!(Instant::now() < sidecar_deadline, "sidecar read timed out");
            thread::yield_now();
        };
        let sidecar = PageStateSidecar::parse_json(&sidecar_bytes)
            .expect("committed native collector sidecar");
        if expected_path == "/versioning/frames" {
            let summary = sidecar.summary();
            assert_eq!(
                summary.frame_count, 2,
                "top document and same-origin child must be retained"
            );
            assert_eq!(
                summary
                    .exclusions_by_reason
                    .get(&ferric_browser_core::versioning::ExclusionReason::CrossOrigin),
                Some(&1),
                "cross-origin child must be recorded as one exclusion"
            );
            let saved: serde_json::Value =
                serde_json::from_slice(&sidecar_bytes).expect("committed frame sidecar JSON");
            let child_url = format!("{fixture_origin}/versioning/frame-child?kind=same");
            let child_route =
                route_fingerprint_for_url(&child_url).expect("same-origin child route fingerprint");
            assert_eq!(saved["frames"][1]["route_fingerprint"], child_route);
        }
        if matches!(
            expected_path,
            "/versioning/editor" | "/versioning/editor-refill"
        ) {
            let summary = sidecar.summary();
            let saved: serde_json::Value = serde_json::from_slice(&sidecar_bytes)
                .expect("committed synthetic editor sidecar JSON");
            if interruption == NativeCaptureScenario::SelectionEditor {
                assert!(
                    summary
                        .requested_categories
                        .contains(&ferric_browser_core::versioning::SidecarCategory::Selection),
                    "selection must be authorized by the exact one-use request"
                );
                assert!(saved["positions"].as_array().is_some_and(|positions| {
                    positions.iter().any(|position| {
                        position["kind"] == "selection-start" && position["value"] == 3
                    }) && positions.iter().any(|position| {
                        position["kind"] == "selection-end" && position["value"] == 12
                    })
                }));
                assert!(saved["coverage"].as_array().is_some_and(|coverage| {
                    coverage.iter().any(|entry| {
                        entry["category"] == "selection"
                            && entry["captured"] == 2
                            && entry["truncated"] == false
                    })
                }));
                assert_eq!(summary.editable_count, 0);
                assert!(
                    !sidecar_bytes
                        .windows(b"synthetic unsaved text".len())
                        .any(|window| window == b"synthetic unsaved text"),
                    "selection-only capture must not retain editor text"
                );
            } else {
                assert!(
                    summary
                        .requested_categories
                        .contains(&ferric_browser_core::versioning::SidecarCategory::Editable)
                );
                let expected = if interruption == NativeCaptureScenario::RestoreEditableRefill {
                    2
                } else {
                    1
                };
                assert_eq!(summary.editable_count, expected, "opted-in draft count");
                assert!(saved["sensitive_editable"].as_array().is_some_and(|items| {
                    items.iter().any(|item| {
                        item["value"]["kind"] == "text"
                            && item["value"]["value"] == "synthetic unsaved text"
                    }) && (interruption != NativeCaptureScenario::RestoreEditableRefill
                        || items.iter().any(|item| {
                            item["value"]["kind"] == "text"
                                && item["value"]["value"] == "synthetic refill draft"
                        }))
                }));
            }
        }
        if expected_path == "/versioning/media" {
            let summary = sidecar.summary();
            assert!(
                summary
                    .requested_categories
                    .contains(&ferric_browser_core::versioning::SidecarCategory::Media)
            );
            let saved: serde_json::Value =
                serde_json::from_slice(&sidecar_bytes).expect("committed local media sidecar JSON");
            assert!(saved["positions"].as_array().is_some_and(|positions| {
                positions
                    .iter()
                    .filter(|position| {
                        position["kind"] == "media-time" && position["paused"] == true
                    })
                    .count()
                    == 1
            }));
            assert!(
                !sidecar_bytes
                    .windows(b"media-silence.wav".len())
                    .any(|window| window == b"media-silence.wav")
            );
            if interruption == NativeCaptureScenario::RestoreMediaPosition {
                assert!(saved["positions"].as_array().is_some_and(|positions| {
                    positions.iter().any(|position| {
                        position["kind"] == "media-time"
                            && position["paused"] == true
                            && position["value"]
                                .as_f64()
                                .is_some_and(|value| (value - 0.4).abs() < 0.1)
                    })
                }));
            }
        }
        let origin = canonical_origin(&source_url).expect("loopback origin");
        let route_fingerprint =
            route_fingerprint_for_url(&source_url).expect("fixture route fingerprint");
        let nonce = operation
            .to_string()
            .parse::<OperationId>()
            .expect("typed disclosure operation")
            .as_uuid()
            .simple()
            .to_string();
        let capture_fingerprint = capture_fingerprint_for_request(&route_fingerprint, &nonce)
            .expect("capture fingerprint");
        assert!(sidecar.qualifies_for(&origin, &route_fingerprint, &capture_fingerprint, &nonce));
        application.close_profile();
        drop(application);
        if matches!(
            interruption,
            NativeCaptureScenario::RestoreGeometry
                | NativeCaptureScenario::RestoreMultiPageGeometry
        ) {
            let saved: serde_json::Value =
                serde_json::from_slice(&sidecar_bytes).expect("committed geometry sidecar JSON");
            assert!(
                saved["positions"].as_array().is_some_and(|positions| {
                    positions.iter().any(|position| {
                        position["kind"] == "element-scroll-y" && position["value"] == 55
                    }) && positions.iter().any(|position| {
                        position["kind"] == "element-scroll-y" && position["value"] == 75
                    })
                }),
                "both nested scroll offsets must be committed"
            );
            if interruption == NativeCaptureScenario::RestoreMultiPageGeometry {
                assert_eq!(manifest.windows.len(), 1);
                assert_eq!(manifest.windows[0].pages.len(), 2);
                assert!(manifest.windows[0].pages[0].sidecar_object.is_some());
            }
            let mut restored_properties = properties.clone();
            restored_properties.insert(
                QString::from("versioningRestoreStartupCommitId"),
                QVariant::from(&QString::from(commits[0].id.to_string().as_str())),
            );
            let mut restore_engine = QQmlApplicationEngine::new();
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .set_initial_properties(&restored_properties);
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .load(&QUrl::from(
                    "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
                ));
            let restore_deadline = Instant::now() + Duration::from_secs(35);
            let mut observed_statuses = Vec::<String>::new();
            let mut last_probe: Option<Instant> = None;
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if let Some(status) = current["status"].as_str()
                    && observed_statuses.last().is_none_or(|last| last != status)
                {
                    observed_statuses.push(status.to_owned());
                }
                if current["view_title"] == "Versioning geometry restored" {
                    break;
                }
                if current["view_url"] == source_url
                    && current["load_state"] == "complete"
                    && last_probe.is_none_or(|last| last.elapsed() >= Duration::from_millis(100))
                {
                    assert!(
                        crate::qobject::ferric_browser_test_probe_restored_page_geometry(
                            restore_engine.as_mut().expect("restore QML engine"),
                            &QString::from(source_url.as_str()),
                        )
                    );
                    last_probe = Some(Instant::now());
                }
                assert!(
                    Instant::now() < restore_deadline,
                    "native restore did not apply committed geometry: statuses={observed_statuses:?}; {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                observed_statuses.iter().any(|status| {
                    status == "Restored structural page"
                        || (interruption == NativeCaptureScenario::RestoreMultiPageGeometry
                            && status.starts_with("Restored 2 structural pages;"))
                }),
                "fresh window never applied the committed structural plan: {observed_statuses:?}"
            );
            if interruption == NativeCaptureScenario::RestoreMultiPageGeometry {
                let restored = state(&mut restore_engine);
                assert_eq!(restored["tab_count"], 2, "both pages must be restored");
                assert_eq!(
                    restored["active_tab_index"], 0,
                    "saved selection must be restored"
                );
                assert_eq!(restored["lazy_restore_pages"][0], false);
                assert_eq!(
                    restored["lazy_restore_pages"][1], true,
                    "inactive safe page must remain lazy until selected"
                );
                let inactive_url = format!("{fixture_origin}/versioning/controlled");
                assert!(crate::qobject::ferric_browser_test_select_page_capture_tab(
                    restore_engine.as_mut().expect("restore QML engine"),
                    1,
                ));
                let activation_deadline = Instant::now() + Duration::from_secs(25);
                loop {
                    app.as_ref().expect("Qt GUI application").process_events();
                    let current = state(&mut restore_engine);
                    if current["active_tab_index"] == 1
                        && current["current_url"] == inactive_url
                        && current["view_url"] == inactive_url
                        && current["load_state"] == "complete"
                        && current["lazy_restore_pages"][1] == false
                    {
                        break;
                    }
                    assert!(
                        Instant::now() < activation_deadline,
                        "lazy sibling did not navigate after selection: {current}"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            }
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                restore_engine.as_mut().expect("restore QML engine"),
            ));
            let shutdown_deadline = Instant::now() + Duration::from_secs(20);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if current["shutdown_approved"] == true || current["root_ready"] == false {
                    break;
                }
                assert!(
                    Instant::now() < shutdown_deadline,
                    "restore shutdown did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(restore_engine);
        }
        if interruption == NativeCaptureScenario::RestoreEditableRefill {
            let mut restored_properties = properties.clone();
            restored_properties.insert(
                QString::from("versioningRestoreStartupCommitId"),
                QVariant::from(&QString::from(commits[0].id.to_string().as_str())),
            );
            restored_properties.insert(
                QString::from("versioningRestoreStartupEditableDrafts"),
                QVariant::from(&true),
            );
            let mut restore_engine = QQmlApplicationEngine::new();
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .set_initial_properties(&restored_properties);
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .load(&QUrl::from(
                    "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
                ));
            let review_deadline = Instant::now() + Duration::from_secs(35);
            let mut blank_probe_at: Option<Instant> = None;
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if current["restore_editable_dialog_visible"] == true
                    && current["restore_preview_refillable_count"] == 1
                    && current["view_title"] == "Versioning refill awaiting review"
                {
                    break;
                }
                if current["restore_editable_dialog_visible"] == true
                    && current["view_url"] == source_url
                    && current["load_state"] == "complete"
                    && blank_probe_at
                        .is_none_or(|last| last.elapsed() >= Duration::from_millis(100))
                {
                    assert!(
                        crate::qobject::ferric_browser_test_probe_blank_restore_refill_draft(
                            restore_engine.as_mut().expect("restore QML engine"),
                            &QString::from(source_url.as_str()),
                        ),
                        "review must hold the exact loaded destination view"
                    );
                    blank_probe_at = Some(Instant::now());
                }
                assert!(
                    Instant::now() < review_deadline,
                    "committed draft review did not show a blank refill target: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                crate::qobject::ferric_browser_test_confirm_restore_editable_review(
                    restore_engine.as_mut().expect("restore QML engine"),
                ),
                "one explicit review must authorize the live refill"
            );
            let refill_deadline = Instant::now() + Duration::from_secs(20);
            let mut refill_probe_at: Option<Instant> = None;
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if current["view_title"] == "Versioning refill restored" {
                    break;
                }
                if current["view_url"] == source_url
                    && current["load_state"] == "complete"
                    && refill_probe_at
                        .is_none_or(|last| last.elapsed() >= Duration::from_millis(100))
                {
                    assert!(
                        crate::qobject::ferric_browser_test_probe_restored_refill_draft(
                            restore_engine.as_mut().expect("restore QML engine"),
                            &QString::from(source_url.as_str()),
                        )
                    );
                    refill_probe_at = Some(Instant::now());
                }
                assert!(
                    Instant::now() < refill_deadline,
                    "explicitly reviewed committed draft was not refilled: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                restore_engine.as_mut().expect("restore QML engine"),
            ));
            let shutdown_deadline = Instant::now() + Duration::from_secs(20);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if current["shutdown_approved"] == true || current["root_ready"] == false {
                    break;
                }
                assert!(
                    Instant::now() < shutdown_deadline,
                    "refill restore shutdown did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(restore_engine);
        }
        if interruption == NativeCaptureScenario::RestoreMediaPosition {
            let mut restored_properties = properties.clone();
            restored_properties.insert(
                QString::from("versioningRestoreStartupCommitId"),
                QVariant::from(&QString::from(commits[0].id.to_string().as_str())),
            );
            restored_properties.insert(
                QString::from("versioningRestoreStartupMediaPositions"),
                QVariant::from(&true),
            );
            let mut restore_engine = QQmlApplicationEngine::new();
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .set_initial_properties(&restored_properties);
            restore_engine
                .as_mut()
                .expect("restore QML engine")
                .load(&QUrl::from(
                    "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
                ));
            let restore_deadline = Instant::now() + Duration::from_secs(35);
            let mut last_probe: Option<Instant> = None;
            let mut observed_statuses = Vec::<String>::new();
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if let Some(status) = current["status"].as_str()
                    && observed_statuses.last().is_none_or(|last| last != status)
                {
                    observed_statuses.push(status.to_owned());
                }
                if current["view_title"] == "Versioning media restored" {
                    break;
                }
                if current["view_url"] == source_url
                    && current["load_state"] == "complete"
                    && last_probe.is_none_or(|last| last.elapsed() >= Duration::from_millis(100))
                {
                    assert!(
                        crate::qobject::ferric_browser_test_probe_restored_media_position(
                            restore_engine.as_mut().expect("restore QML engine"),
                            &QString::from(source_url.as_str()),
                        )
                    );
                    last_probe = Some(Instant::now());
                }
                assert!(
                    Instant::now() < restore_deadline,
                    "committed paused media position did not restore: statuses={observed_statuses:?}; {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                restore_engine.as_mut().expect("restore QML engine"),
            ));
            let shutdown_deadline = Instant::now() + Duration::from_secs(20);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut restore_engine);
                if current["shutdown_approved"] == true || current["root_ready"] == false {
                    break;
                }
                assert!(
                    Instant::now() < shutdown_deadline,
                    "media restore shutdown did not settle: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(restore_engine);
        }
        drop(app);
        fs::remove_dir_all(base).expect("remove exact disposable profile");
    }

    #[test]
    #[ignore = "requires verified native Qt archive output and a Qt GUI runtime"]
    fn native_qt_archive_closes_through_real_browser_window_and_storage() {
        archive_window_cleanup_journey(ArchiveWindowJourney::Shutdown);
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires verified native Qt archive output and a Qt GUI runtime"]
    fn native_qt_archive_unsafe_cleanup_retains_file_through_shutdown() {
        archive_window_cleanup_journey(ArchiveWindowJourney::UnsafeCleanupRetention);
    }

    #[test]
    #[ignore = "requires verified native Qt archive output and a Qt GUI runtime"]
    fn native_qt_archive_close_then_switches_to_fresh_profile() {
        archive_window_cleanup_journey(ArchiveWindowJourney::PostCloseProfileSwitch);
    }

    #[cfg(feature = "native-archive-fault-test")]
    #[test]
    #[ignore = "requires verified native Qt archive output and a Qt GUI runtime"]
    fn native_qt_archive_cleanup_retries_after_one_worker_refusal() {
        archive_window_cleanup_journey(ArchiveWindowJourney::InjectedWorkerRefusal);
    }

    enum ArchiveWindowJourney {
        Shutdown,
        UnsafeCleanupRetention,
        PostCloseProfileSwitch,
        #[cfg(feature = "native-archive-fault-test")]
        InjectedWorkerRefusal,
    }

    fn archive_window_cleanup_journey(journey: ArchiveWindowJourney) {
        use cxx_qt_lib::{
            QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl,
            QVariant,
        };

        let fixture = PathBuf::from(
            std::env::var_os("FERRIC_NATIVE_ARCHIVE_FIXTURE")
                .expect("xtask must provide the verified native archive"),
        );
        let (mut seeded_ui, base, profile_uuid, commit, _) =
            publish_native_archive_fixture(&fixture);
        assert!(stored_manifest(&mut seeded_ui, profile_uuid, commit).is_some());
        seeded_ui
            .state
            .as_mut()
            .expect("seed application")
            .close_profile();
        drop(seeded_ui);

        crate::register_internal_scheme();
        cxx_qt::init_crate!(ferric_browser_engine_qt);
        cxx_qt::init_qml_module!("io.github.ferricbrowser");
        let app = QGuiApplication::new();
        assert!(app.as_ref().is_some(), "Qt GUI application");
        let mut engine = QQmlApplicationEngine::new();
        let config = serde_json::to_string(&Config::default()).expect("default config");
        let contexts = serde_json::to_string(&ContextsConfig::default()).expect("contexts");
        let overrides =
            serde_json::to_string(&RuntimeOverrides::default()).expect("default overrides");
        let mut properties: QMap<QMapPair_QString_QVariant> = QMap::default();
        for (name, value) in [
            ("startupConfigJson", config.as_str()),
            ("startupConfigBaseJson", config.as_str()),
            ("startupContextsJson", contexts.as_str()),
            ("startupCliOverridesJson", overrides.as_str()),
            ("startupProfileOverridesJson", overrides.as_str()),
            (
                "storageBasePath",
                base.to_str().expect("UTF-8 disposable path"),
            ),
        ] {
            properties.insert(QString::from(name), QVariant::from(&QString::from(value)));
        }
        properties.insert(
            QString::from("instanceRequestOwner"),
            QVariant::from(&false),
        );
        engine
            .as_mut()
            .expect("QML engine")
            .set_initial_properties(&properties);
        engine.as_mut().expect("QML engine").load(&QUrl::from(
            "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
        ));

        let state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
            let json = crate::qobject::ferric_browser_test_archive_window_state(
                engine.as_mut().expect("QML engine"),
            );
            serde_json::from_str::<serde_json::Value>(&json.to_string())
                .expect("bounded archive window state")
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        let accepted = loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            assert!(
                current["root_ready"] == true,
                "production QML root failed: {current}"
            );
            if crate::qobject::ferric_browser_test_request_archive_view(
                engine.as_mut().expect("QML engine"),
                &QString::from(commit.to_string()),
            ) {
                break current;
            }
            assert!(
                Instant::now() < deadline,
                "archive request unavailable: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(accepted["ui_ready"].as_bool().unwrap_or(false));

        let archive_url = loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            if let Some(url) = current["archive_url"].as_str()
                && !url.is_empty()
                && current["archive_host_active"] == true
                && current["archive_host_item_ready"] == true
                && current["archive_presented_count"].as_i64().unwrap_or(0) > 0
            {
                break url.to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "archive host not presented: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        };
        let archive_file = PathBuf::from(
            QUrl::from(archive_url.as_str())
                .to_local_file()
                .expect("private file URL")
                .to_string(),
        );
        assert!(
            archive_file.is_file(),
            "materialized viewer file must exist"
        );
        assert!(
            crate::qobject::ferric_browser_test_attempt_archive_profile_replacement(
                engine.as_mut().expect("QML engine"),
            )
        );
        let refused_switch = state(&mut engine);
        assert_eq!(
            refused_switch["status"],
            "Close the isolated stored archive before changing profiles"
        );
        assert_eq!(refused_switch["archive_url"], archive_url);
        assert_eq!(refused_switch["archive_host_active"], true);
        assert!(
            archive_file.is_file(),
            "profile refusal must retain viewer file"
        );
        let unsafe_cleanup = matches!(journey, ArchiveWindowJourney::UnsafeCleanupRetention);
        let switch_after_cleanup = matches!(journey, ArchiveWindowJourney::PostCloseProfileSwitch);
        #[cfg(feature = "native-archive-fault-test")]
        if matches!(journey, ArchiveWindowJourney::InjectedWorkerRefusal) {
            ferric_browser_storage::versioning::arm_one_archive_cleanup_refusal();
        }
        #[cfg(unix)]
        if unsafe_cleanup {
            use std::os::unix::fs::PermissionsExt;

            // The real storage worker must refuse an unsafe archive leaf.
            // Keep it unsafe through shutdown: racing an immediate retry
            // against a test-side chmod cannot prove deterministic recovery.
            fs::set_permissions(&archive_file, fs::Permissions::from_mode(0o644))
                .expect("make exact disposable viewer leaf unsafe");
        }
        #[cfg(not(unix))]
        let _ = unsafe_cleanup;
        if switch_after_cleanup {
            assert!(crate::qobject::ferric_browser_test_close_archive_view(
                engine.as_mut().expect("QML engine"),
            ));
        } else {
            assert!(crate::qobject::ferric_browser_test_begin_window_shutdown(
                engine.as_mut().expect("QML engine"),
            ));
        }

        let mut blank_acknowledged = false;
        let closed = loop {
            app.as_ref().expect("Qt GUI application").process_events();
            let current = state(&mut engine);
            blank_acknowledged |= current["archive_blanked_count"].as_i64().unwrap_or(0) > 0;
            if current["archive_url"]
                .as_str()
                .is_some_and(|url| !url.is_empty())
            {
                assert!(
                    archive_file.exists(),
                    "viewer file was removed while still presented: {current}"
                );
            }
            let refused_shutdown_settled = unsafe_cleanup
                && current["shutdown_approved"] == true
                && current["archive_shutdown_ready"] == true;
            let removed_and_settled = !archive_file.exists()
                && (switch_after_cleanup && current["archive_shutdown_ready"] == true
                    || current["shutdown_approved"] == true
                    || current["root_ready"] == false);
            if refused_shutdown_settled || removed_and_settled {
                break current;
            }
            assert!(
                Instant::now() < deadline,
                "archive shutdown did not finish: {current}"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            blank_acknowledged,
            "archive close requires blank acknowledgement"
        );
        assert!(closed["archive_url"].is_null() || closed["archive_url"] == "");
        #[cfg(feature = "native-archive-fault-test")]
        if matches!(journey, ArchiveWindowJourney::InjectedWorkerRefusal) {
            assert_eq!(
                ferric_browser_storage::versioning::injected_archive_cleanup_refusals(),
                1,
                "one refusal must cross the actual storage worker"
            );
            assert!(
                !archive_file.exists(),
                "bounded UI retry must remove exact file"
            );
        }
        #[cfg(unix)]
        if unsafe_cleanup {
            use std::os::unix::fs::PermissionsExt;

            assert!(
                archive_file.is_file(),
                "unsafe viewer file must be retained"
            );
            assert_eq!(
                fs::metadata(&archive_file)
                    .expect("retained viewer metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o644,
            );
            fs::set_permissions(&archive_file, fs::Permissions::from_mode(0o600))
                .expect("restore disposable viewer mode after shutdown approval");
        }
        if switch_after_cleanup {
            assert_eq!(closed["archive_host_active"], false);
            assert_eq!(closed["shutdown_in_progress"], false);
            assert!(
                crate::qobject::ferric_browser_test_attempt_archive_profile_replacement(
                    engine.as_mut().expect("QML engine"),
                )
            );
            let requested = state(&mut engine);
            assert_eq!(requested["status"], "Profile bootstrap requested");
            let switched = loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let current = state(&mut engine);
                if current["profile_bootstrap_pending"] == false
                    && current["status"].as_str().is_some_and(|status| {
                        status == "Ready" || status.starts_with("Profile ready")
                    })
                {
                    break current;
                }
                assert!(
                    Instant::now() < deadline,
                    "post-close profile switch failed: {current}"
                );
                thread::sleep(Duration::from_millis(10));
            };
            assert_eq!(switched["archive_host_active"], false);
            assert!(!crate::qobject::ferric_browser_test_request_archive_view(
                engine.as_mut().expect("QML engine"),
                &QString::from(commit.to_string()),
            ));
        }
        drop(engine);
        drop(app);
        if unsafe_cleanup {
            assert_retained_archive_is_quarantined_on_profile_reopen(
                &base,
                profile_uuid,
                &archive_file,
                &fixture,
            );
        }
        fs::remove_dir_all(&base).expect("remove exact disposable profile");
    }

    fn assert_retained_archive_is_quarantined_on_profile_reopen(
        base: &Path,
        profile_uuid: Uuid,
        viewer_file: &Path,
        verified_fixture: &Path,
    ) {
        let operation_name = viewer_file
            .parent()
            .and_then(Path::file_name)
            .expect("exact viewer operation directory")
            .to_string_lossy()
            .into_owned();
        let profile_root = viewer_file
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("viewer profile root");
        assert_eq!(
            profile_root.file_name().expect("profile UUID directory"),
            profile_uuid.to_string().as_str()
        );

        let (mut reopened, _, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("restart normal application");
        reopened
            .request_profile_open(ProfileOpenRequest {
                name: "default".into(),
                label: "Default".into(),
                privacy: PrivacyKind::Normal,
                storage: ProfileStorage::Durable(RootSpec::Base(base.to_owned())),
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: RuntimeOverrides::default(),
                config_path: None,
                contexts: ContextsConfig::default(),
                context_configuration_issue: false,
            })
            .expect("same disposable profile request");
        let deadline = Instant::now() + Duration::from_secs(10);
        let profile = loop {
            if let Some(result) = reopened.poll_profile_open() {
                break result.expect("reopened durable profile").profile;
            }
            assert!(Instant::now() < deadline, "profile reopen timed out");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(profile.id, Some(profile_uuid));

        while viewer_file.exists() {
            for effect in reopened.poll_storage_effects() {
                assert!(
                    !matches!(
                        effect.completion,
                        StorageCompletion::VersioningRecover(Err(_))
                    ),
                    "viewer recovery failed: {effect:?}"
                );
            }
            assert!(
                Instant::now() < deadline,
                "retained viewer was not quarantined"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let quarantined_viewers = fs::read_dir(profile_root.join("quarantine"))
            .expect("quarantine batches")
            .map(|entry| {
                entry
                    .expect("quarantine batch")
                    .path()
                    .join(format!("view-{operation_name}"))
                    .join("capture.mhtml")
            })
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        assert_eq!(quarantined_viewers.len(), 1, "one exact retained viewer");
        assert_eq!(
            fs::read(&quarantined_viewers[0]).expect("quarantined viewer bytes"),
            fs::read(verified_fixture).expect("verified Qt archive bytes")
        );
        reopened.close_profile();
    }

    #[test]
    fn successful_collector_with_failed_archive_publishes_nothing() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        assert!(ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge)));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "102", &path, true, true, true
        ));
        assert!(!ui.finish_native_page_capture_download(
            operation,
            "102",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "interrupted",
            0,
            "not-observed"
        ));
        assert!(ui.native_stash_bridge.page_capture_authorization.is_none());
        assert!(stored_manifest(&mut ui, profile_uuid, commit).is_none());
        finish(&mut ui, base);
    }

    #[test]
    fn failed_sidecar_install_keeps_successful_archive_with_unavailable_page_fidelity() {
        let (mut ui, base, profile_uuid, operation, commit) = prepared_capture();
        let tab = ui.tab.expect("captured tab");
        let challenge = ui
            .native_page_capture_sidecar_request(operation, tab)
            .expect("collector challenge");
        assert!(ui.complete_native_page_capture_sidecar(operation, synthetic_sidecar(challenge)));
        let path = ui
            .claim_native_page_capture_save(operation, tab, SOURCE_URL)
            .expect("exact native save path");
        let bytes = b"synthetic bounded MHTML archive";
        fs::write(&path, bytes).expect("native save fixture");
        assert!(ui.note_native_page_capture_download_requested(
            operation, "103", &path, true, true, true
        ));
        assert!(ui.finish_native_page_capture_download(
            operation,
            "103",
            &path,
            true,
            true,
            true,
            SOURCE_URL,
            "completed",
            i64::try_from(bytes.len()).expect("bounded archive size"),
            "not-observed"
        ));
        let (_, archive_ticket) = ui
            .native_stash_bridge
            .page_capture_install_ticket
            .expect("archive install ticket");
        let StorageCompletion::VersioningObjectInstall(Ok(archive)) =
            wait_for(&mut ui, archive_ticket)
        else {
            panic!("archive installation expected");
        };
        assert!(ui.complete_native_page_capture_install(archive_ticket, Ok(archive)));
        let (_, sidecar_ticket) = ui
            .native_stash_bridge
            .page_capture_sidecar_install_ticket
            .expect("sidecar install ticket");
        assert!(matches!(
            wait_for(&mut ui, sidecar_ticket),
            StorageCompletion::VersioningObjectInstall(Ok(_))
        ));
        assert!(ui.complete_native_page_capture_sidecar_install(
            sidecar_ticket,
            Err(StorageOperationError {
                operation: StorageOperation::VersioningObjectInstall,
                kind: ferric_browser_storage::StorageFailureKind::Other,
                message: "injected sidecar callback failure".into(),
            })
        ));
        let (_, publish_ticket) = ui
            .native_stash_bridge
            .page_capture_publish_ticket
            .expect("archive-only publication ticket");
        assert!(matches!(
            wait_for(&mut ui, publish_ticket),
            StorageCompletion::VersioningObjectPublish(Ok(
                ferric_browser_storage::VersioningObjectPublishReceipt {
                    staging_cleanup_complete: true,
                },
            ))
        ));
        assert!(ui.complete_native_page_capture_publish(
            publish_ticket,
            Ok(ferric_browser_storage::VersioningObjectPublishReceipt {
                staging_cleanup_complete: true,
            }),
        ));
        let manifest =
            stored_manifest(&mut ui, profile_uuid, commit).expect("durable archive-only manifest");
        let page = &manifest.windows[0].pages[0];
        assert!(page.archive_object.is_some());
        assert!(page.sidecar_object.is_none());
        assert_eq!(
            page.fidelity.archive,
            ferric_browser_storage::versioning::ManifestCoverage::CompleteForRequest
        );
        assert_eq!(
            page.fidelity.page,
            ferric_browser_storage::versioning::ManifestCoverage::Unavailable
        );
        finish(&mut ui, base);
    }
}
