//! Bounded, storage-backed structural diff reads.
//!
//! INVARIANT: manifests are converted to the core projection in the
//! application boundary; only redacted structural change events leave it.
//! INVARIANT: a pending diff can publish only for its captured native-owner
//! generation, even when that owner is rebound to the same profile.

use ferric_browser_core::{
    CaptureField, CaptureUnavailable, CommitId, DiffKind, LogicalPageId, LogicalWindowId,
    NativeProfileOwner, SafePageUrl, ScrollPoint, StructuralPage, StructuralSnapshot,
    StructuralWindow, VersioningFailure, VersioningReason, diff_structural_kind,
    versioning::{Coverage, StructuralDiffPage},
};
use ferric_browser_storage::versioning::{
    ManifestCoverage, ManifestSafeUrl, ManifestScroll, ManifestUnavailable,
    VersionedCaptureManifest,
};
use ferric_browser_storage::{StorageCommand, StorageCompletion};
use std::fmt;
use uuid::Uuid;

use crate::{BrowserApplication, StorageTicket};

const DIFF_PAGE_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffSide {
    Left,
    Right,
}

#[derive(Clone)]
enum ManifestRead {
    Waiting,
    Found(Box<VersionedCaptureManifest>),
    Missing,
    Failed,
}

pub(crate) struct PendingStructuralDiff {
    window: ferric_browser_core::WindowId,
    profile_uuid: Uuid,
    native: NativeProfileOwner,
    left_id: CommitId,
    right_id: CommitId,
    kind: DiffKind,
    offset: usize,
    left: ManifestRead,
    right: ManifestRead,
}

#[derive(Clone, Eq, PartialEq)]
pub enum CommitDiffEvent {
    Compared {
        request_id: u64,
        offset: usize,
        left: CommitId,
        right: CommitId,
        kind: DiffKind,
        page: StructuralDiffPage,
        unsupported_categories: Vec<&'static str>,
    },
    NotFound {
        request_id: u64,
    },
    Failed {
        request_id: u64,
    },
}

impl fmt::Debug for CommitDiffEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compared {
                offset,
                kind,
                page,
                unsupported_categories,
                ..
            } => formatter
                .debug_struct("CommitDiffEvent::Compared")
                .field("request_id", &"[redacted]")
                .field("offset", offset)
                .field("left", &"[redacted]")
                .field("right", &"[redacted]")
                .field("kind", kind)
                .field("total_events", &page.total_events)
                .field("returned_events", &page.events.len())
                .field("has_next", &page.next_offset.is_some())
                .field("unsupported_category_count", &unsupported_categories.len())
                .finish(),
            Self::NotFound { .. } => formatter
                .debug_struct("CommitDiffEvent::NotFound")
                .field("request_id", &"[redacted]")
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("CommitDiffEvent::Failed")
                .field("request_id", &"[redacted]")
                .finish(),
        }
    }
}

impl BrowserApplication {
    /// Requests an exact, profile-scoped structural diff and one bounded page
    /// of results. Neither the manifest nor captured page values leave this
    /// application boundary.
    ///
    /// # Errors
    /// Refuses wrong/transient owners, ambiguous bounds and saturated queues.
    pub fn request_commit_diff(
        &mut self,
        window: ferric_browser_core::WindowId,
        left: CommitId,
        right: CommitId,
        kind: DiffKind,
        offset: usize,
    ) -> Result<StorageTicket, VersioningFailure> {
        if offset > 100_000 {
            return Err(failure(VersioningReason::QuotaExceeded));
        }
        if self.pending_commit_reads.len() > 14 || !self.pending_structural_diffs.is_empty() {
            return Err(failure(VersioningReason::Busy));
        }
        let (profile_uuid, native) = self.structural_commit_owner(window)?;
        let left_ticket = self
            .storage
            .submit(StorageCommand::VersioningManifest {
                profile_uuid,
                id: left,
            })
            .map_err(|_| failure(VersioningReason::Busy))?;
        let request_id = left_ticket.value();
        let Ok(right_ticket) = self.storage.submit(StorageCommand::VersioningManifest {
            profile_uuid,
            id: right,
        }) else {
            self.pending_commit_reads
                .insert(left_ticket, super::capture::CommitReadKind::Ignore);
            return Err(failure(VersioningReason::Busy));
        };
        self.pending_structural_diffs.insert(
            request_id,
            PendingStructuralDiff {
                window,
                profile_uuid,
                native,
                left_id: left,
                right_id: right,
                kind,
                offset,
                left: ManifestRead::Waiting,
                right: ManifestRead::Waiting,
            },
        );
        self.pending_commit_reads.insert(
            left_ticket,
            super::capture::CommitReadKind::Diff(request_id, DiffSide::Left),
        );
        self.pending_commit_reads.insert(
            right_ticket,
            super::capture::CommitReadKind::Diff(request_id, DiffSide::Right),
        );
        Ok(left_ticket)
    }

    pub(crate) fn route_commit_diff_completion(
        &mut self,
        request_id: u64,
        side: DiffSide,
        completion: StorageCompletion,
    ) {
        let owner_matches = self
            .pending_structural_diffs
            .get(&request_id)
            .is_some_and(|pending| {
                self.structural_commit_owner(pending.window)
                    .is_ok_and(|(profile, native)| {
                        profile == pending.profile_uuid && native == pending.native
                    })
            });
        if !owner_matches {
            if self.pending_structural_diffs.remove(&request_id).is_some() {
                self.commit_read_events
                    .push_back(super::capture::CommitReadEvent::Diff(
                        CommitDiffEvent::Failed { request_id },
                    ));
            }
            return;
        }
        let read = match completion {
            StorageCompletion::VersioningManifest(Ok(Some(manifest))) => {
                ManifestRead::Found(Box::new(manifest))
            }
            StorageCompletion::VersioningManifest(Ok(None)) => ManifestRead::Missing,
            _ => ManifestRead::Failed,
        };
        let Some(pending) = self.pending_structural_diffs.get_mut(&request_id) else {
            return;
        };
        match side {
            DiffSide::Left => pending.left = read,
            DiffSide::Right => pending.right = read,
        }
        if matches!(&pending.left, ManifestRead::Waiting)
            || matches!(&pending.right, ManifestRead::Waiting)
        {
            return;
        }
        let Some(pending) = self.pending_structural_diffs.remove(&request_id) else {
            return;
        };
        let event = match (pending.left, pending.right) {
            (ManifestRead::Found(left_manifest), ManifestRead::Found(right_manifest)) => {
                let result = structural_snapshot(&left_manifest)
                    .and_then(|left| {
                        structural_snapshot(&right_manifest).map(|right| (left, right))
                    })
                    .and_then(|(left, right)| {
                        diff_structural_kind(
                            &left,
                            &right,
                            Some(pending.kind),
                            pending.offset,
                            DIFF_PAGE_LIMIT,
                        )
                        .map_err(|_| ())
                    });
                match result {
                    Ok(page) => CommitDiffEvent::Compared {
                        request_id,
                        offset: pending.offset,
                        left: pending.left_id,
                        right: pending.right_id,
                        kind: pending.kind,
                        page,
                        unsupported_categories: match pending.kind {
                            DiffKind::Browser => vec![
                                "Page bodies, form values, archive contents, and policy changes are not included in structural diffs.",
                            ],
                            DiffKind::Page => vec![
                                "DOM/page-body content, form values, archive contents, and policy changes are not included in structural diffs.",
                            ],
                            DiffKind::Cookies => vec![
                                "Cookie names, identity, and values are not captured by structural snapshots.",
                            ],
                        },
                    },
                    Err(()) => CommitDiffEvent::Failed { request_id },
                }
            }
            (ManifestRead::Missing, _) | (_, ManifestRead::Missing) => {
                CommitDiffEvent::NotFound { request_id }
            }
            _ => CommitDiffEvent::Failed { request_id },
        };
        self.commit_read_events
            .push_back(super::capture::CommitReadEvent::Diff(event));
    }
}

fn structural_snapshot(manifest: &VersionedCaptureManifest) -> Result<StructuralSnapshot, ()> {
    let windows = manifest
        .windows
        .iter()
        .map(|window| {
            let logical_id = window
                .logical_window_id
                .parse::<LogicalWindowId>()
                .map_err(|_| ())?;
            let selected_page = window
                .selected_logical_page
                .as_deref()
                .map(str::parse::<LogicalPageId>)
                .transpose()
                .map_err(|_| ())?;
            let pages = window
                .pages
                .iter()
                .map(|page| {
                    Ok(StructuralPage {
                        logical_id: page
                            .logical_page_id
                            .parse::<LogicalPageId>()
                            .map_err(|_| ())?,
                        lineage_parent: page
                            .lineage_parent
                            .as_deref()
                            .map(str::parse::<LogicalPageId>)
                            .transpose()
                            .map_err(|_| ())?,
                        url: safe_url(&page.safe_url_or_placeholder),
                        pinned: page.pinned,
                        muted: page.muted,
                        zoom_hundredths: page.zoom_hundredths,
                        scroll: match page.document_scroll {
                            ManifestScroll::Known {
                                x_hundredths,
                                y_hundredths,
                            } => CaptureField::Known(ScrollPoint {
                                x_hundredths,
                                y_hundredths,
                            }),
                            ManifestScroll::Absent => CaptureField::Absent,
                            ManifestScroll::Unavailable(reason) => {
                                CaptureField::Unavailable(unavailable(reason))
                            }
                        },
                        page_coverage: coverage(page.fidelity.page),
                        cookie_coverage: coverage(page.fidelity.cookies),
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?;
            Ok(StructuralWindow {
                logical_id,
                context_ref: window.context_ref.clone(),
                selected_page,
                pages,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    Ok(StructuralSnapshot {
        application_revision: manifest.application_revision,
        windows,
    })
}

fn safe_url(url: &ManifestSafeUrl) -> SafePageUrl {
    match url {
        ManifestSafeUrl::Replayable(url) => {
            let validated = SafePageUrl::from_observed(Some(url), true);
            if validated.replay_url().is_some() {
                validated
            } else {
                SafePageUrl::placeholder(CaptureUnavailable::UnsafeNavigation)
            }
        }
        ManifestSafeUrl::Placeholder(reason) => SafePageUrl::placeholder(unavailable(*reason)),
    }
}

fn unavailable(reason: ManifestUnavailable) -> CaptureUnavailable {
    match reason {
        ManifestUnavailable::NotObserved => CaptureUnavailable::NotObserved,
        ManifestUnavailable::Unsupported => CaptureUnavailable::Unsupported,
        ManifestUnavailable::UnsafeNavigation => CaptureUnavailable::UnsafeNavigation,
    }
}

fn coverage(value: ManifestCoverage) -> Coverage {
    let counts = match value {
        ManifestCoverage::Unknown => return Coverage::unknown(),
        ManifestCoverage::NotRequested => (0, 0),
        ManifestCoverage::Unavailable => (1, 0),
        ManifestCoverage::Partial => (2, 1),
        ManifestCoverage::CompleteForRequest => (1, 1),
    };
    Coverage::from_counts(Some(counts.0), Some(counts.1)).unwrap_or_else(|_| Coverage::unknown())
}

fn failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(
        reason,
        ferric_browser_core::versioning::SensitiveCategory::None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_storage::versioning::{
        ManifestFidelity, ManifestPage, ManifestScope, ManifestWindow,
    };

    #[test]
    fn commit_diff_event_debug_redacts_commit_and_request_identity() {
        let left = CommitId::fresh();
        let right = CommitId::fresh();
        let left_text = left.to_string();
        let right_text = right.to_string();
        let event = CommitDiffEvent::Compared {
            request_id: 987_654_321,
            offset: 0,
            left,
            right,
            kind: DiffKind::Browser,
            page: StructuralDiffPage {
                total_events: 0,
                next_offset: None,
                events: Vec::new(),
            },
            unsupported_categories: Vec::new(),
        };
        let debug = format!("{event:?}");
        assert!(!debug.contains(left_text.as_str()));
        assert!(!debug.contains(right_text.as_str()));
        assert!(!debug.contains("987654321"));
    }

    fn fixture() -> VersionedCaptureManifest {
        let window = LogicalWindowId::fresh();
        let page = LogicalPageId::fresh();
        VersionedCaptureManifest {
            schema_version: 1,
            id: CommitId::fresh().to_string(),
            profile_uuid: uuid::Uuid::new_v4(),
            created_at_ms: 1,
            message: "never projected".to_owned(),
            parent_commit: None,
            application_revision: 2,
            configuration_revision: 0,
            scope: ManifestScope::Window,
            requested_capture: ferric_browser_storage::versioning::CaptureRequest::structural_only(
            ),
            capture_interval: ferric_browser_storage::versioning::CaptureInterval {
                started_at_ms: 1,
                finished_at_ms: 1,
            },
            windows: vec![ManifestWindow {
                logical_window_id: window.to_string(),
                context_ref: Some("private context label".to_owned()),
                capture_interval: None,
                context_member_ordinal: None,
                selected_logical_page: Some(page.to_string()),
                pages: vec![ManifestPage {
                    logical_page_id: page.to_string(),
                    lineage_parent: None,
                    safe_url_or_placeholder: ManifestSafeUrl::Replayable(
                        "https://example.test/private-path".to_owned(),
                    ),
                    title_redacted: Some("private title".to_owned()),
                    pinned: false,
                    muted: false,
                    zoom_hundredths: 100,
                    document_scroll: ManifestScroll::Absent,
                    archive_object: None,
                    archive_failure: None,
                    sidecar_object: None,
                    fidelity: ManifestFidelity {
                        structural: true,
                        archive: ManifestCoverage::NotRequested,
                        page: ManifestCoverage::NotRequested,
                        cookies: ManifestCoverage::NotRequested,
                    },
                    page_capture: None,
                }],
            }],
            context_member_count: None,
            invalid_context_members: Vec::new(),
            policy_snapshot: None,
            engine_fingerprint: "fingerprint".to_owned(),
            helper_bundle_version: "helpers".to_owned(),
            warnings: vec!["private warning".to_owned()],
        }
    }

    #[test]
    fn manifest_conversion_retains_only_typed_structural_fields() {
        let manifest = fixture();
        let snapshot = structural_snapshot(&manifest).expect("valid fixture");
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].pages.len(), 1);
        let mut changed = manifest.clone();
        changed.windows[0].context_ref = Some("another private label".to_owned());
        changed.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/other-private-path".to_owned());
        let changed = structural_snapshot(&changed).expect("valid changed fixture");
        let page = diff_structural_kind(&snapshot, &changed, Some(DiffKind::Page), 0, 10)
            .expect("bounded page diff");
        let serialized = serde_json::to_string(&page).expect("serializes redacted diff DTO");
        assert!(serialized.contains("url-changed"));
        assert!(!serialized.contains("private title"));
        assert!(!serialized.contains("private context label"));
        assert!(!serialized.contains("private warning"));
        assert!(!serialized.contains("private-path"));
        assert!(!serialized.contains("another private label"));
    }
}
