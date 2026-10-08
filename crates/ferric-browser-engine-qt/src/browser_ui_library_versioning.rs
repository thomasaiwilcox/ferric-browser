//! Routes versioned-storage completions and application events to the panel.

use super::{
    Pin, QString, StorageCompletion, StorageEffect, display_url, path_to_file_url, qobject,
};
use cxx_qt::CxxQtType;
use ferric_browser_application::versioning::{
    CleanBranchForkEvent, CommitEvent, CommitReadEvent, PolicyRestoreEvent, RestoreNavigation,
    RestorePlanEvent, RestoreSidecarEvent,
};

fn commit_info_panel_data(
    summary: &ferric_browser_storage::versioning::CommitInfo,
) -> serde_json::Value {
    serde_json::json!({
        "id": summary.summary.id.to_string(),
        "createdAtMs": summary.summary.created_at_ms,
        "pinned": summary.summary.pinned,
        "objectLinks": summary.summary.object_links,
        "scope": match summary.scope {
            ferric_browser_storage::versioning::ManifestScope::Tab => "tab",
            ferric_browser_storage::versioning::ManifestScope::Window => "window",
            ferric_browser_storage::versioning::ManifestScope::Context => "context",
        },
        "windows": summary.window_count,
        "pages": summary.page_count,
        "replayablePages": summary.replayable_pages,
        "unavailableUrls": summary.unavailable_urls.iter().copied().sum::<u16>(),
        "archiveObjects": summary.archive_objects,
        "partialPagesAllowed": summary.partial_pages_allowed,
        "archiveFailures": summary.archive_failures.iter().map(|failure| serde_json::json!({
            "logicalPageId": failure.logical_page_id,
            "reason": failure.reason,
        })).collect::<Vec<_>>(),
        "sidecarObjects": summary.sidecar_objects,
        "missingObjects": summary.missing_objects,
        "quarantinedObjects": summary.quarantined_objects,
        "warningCount": summary.warning_count,
        "coverage": {
            "archive": {
                "unknown": summary.archive_coverage.unknown,
                "notRequested": summary.archive_coverage.not_requested,
                "unavailable": summary.archive_coverage.unavailable,
                "partial": summary.archive_coverage.partial,
                "completeForRequest": summary.archive_coverage.complete_for_request,
            },
            "page": {
                "unknown": summary.page_coverage.unknown,
                "notRequested": summary.page_coverage.not_requested,
                "unavailable": summary.page_coverage.unavailable,
                "partial": summary.page_coverage.partial,
                "completeForRequest": summary.page_coverage.complete_for_request,
            },
            "cookies": {
                "unknown": summary.cookie_coverage.unknown,
                "notRequested": summary.cookie_coverage.not_requested,
                "unavailable": summary.cookie_coverage.unavailable,
                "partial": summary.cookie_coverage.partial,
                "completeForRequest": summary.cookie_coverage.complete_for_request,
            },
        },
        "maxManifestBytes": summary.max_manifest_bytes,
        "maxSingleObjectBytes": summary.max_single_object_bytes,
        "maxCommitObjectBytes": summary.max_commit_object_bytes,
    })
}

fn commit_event_status(event: &CommitEvent) -> &'static str {
    match event {
        CommitEvent::Durable { .. } => "Versioned structural commit saved",
        CommitEvent::Failed { .. } => {
            "Versioned capture outcome unconfirmed; inspect the commit list"
        }
    }
}

fn archive_source_display(source_url: Option<&str>) -> String {
    source_url.map(display_url).unwrap_or_default()
}

impl qobject::BrowserUi {
    pub(super) fn route_versioning_storage_completion(
        mut self: Pin<&mut Self>,
        effect: StorageEffect,
    ) -> Option<StorageEffect> {
        let StorageEffect { ticket, completion } = effect;
        match completion {
            StorageCompletion::VersioningCaptureStage(result) => {
                let routed = {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.complete_native_page_capture_stage(ticket, result)
                };
                self.as_mut().set_status_text(QString::from(if routed {
                    "Page archive destination prepared"
                } else {
                    "Page archive destination could not be prepared"
                }));
                None
            }
            StorageCompletion::VersioningObjectInstall(result) => {
                let routed = {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.complete_native_page_capture_object_install(ticket, result)
                };
                self.as_mut().set_status_text(QString::from(if routed {
                    "Page archive validated; finalizing capture metadata"
                } else {
                    "Page archive validation failed; no capture was published"
                }));
                None
            }
            StorageCompletion::VersioningObjectPublish(result) => {
                let staging_cleanup_pending =
                    matches!(&result, Ok(receipt) if !receipt.staging_cleanup_complete);
                let routed = {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.complete_native_page_capture_publish(ticket, result)
                };
                self.as_mut().set_status_text(QString::from(if routed {
                    if staging_cleanup_pending {
                        "Page capture saved; staging cleanup needs recovery"
                    } else {
                        "Page capture saved; refresh the versioning list to inspect it"
                    }
                } else {
                    "Page archive publication may have completed; refresh the versioning list before retrying"
                }));
                None
            }
            StorageCompletion::VersioningArchiveView(result) => {
                let completion_ticket = ticket.value();
                let mut archive_url = None;
                let mut archive_metadata = None;
                let mut status = "Stored archive view could not be prepared";
                {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    let owns_request = this.pending_archive_view_request == Some(completion_ticket);
                    if owns_request {
                        this.pending_archive_view_request = None;
                    }
                    match result {
                        Ok(file) if owns_request => match path_to_file_url(file.path()) {
                            Ok(url) => {
                                let metadata = this.window.and_then(|window| {
                                    this.state.as_mut().and_then(|application| {
                                        application.versioned_archive_view_metadata(
                                            window,
                                            file.operation(),
                                        )
                                    })
                                });
                                if let Some(metadata) = metadata {
                                    this.active_archive_view_operation = Some(file.operation());
                                    archive_url = Some(url);
                                    archive_metadata = Some(metadata);
                                    status = "Opening isolated stored archive view";
                                } else {
                                    if let (Some(window), Some(application)) =
                                        (this.window, this.state.as_mut())
                                        && let Ok(ticket) = application
                                            .close_versioned_archive_view(window, file.operation())
                                    {
                                        this.pending_archive_view_cleanup = Some(ticket.value());
                                        this.active_archive_view_operation = Some(file.operation());
                                    }
                                    status = "Stored archive provenance is unavailable";
                                }
                            }
                            Err(_) => {
                                if let (Some(window), Some(application)) =
                                    (this.window, this.state.as_mut())
                                    && let Ok(ticket) = application
                                        .close_versioned_archive_view(window, file.operation())
                                {
                                    this.pending_archive_view_cleanup = Some(ticket.value());
                                    this.active_archive_view_operation = Some(file.operation());
                                }
                                status = "Stored archive path could not be opened safely";
                            }
                        },
                        Ok(file) => {
                            // The application accepted this exact file but
                            // its presentation request is stale. Retire only
                            // that operation's private staging file.
                            if let (Some(window), Some(application)) =
                                (this.window, this.state.as_mut())
                                && let Ok(ticket) = application
                                    .close_versioned_archive_view(window, file.operation())
                            {
                                this.pending_archive_view_cleanup = Some(ticket.value());
                                this.active_archive_view_operation = Some(file.operation());
                            }
                            status = "Stale archive view result was discarded";
                        }
                        Err(_) => {}
                    }
                }
                if let Some(metadata) = archive_metadata {
                    self.as_mut()
                        .set_versioning_archive_source_url(QString::from(archive_source_display(
                            metadata.source_url.as_deref(),
                        )));
                    self.as_mut()
                        .set_versioning_archive_captured_at_ms(QString::from(
                            metadata.captured_at_ms.to_string(),
                        ));
                    self.as_mut()
                        .set_versioning_archive_fidelity_label(QString::from(
                            metadata.fidelity_label,
                        ));
                }
                if let Some(url) = archive_url {
                    if let Some(operation) = self.as_ref().rust().active_archive_view_operation {
                        self.as_mut()
                            .set_versioning_archive_view_operation(QString::from(
                                operation.to_string(),
                            ));
                    }
                    self.as_mut()
                        .set_versioning_archive_view_url(QString::from(url));
                }
                self.as_mut().set_status_text(QString::from(status));
                None
            }
            StorageCompletion::VersioningArchiveViewCleanup(result) => {
                let cleanup_ticket = ticket.value();
                let cleanup_succeeded = result.is_ok();
                let status = {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    if this.pending_archive_view_cleanup == Some(cleanup_ticket) {
                        this.pending_archive_view_cleanup = None;
                        if cleanup_succeeded {
                            this.active_archive_view_operation = None;
                            this.archive_view_cleanup_retries = 0;
                            "Stored archive view closed and temporary file removed"
                        } else if this.archive_view_cleanup_retries < 2 {
                            let retry_ticket = match (
                                this.window,
                                this.active_archive_view_operation,
                                this.state.as_mut(),
                            ) {
                                (Some(window), Some(operation), Some(application)) => application
                                    .close_versioned_archive_view(window, operation)
                                    .ok(),
                                _ => None,
                            };
                            if let Some(ticket) = retry_ticket {
                                this.pending_archive_view_cleanup = Some(ticket.value());
                                this.archive_view_cleanup_retries += 1;
                                "Stored archive view closed; retrying temporary-file cleanup"
                            } else {
                                "Stored archive view closed, but its temporary file could not be removed"
                            }
                        } else {
                            "Stored archive view closed, but its temporary file could not be removed"
                        }
                    } else {
                        "Stale archive cleanup result was discarded"
                    }
                };
                self.as_mut().set_status_text(QString::from(status));
                None
            }
            StorageCompletion::VersioningBegin(_)
            | StorageCompletion::VersioningAbort(_)
            | StorageCompletion::VersioningStructuralPublish(_)
            | StorageCompletion::VersioningRecover(_)
            | StorageCompletion::VersioningList(_)
            | StorageCompletion::VersioningInfo(_)
            | StorageCompletion::VersioningDeletePreview(_)
            | StorageCompletion::VersioningDelete(_)
            | StorageCompletion::VersioningCollect(_)
            | StorageCompletion::VersioningManifest(_)
            | StorageCompletion::VersioningSidecarRead(_) => {
                // Only application-correlated commit tickets are consumed
                // as versioning results. Raw metadata never means visible
                // commit success here.
                self.as_mut().set_status_text(QString::from(
                    "Versioned capture completion has no presentation owner",
                ));
                None
            }
            completion => Some(StorageEffect { ticket, completion }),
        }
    }

    pub(super) fn poll_versioning_events(mut self: Pin<&mut Self>) -> bool {
        let (
            commit_events,
            read_events,
            restore_events,
            sidecar_events,
            policy_events,
            branch_events,
        ) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.state.as_mut().map_or_else(
                || {
                    (
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                },
                |application| {
                    (
                        application.poll_commit_events(),
                        application.poll_commit_read_events(),
                        application.poll_restore_plan_events(),
                        application.poll_restore_sidecar_events(),
                        application.poll_policy_restore_events(),
                        application.poll_clean_branch_fork_events(),
                    )
                },
            )
        };
        let mut consumed = false;
        for event in commit_events {
            consumed = true;
            let status = commit_event_status(&event);
            self.as_mut().set_status_text(QString::from(status));
        }
        for event in branch_events {
            consumed = true;
            match event {
                CleanBranchForkEvent::Durable {
                    branch, navigation, ..
                } => {
                    let payload = match navigation {
                        RestoreNavigation::SafeGet(url) => serde_json::json!({
                            "branch": branch.to_string(),
                            "navigation": "safe-get",
                            "url": url,
                        }),
                        RestoreNavigation::Placeholder(_) => serde_json::json!({
                            "branch": branch.to_string(),
                            "navigation": "placeholder",
                        }),
                    };
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_actions
                        .push_back(format!("versioning-clean-branch-ready\t{}", payload));
                    self.as_mut().set_status_text(QString::from(
                        "Clean branch fork saved; preparing isolated child",
                    ));
                }
                CleanBranchForkEvent::WindowDurable { branch, plan, .. } => {
                    let pages: Vec<_> = plan
                        .pages
                        .into_iter()
                        .map(|page| match page.navigation {
                            RestoreNavigation::SafeGet(url) => serde_json::json!({
                                "navigation": "safe-get",
                                "url": url,
                            }),
                            RestoreNavigation::Placeholder(_) => serde_json::json!({
                                "navigation": "placeholder",
                            }),
                        })
                        .collect();
                    let payload = serde_json::json!({
                        "branch": branch.to_string(),
                        "scope": "window",
                        "selected_index": plan.selected_index,
                        "pages": pages,
                    });
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_actions
                        .push_back(format!("versioning-clean-branch-ready\t{}", payload));
                    self.as_mut().set_status_text(QString::from(
                        "Window branch fork saved; preparing isolated tab-capable child",
                    ));
                }
                CleanBranchForkEvent::Failed { .. } => {
                    self.as_mut().set_status_text(QString::from(
                        "Clean branch preparation failed; source window remains active",
                    ));
                }
            }
        }
        for event in read_events {
            consumed = true;
            let request_id = match &event {
                CommitReadEvent::Listed { request_id, .. }
                | CommitReadEvent::Info { request_id, .. }
                | CommitReadEvent::DeletePreview { request_id, .. }
                | CommitReadEvent::Deleted { request_id, .. }
                | CommitReadEvent::Failed { request_id } => *request_id,
                CommitReadEvent::Diff(
                    ferric_browser_application::versioning::CommitDiffEvent::Compared {
                        request_id,
                        ..
                    }
                    | ferric_browser_application::versioning::CommitDiffEvent::NotFound {
                        request_id,
                    }
                    | ferric_browser_application::versioning::CommitDiffEvent::Failed { request_id },
                ) => *request_id,
            };
            if self.as_ref().rust().latest_versioning_read_request != Some(request_id) {
                continue;
            }
            let (kind, data, status) = match event {
                CommitReadEvent::Listed {
                    commits,
                    offset,
                    limit,
                    has_next,
                    ..
                } => {
                    let data = serde_json::json!({
                        "commits": commits.iter().map(|summary| serde_json::json!({
                            "id": summary.id.to_string(),
                            "createdAtMs": summary.created_at_ms,
                            "pinned": summary.pinned,
                            "objectLinks": summary.object_links,
                        })).collect::<Vec<_>>(),
                        "returned": commits.len(),
                        "offset": offset,
                        "limit": limit,
                        "hasNext": has_next,
                    });
                    (
                        "list",
                        data.to_string(),
                        format!("Versioned commits: {}", commits.len()),
                    )
                }
                CommitReadEvent::Info {
                    commit: Some(summary),
                    ..
                } => {
                    let data = commit_info_panel_data(&summary);
                    (
                    "info",
                    data.to_string(),
                    "Versioned commit details".to_owned(),
                    )
                }
                CommitReadEvent::Info { commit: None, .. } => (
                    "error",
                    serde_json::json!({ "message": "Commit was not found in this profile." })
                        .to_string(),
                    "Versioned commit was not found in this profile".to_owned(),
                ),
                CommitReadEvent::Failed { .. } => (
                    "error",
                    serde_json::json!({ "message": "Commit metadata could not be read." })
                        .to_string(),
                    "Versioned commit metadata could not be read".to_owned(),
                ),
                CommitReadEvent::DeletePreview {
                    preview: Some(preview),
                    request_id,
                } => {
                    let data = serde_json::json!({
                        "requestId": request_id.to_string(),
                        "id": preview.id.to_string(),
                        "pinned": preview.pinned,
                        "childCommits": preview.child_commits,
                        "namedReferences": preview.named_references,
                        "activeOperations": preview.active_operations,
                        "objectLinks": preview.object_links,
                        "sharedObjects": preview.shared_objects,
                        "reclaimableObjects": preview.reclaimable_objects,
                        "reclaimableBytes": preview.reclaimable_bytes,
                        "canDelete": preview.can_delete,
                    });
                    (
                        "delete-preview",
                        data.to_string(),
                        "Commit deletion preview ready".to_owned(),
                    )
                }
                CommitReadEvent::DeletePreview {
                    preview: None,
                    ..
                } => (
                    "error",
                    serde_json::json!({ "message": "Commit was not found in this profile." })
                        .to_string(),
                    "Versioned commit was not found in this profile".to_owned(),
                ),
                CommitReadEvent::Deleted { receipt, .. } => {
                    let data = serde_json::json!({
                        "id": receipt.id.to_string(),
                        "reclaimableObjects": receipt.reclaimable_objects,
                        "reclaimableBytes": receipt.reclaimable_bytes,
                        "cleanupPending": receipt.cleanup_pending,
                        "quarantinedStagingOperations": receipt.quarantined_staging_operations,
                        "quarantinedOrphanObjects": receipt.quarantined_orphan_objects,
                        "quarantinedCorruptObjects": receipt.quarantined_corrupt_objects,
                        "missingObjects": receipt.missing_objects,
                        "forensicErasure": false,
                    });
                    let status = if receipt.cleanup_pending {
                        "Commit removed from history; safe object cleanup is pending"
                    } else {
                        "Commit removed from versioned history"
                    };
                    ("delete-complete", data.to_string(), status.to_owned())
                }
                CommitReadEvent::Diff(event) => match event {
                    ferric_browser_application::versioning::CommitDiffEvent::Compared {
                        offset,
                        left,
                        right,
                        kind,
                        page,
                        unsupported_categories,
                        ..
                    } => {
                        let data = serde_json::json!({
                            "left": left.to_string(),
                            "right": right.to_string(),
                            "offset": offset,
                            "kind": match kind {
                                ferric_browser_core::DiffKind::Browser => "browser",
                                ferric_browser_core::DiffKind::Page => "page",
                                ferric_browser_core::DiffKind::Cookies => "cookies",
                            },
                            "page": page,
                            "unsupportedCategories": unsupported_categories,
                        });
                        (
                            "diff",
                            data.to_string(),
                            "Versioned structural diff ready".to_owned(),
                        )
                    }
                    ferric_browser_application::versioning::CommitDiffEvent::NotFound { .. } => (
                        "error",
                        serde_json::json!({ "message": "One or both commits were not found in this profile." }).to_string(),
                        "One or both commits were not found in this profile".to_owned(),
                    ),
                    ferric_browser_application::versioning::CommitDiffEvent::Failed { .. } => (
                        "error",
                        serde_json::json!({ "message": "The structural diff could not be computed." }).to_string(),
                        "The structural diff could not be computed".to_owned(),
                    ),
                },
            };
            self.as_mut().set_versioning_panel_kind(QString::from(kind));
            self.as_mut().set_versioning_panel_data(QString::from(data));
            self.as_mut().set_versioning_panel_visible(true);
            self.as_mut().set_status_text(QString::from(status));
        }
        for event in restore_events {
            consumed = true;
            match event {
                RestorePlanEvent::Ready { request_id, plan } => {
                    let pristine =
                        self.as_ref().rust().pending_pristine_restore_request == Some(request_id);
                    if pristine {
                        let applied = {
                            let mut rust = self.as_mut().rust_mut();
                            let this = rust.as_mut().get_mut();
                            this.pending_pristine_restore_request = None;
                            let restore_media_positions = this.pending_pristine_restore_media;
                            let restore_editable_drafts = this.pending_pristine_restore_editable;
                            let tab_capable_surface = this.pending_pristine_restore_tab_surface;
                            this.pending_pristine_restore_media = false;
                            this.pending_pristine_restore_editable = false;
                            this.pending_pristine_restore_tab_surface = false;
                            this.state.as_mut().map(|application| {
                                let previous_revision = application.revision();
                                let result = if tab_capable_surface {
                                    application
                                        .apply_restore_to_pristine_window_with_options(
                                            &plan,
                                            restore_media_positions,
                                            restore_editable_drafts,
                                        )
                                        .map(|applied| applied.runtime_effects)
                                } else {
                                    application.apply_single_page_restore_to_pristine_window(
                                        &plan,
                                        restore_media_positions,
                                        restore_editable_drafts,
                                    )
                                };
                                result.map(|effects| (previous_revision, effects))
                            })
                        };
                        match applied {
                            Some(Ok((previous_revision, effects))) => {
                                let qt_effects = self
                                    .as_mut()
                                    .accept_runtime_effects(previous_revision, effects);
                                self.as_mut().sync_tab_order_from_core();
                                self.as_mut().set_pending_engine_action(&qt_effects);
                                let selected_page = plan
                                    .pages
                                    .iter()
                                    .find(|page| page.runtime_tab == plan.selected_tab);
                                let status = if selected_page.is_some_and(|page| {
                                    matches!(&page.navigation, RestoreNavigation::Placeholder(_))
                                }) {
                                    "Restored a structural placeholder; page URL was not safely replayable".to_owned()
                                } else if selected_page.is_none() {
                                    "Restored structure; selected page was unavailable".to_owned()
                                } else if plan.pages.len() > 1 {
                                    format!(
                                        "Restored {} structural pages; only the selected safe page was loaded",
                                        plan.pages.len()
                                    )
                                } else {
                                    "Restored structural page".to_owned()
                                };
                                self.as_mut().set_status_text(QString::from(status));
                            }
                            Some(Err(_)) | None => self.as_mut().set_status_text(QString::from(
                                "Versioned restore refused; this destination was not pristine",
                            )),
                        }
                    } else {
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .pending_engine_actions
                            .push_back(format!("versioning-restore-window\t{}", plan.commit));
                        self.as_mut().set_status_text(QString::from(
                            "Opening a new window for versioned restore",
                        ));
                    }
                }
                RestorePlanEvent::NotFound { request_id } => {
                    if self.as_ref().rust().pending_pristine_restore_request == Some(request_id) {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.pending_pristine_restore_request = None;
                        this.pending_pristine_restore_media = false;
                        this.pending_pristine_restore_editable = false;
                        this.pending_pristine_restore_tab_surface = false;
                    }
                    self.as_mut().set_status_text(QString::from(
                        "Versioned commit was not found in this profile",
                    ));
                }
                RestorePlanEvent::Failed { request_id } => {
                    if self.as_ref().rust().pending_pristine_restore_request == Some(request_id) {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.pending_pristine_restore_request = None;
                        this.pending_pristine_restore_media = false;
                        this.pending_pristine_restore_editable = false;
                        this.pending_pristine_restore_tab_surface = false;
                    }
                    self.as_mut().set_status_text(QString::from(
                        "Versioned restore plan could not be validated",
                    ));
                }
            }
        }
        for event in policy_events {
            consumed = true;
            match event {
                PolicyRestoreEvent::PreviewReady {
                    request_id,
                    preview,
                } => {
                    let data = serde_json::json!({
                        "requestId": request_id.to_string(),
                        "commit": preview.commit.to_string(),
                        "configurationRevision": preview.configuration_revision,
                        "key": preview.change.key,
                        "currentEnabled": preview.change.current_enabled,
                        "capturedEnabled": preview.change.captured_enabled,
                    });
                    self.as_mut()
                        .set_versioning_panel_kind(QString::from("policy-preview"));
                    self.as_mut()
                        .set_versioning_panel_data(QString::from(data.to_string()));
                    self.as_mut().set_versioning_panel_visible(true);
                    self.as_mut().set_status_text(QString::from(
                        "Review the allowlisted configuration change before applying it",
                    ));
                }
                PolicyRestoreEvent::Failed { .. } => {
                    self.as_mut()
                        .set_versioning_panel_kind(QString::from("error"));
                    self.as_mut().set_versioning_panel_data(QString::from(
                        serde_json::json!({
                            "message": "This commit has no supported policy snapshot, or its preview is no longer valid."
                        })
                        .to_string(),
                    ));
                    self.as_mut().set_versioning_panel_visible(true);
                    self.as_mut().set_status_text(QString::from(
                        "Allowlisted policy preview is unavailable",
                    ));
                }
            }
        }
        for event in sidecar_events {
            consumed = true;
            match event {
                RestoreSidecarEvent::Ready {
                    request_id,
                    destination_tab,
                    ..
                } => {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_actions
                        .push_back(format!(
                            "versioning-sidecar-ready\t{request_id}\t{destination_tab}"
                        ));
                }
                RestoreSidecarEvent::Missing { .. } => self.as_mut().set_status_text(
                    QString::from("No stored geometry sidecar was available for this page"),
                ),
                RestoreSidecarEvent::Failed { .. } => self.as_mut().set_status_text(QString::from(
                    "Stored geometry was rejected; structural restore is unchanged",
                )),
            }
        }
        consumed
    }
}

#[cfg(test)]
mod tests {
    use super::{CommitEvent, archive_source_display, commit_event_status, commit_info_panel_data};
    use ferric_browser_core::CommitId;
    use ferric_browser_storage::versioning::{
        CommitArchiveFailure, CommitInfo, CommitSummary, CoverageCounts,
        ManifestPageArchiveFailure, ManifestScope,
    };

    #[test]
    fn commit_event_status_does_not_expose_durable_or_operation_identity() {
        use ferric_browser_core::{CommitId, OperationId};

        let operation = OperationId::fresh();
        let commit = CommitId::fresh();
        let durable = commit_event_status(&CommitEvent::Durable { operation, commit });
        let failed = commit_event_status(&CommitEvent::Failed { operation, commit });

        assert_eq!(durable, "Versioned structural commit saved");
        assert_eq!(
            failed,
            "Versioned capture outcome unconfirmed; inspect the commit list"
        );
        for status in [durable, failed] {
            assert!(!status.contains(&operation.to_string()));
            assert!(!status.contains(&commit.to_string()));
        }
    }

    #[test]
    fn archive_source_metadata_is_sanitized_before_qml_presentation() {
        assert_eq!(
            archive_source_display(Some(
                "https://user:password@example.test/path?keep=1&token=secret#private",
            )),
            "https://example.test\u{2068}/path?keep=1\u{2069}"
        );
        assert_eq!(
            archive_source_display(Some(
                "https://example.test/a\u{061c}\u{200e}\u{200f}b\u{202e}\u{0007}",
            )),
            "https://example.test\u{2068}/a[bidi][bidi][bidi]b[bidi]�\u{2069}"
        );
        assert_eq!(archive_source_display(None), "");
    }

    #[test]
    fn commit_info_panel_projection_is_allowlisted_and_payload_free() {
        let info = CommitInfo {
            summary: CommitSummary {
                id: CommitId::fresh(),
                created_at_ms: 17,
                pinned: false,
                object_links: 1,
            },
            scope: ManifestScope::Window,
            window_count: 1,
            page_count: 2,
            replayable_pages: 1,
            unavailable_urls: [0, 0, 1],
            archive_objects: 1,
            sidecar_objects: 0,
            missing_objects: 0,
            quarantined_objects: 0,
            requested_capture: Vec::new(),
            partial_pages_allowed: true,
            archive_failures: vec![CommitArchiveFailure {
                logical_page_id: "opaque-page-id".to_owned(),
                reason: ManifestPageArchiveFailure::Timeout,
            }],
            archive_coverage: CoverageCounts {
                unavailable: 1,
                complete_for_request: 1,
                ..CoverageCounts::default()
            },
            page_coverage: CoverageCounts::default(),
            cookie_coverage: CoverageCounts::default(),
            engine_fingerprint: "private-engine-fingerprint".to_owned(),
            helper_bundle_version: "private-helper-version".to_owned(),
            warning_count: 0,
            max_manifest_bytes: 8 * 1024 * 1024,
            max_single_object_bytes: 64 * 1024 * 1024,
            max_commit_object_bytes: 2 * 1024 * 1024 * 1024,
        };

        let payload = commit_info_panel_data(&info);
        let encoded = payload.to_string();
        assert_eq!(payload["scope"], "window");
        assert_eq!(payload["archiveFailures"][0]["reason"], "timeout");
        assert_eq!(
            payload["archiveFailures"][0]["logicalPageId"],
            "opaque-page-id"
        );
        assert_eq!(payload["coverage"]["archive"]["unavailable"], 1);
        assert!(!encoded.contains("private-engine-fingerprint"));
        assert!(!encoded.contains("private-helper-version"));
        assert!(!encoded.contains("source-url-secret"));
        assert!(!encoded.contains("title-secret"));
        assert!(!encoded.contains("capture-message-secret"));
        assert!(!encoded.contains("private-stage-path"));
        assert!(!encoded.contains("cookie-value-secret"));
        assert!(!encoded.contains("archive-payload-secret"));
    }
}
