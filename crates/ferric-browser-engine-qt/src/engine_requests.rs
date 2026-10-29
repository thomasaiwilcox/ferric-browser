//! Projection of captured reducer effects into native request records.

use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString, QVariant};
use ferric_browser_core::{EngineEffect, SearchCase, Target};
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_QUEUE_ID: AtomicU64 = AtomicU64::new(1);

/// The pending queue owns its storage and invalidates any issued claim before
/// every mutation. Deref is deliberately read-only; mutable queue access never
/// escapes this wrapper.
// This experiment defines the claim seam and verifies it directly, but does
// not wire a production consumer yet. Keep the dormant claim metadata local.
#[allow(dead_code)]
pub(super) struct PendingEngineRequests {
    requests: VecDeque<NativeRequest>,
    queue_id: Option<u64>,
    revision: Option<u64>,
    next_claim_id: Option<u64>,
    active_claim: Option<u64>,
}

impl PendingEngineRequests {
    pub(super) fn new() -> Self {
        let queue_id = NEXT_QUEUE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .ok();
        Self {
            requests: VecDeque::new(),
            queue_id,
            revision: Some(0),
            next_claim_id: Some(1),
            active_claim: None,
        }
    }

    fn invalidate_claim(&mut self) {
        self.active_claim = None;
        self.revision = self.revision.and_then(|revision| revision.checked_add(1));
    }

    #[allow(dead_code)]
    pub(super) fn claim_head(&mut self, consumer_epoch: u64) -> Option<PendingRequestClaim> {
        if self.active_claim.is_some() || self.requests.is_empty() {
            return None;
        }
        let queue_id = self.queue_id?;
        let revision = self.revision?;
        let claim_id = self.next_claim_id?;
        self.next_claim_id = claim_id.checked_add(1);
        self.active_claim = Some(claim_id);
        Some(PendingRequestClaim {
            queue_id,
            revision,
            consumer_epoch,
            claim_id,
        })
    }

    fn is_current(&self, claim: &PendingRequestClaim, consumer_epoch: u64) -> bool {
        self.queue_id == Some(claim.queue_id)
            && self.revision == Some(claim.revision)
            && self.active_claim == Some(claim.claim_id)
            && claim.consumer_epoch == consumer_epoch
    }

    #[allow(dead_code)]
    pub(super) fn claimed_head(
        &self,
        claim: &PendingRequestClaim,
        consumer_epoch: u64,
    ) -> Option<&NativeRequest> {
        self.is_current(claim, consumer_epoch)
            .then(|| self.requests.front())
            .flatten()
    }

    #[allow(dead_code)]
    pub(super) fn commit(
        &mut self,
        claim: &PendingRequestClaim,
        consumer_epoch: u64,
    ) -> Option<NativeRequest> {
        if !self.is_current(claim, consumer_epoch) {
            return None;
        }
        self.invalidate_claim();
        self.requests.pop_front()
    }

    #[allow(dead_code)]
    pub(super) fn release(&mut self, claim: &PendingRequestClaim, consumer_epoch: u64) -> bool {
        if !self.is_current(claim, consumer_epoch) {
            return false;
        }
        self.active_claim = None;
        true
    }

    pub(super) fn push_back(&mut self, request: NativeRequest) {
        self.invalidate_claim();
        self.requests.push_back(request);
    }

    pub(super) fn pop_front(&mut self) -> Option<NativeRequest> {
        self.invalidate_claim();
        self.requests.pop_front()
    }

    pub(super) fn pop_back(&mut self) -> Option<NativeRequest> {
        self.invalidate_claim();
        self.requests.pop_back()
    }

    pub(super) fn clear(&mut self) {
        self.invalidate_claim();
        self.requests.clear();
    }

    pub(super) fn retain(&mut self, keep: impl FnMut(&NativeRequest) -> bool) {
        self.invalidate_claim();
        self.requests.retain(keep);
    }
}

impl Default for PendingEngineRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for PendingEngineRequests {
    type Target = VecDeque<NativeRequest>;

    fn deref(&self) -> &Self::Target {
        &self.requests
    }
}

#[allow(dead_code)] // Constructed only by the not-yet-wired consumer claim seam.
#[derive(Debug)]
pub(super) struct PendingRequestClaim {
    queue_id: u64,
    revision: u64,
    consumer_epoch: u64,
    claim_id: u64,
}

#[derive(Clone, Debug)]
pub(super) enum NativeRequest {
    Engine(EngineEffect),
    ReparentView {
        effect: EngineEffect,
        target: Target,
        profile_id: String,
        host_token: String,
        page_owner_token: String,
        operation: PageHostOperation,
    },
    View(ferric_browser_application::ViewRequest),
    Presentation(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PageHostOperation {
    Attach,
    Detach,
}

impl PageHostOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Attach => "attach",
            Self::Detach => "detach",
        }
    }

    pub(super) fn attaches(self) -> bool {
        self == Self::Attach
    }
}

pub(super) fn target(effect: &EngineEffect) -> Option<Target> {
    match effect {
        EngineEffect::Navigate { target, .. }
        | EngineEffect::Reload { target, .. }
        | EngineEffect::ClipboardEdit { target, .. }
        | EngineEffect::Stop { target }
        | EngineEffect::TraverseHistory { target, .. }
        | EngineEffect::FindText { target, .. }
        | EngineEffect::ClearFindText { target } => Some(*target),
        _ => None,
    }
}

pub(super) fn project(effect: &EngineEffect) -> QVariant {
    let mut row = QMap::<QMapPair_QString_QVariant>::default();
    let mut text = |key: &str, value: &str| {
        row.insert(QString::from(key), QVariant::from(&QString::from(value)));
    };
    if let Some(target) = target(effect) {
        text("tabId", &target.tab.to_string());
        text("generation", &target.generation.to_string());
        text("documentId", &target.document.to_string());
    }
    match effect {
        EngineEffect::Navigate { url, .. } => {
            text("kind", "navigate");
            text("url", url.as_str());
        }
        EngineEffect::Reload { bypass_cache, .. } => {
            text("kind", "reload");
            row.insert(QString::from("bypassCache"), QVariant::from(bypass_cache));
        }
        EngineEffect::Stop { .. } => text("kind", "stop"),
        EngineEffect::TraverseHistory { offset, .. } => {
            text("kind", "history");
            row.insert(QString::from("offset"), QVariant::from(offset));
        }
        EngineEffect::FindText {
            new_search,
            query,
            backward,
            case,
            ..
        } => {
            text("kind", "find");
            text("query", query);
            text(
                "caseMode",
                match case {
                    SearchCase::Smart => "smart",
                    SearchCase::Sensitive => "sensitive",
                    SearchCase::Insensitive => "insensitive",
                },
            );
            row.insert(QString::from("newSearch"), QVariant::from(new_search));
            row.insert(QString::from("backward"), QVariant::from(backward));
        }
        EngineEffect::ClearFindText { .. } => text("kind", "clear-find"),
        EngineEffect::ClipboardEdit { operation, .. } => {
            text("kind", "edit");
            text("operation", operation.as_str());
        }
        _ => return QVariant::default(),
    }
    QVariant::from(&row)
}

impl NativeRequest {
    pub(super) fn captured_target(&self) -> Option<Target> {
        match self {
            Self::Engine(effect) => target(effect),
            Self::ReparentView { target, .. } => Some(*target),
            Self::View(request) => Some(request.target),
            Self::Presentation(_) => None,
        }
    }

    pub(super) fn page_host_tokens(&self) -> Option<(&str, &str, &str, bool)> {
        match self {
            Self::ReparentView {
                profile_id,
                host_token,
                page_owner_token,
                operation,
                ..
            } => Some((
                profile_id,
                host_token,
                page_owner_token,
                operation.attaches(),
            )),
            _ => None,
        }
    }

    pub(super) fn projection(&self) -> QVariant {
        match self {
            Self::Engine(effect) => project(effect),
            Self::ReparentView {
                effect,
                target,
                profile_id,
                host_token,
                page_owner_token,
                operation,
            } => project_reparent_view(
                effect,
                *target,
                profile_id,
                host_token,
                page_owner_token,
                *operation,
            ),
            Self::View(request) => project_view(request),
            Self::Presentation(_) => QVariant::default(),
        }
    }
}

fn project_reparent_view(
    effect: &EngineEffect,
    target: Target,
    profile_id: &str,
    host_token: &str,
    page_owner_token: &str,
    operation: PageHostOperation,
) -> QVariant {
    let EngineEffect::ReparentView {
        tab,
        generation,
        window,
    } = effect
    else {
        return QVariant::default();
    };
    if *tab != target.tab || *generation != target.generation {
        return QVariant::default();
    }

    let mut row = QMap::<QMapPair_QString_QVariant>::default();
    let mut text = |key: &str, value: &str| {
        row.insert(QString::from(key), QVariant::from(&QString::from(value)));
    };
    text("kind", "reparent-view");
    text("operation", operation.as_str());
    text("tabId", &target.tab.to_string());
    text("generation", &target.generation.to_string());
    text("documentId", &target.document.to_string());
    text("windowId", &window.to_string());
    text("profileId", profile_id);
    text("hostToken", host_token);
    text("pageOwnerToken", page_owner_token);
    QVariant::from(&row)
}

fn project_view(request: &ferric_browser_application::ViewRequest) -> QVariant {
    use ferric_browser_application::{
        ScriptWorld, ScrollEdge, ScrollTargetPolicy, VerticalDirection, ViewCommand,
    };
    fn text(row: &mut QMap<QMapPair_QString_QVariant>, key: &str, value: &str) {
        row.insert(QString::from(key), QVariant::from(&QString::from(value)));
    }
    let mut row = QMap::<QMapPair_QString_QVariant>::default();
    text(&mut row, "tabId", &request.target.tab.to_string());
    text(
        &mut row,
        "generation",
        &request.target.generation.to_string(),
    );
    text(&mut row, "documentId", &request.target.document.to_string());
    let kind = match &request.command {
        ViewCommand::Scroll { direction, count } => {
            text(&mut row, "direction", direction.as_str());
            row.insert(QString::from("count"), QVariant::from(count));
            "scroll"
        }
        ViewCommand::ScrollPage {
            direction,
            half,
            count,
        } => {
            text(
                &mut row,
                "direction",
                match direction {
                    VerticalDirection::Up => "up",
                    VerticalDirection::Down => "down",
                },
            );
            row.insert(QString::from("count"), QVariant::from(count));
            row.insert(QString::from("half"), QVariant::from(half));
            "scroll-page"
        }
        ViewCommand::ScrollTo { edge } => {
            text(
                &mut row,
                "direction",
                match edge {
                    ScrollEdge::Top => "top",
                    ScrollEdge::Bottom => "bottom",
                },
            );
            "scroll-to"
        }
        ViewCommand::ScrollTarget(policy) => {
            text(
                &mut row,
                "action",
                match policy {
                    ScrollTargetPolicy::Auto => "auto",
                    ScrollTargetPolicy::Document => "document",
                    ScrollTargetPolicy::Status => "status",
                },
            );
            "scroll-target"
        }
        ViewCommand::Zoom { hundredths } => {
            row.insert(
                QString::from("factor"),
                QVariant::from(&(f64::from(*hundredths) / 100.0)),
            );
            "zoom"
        }
        ViewCommand::PrintPdf { path } => {
            text(&mut row, "path", &path.to_string_lossy());
            "print-pdf"
        }
        ViewCommand::SavePage { path } => {
            text(&mut row, "path", &path.to_string_lossy());
            "save-page"
        }
        ViewCommand::ViewSource { url } => {
            text(&mut row, "url", url.as_str());
            "view-source"
        }
        ViewCommand::DeveloperTools { detach } => {
            row.insert(QString::from("detach"), QVariant::from(detach));
            "devtools"
        }
        ViewCommand::Print => "print",
        ViewCommand::SiteDoctorReload { experiment_id } => {
            text(&mut row, "experimentId", experiment_id);
            "site-doctor-reload"
        }
        ViewCommand::SiteDoctorFreshView { experiment_id, url } => {
            text(&mut row, "experimentId", experiment_id);
            text(&mut row, "url", url.as_str());
            "site-doctor-fresh-view"
        }
        ViewCommand::Evaluate { script, world } => {
            text(&mut row, "script", script);
            text(
                &mut row,
                "world",
                match world {
                    ScriptWorld::Application => "application",
                    ScriptWorld::Page => "page",
                },
            );
            "evaluate"
        }
    };
    text(&mut row, "kind", kind);
    QVariant::from(&row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{DocumentId, SearchCase, TabId};

    fn target(n: u64) -> Target {
        Target {
            tab: TabId::from_display(&format!("tabid-{n}")).expect("tab id"),
            generation: n,
            document: DocumentId::from_display(&format!("documentid-{n}")).expect("document id"),
        }
    }

    fn find(n: u64) -> NativeRequest {
        NativeRequest::Engine(EngineEffect::FindText {
            new_search: true,
            target: target(n),
            query: format!("query-{n}"),
            backward: false,
            case: SearchCase::Sensitive,
        })
    }

    fn clear(n: u64) -> NativeRequest {
        NativeRequest::Engine(EngineEffect::ClearFindText { target: target(n) })
    }

    fn front_target(queue: &PendingEngineRequests) -> Option<Target> {
        queue.front().and_then(NativeRequest::captured_target)
    }

    #[test]
    fn claim_retains_head_and_commit_is_once() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let claim = queue.claim_head(7).expect("claim");
        assert!(matches!(
            queue.claimed_head(&claim, 7),
            Some(NativeRequest::Engine(EngineEffect::FindText { .. }))
        ));
        assert_eq!(front_target(&queue), Some(target(1)));
        assert!(queue.commit(&claim, 7).is_some());
        assert!(queue.commit(&claim, 7).is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn clear_then_identical_reinsert_rejects_old_claim() {
        let mut queue = PendingEngineRequests::new();
        let request = find(1);
        queue.push_back(request.clone());
        let claim = queue.claim_head(1).unwrap();
        queue.clear();
        queue.push_back(request);
        assert!(queue.claimed_head(&claim, 1).is_none());
        assert!(queue.commit(&claim, 1).is_none());
        assert_eq!(front_target(&queue), Some(target(1)));
    }

    #[test]
    fn retain_cancellation_rejects_claimed_find() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let claim = queue.claim_head(4).unwrap();
        queue.retain(|request| {
            !matches!(
                request,
                NativeRequest::Engine(EngineEffect::FindText { .. })
            )
        });
        assert!(queue.commit(&claim, 4).is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn pop_back_search_coalescing_rejects_claim() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        queue.push_back(find(1));
        let claim = queue.claim_head(1).unwrap();
        while matches!(queue.back(), Some(NativeRequest::Engine(EngineEffect::FindText { target: queued, .. })) if *queued == target(1))
        {
            queue.pop_back();
        }
        assert!(queue.commit(&claim, 1).is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn append_invalidates_without_losing_fifo() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let claim = queue.claim_head(2).unwrap();
        queue.push_back(clear(2));
        assert!(queue.commit(&claim, 2).is_none());
        assert_eq!(front_target(&queue), Some(target(1)));
        assert_eq!(
            queue.iter().nth(1).and_then(NativeRequest::captured_target),
            Some(target(2))
        );
    }

    #[test]
    fn wrong_queue_and_epoch_claims_are_rejected() {
        let mut first = PendingEngineRequests::new();
        let mut second = PendingEngineRequests::new();
        first.push_back(find(1));
        second.push_back(clear(2));
        let claim = first.claim_head(9).unwrap();
        assert!(second.claimed_head(&claim, 9).is_none());
        assert!(first.claimed_head(&claim, 8).is_none());
        assert!(first.commit(&claim, 8).is_none());
        assert_eq!(front_target(&first), Some(target(1)));
        assert!(first.commit(&claim, 9).is_some());
    }

    #[test]
    fn release_after_clear_never_restores_work() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let claim = queue.claim_head(3).unwrap();
        queue.clear();
        assert!(!queue.release(&claim, 3));
        assert!(queue.is_empty());
    }

    #[test]
    fn double_ended_iteration_and_legacy_pop_match_existing_order() {
        let mut queue = PendingEngineRequests::new();
        let mut oracle = VecDeque::new();
        for n in 1..=3 {
            queue.push_back(if n == 2 { clear(n) } else { find(n) });
            oracle.push_back(target(n));
        }
        let forward: Vec<_> = queue
            .iter()
            .filter_map(NativeRequest::captured_target)
            .collect();
        let reverse: Vec<_> = queue
            .iter()
            .rev()
            .filter_map(NativeRequest::captured_target)
            .collect();
        assert_eq!(forward, oracle.iter().copied().collect::<Vec<_>>());
        assert_eq!(reverse, oracle.iter().rev().copied().collect::<Vec<_>>());
        while let Some(expected) = oracle.pop_front() {
            assert_eq!(
                queue.pop_front().and_then(|r| r.captured_target()),
                Some(expected)
            );
        }
        assert!(queue.is_empty());
    }

    #[test]
    fn release_then_reclaim_rejects_old_claim() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let old = queue.claim_head(5).unwrap();
        assert!(queue.release(&old, 5));
        let new = queue.claim_head(5).unwrap();
        assert!(queue.commit(&old, 5).is_none());
        assert!(queue.commit(&new, 5).is_some());
    }

    #[test]
    fn concurrent_claims_are_rejected() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let first = queue.claim_head(1).unwrap();
        assert!(queue.claim_head(1).is_none());
        assert!(queue.claimed_head(&first, 1).is_some());
    }

    #[test]
    fn noop_clear_and_retain_invalidate_claim() {
        let mut empty = PendingEngineRequests::new();
        empty.clear();
        assert_eq!(empty.revision, Some(1));

        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        let after_clear = queue.claim_head(1).unwrap();
        queue.clear();
        assert!(queue.commit(&after_clear, 1).is_none());
        queue.push_back(find(2));
        let after_retain = queue.claim_head(1).unwrap();
        queue.retain(|_| true);
        assert!(queue.commit(&after_retain, 1).is_none());
        assert_eq!(front_target(&queue), Some(target(2)));
    }

    #[test]
    fn claim_identity_exhaustion_refuses_claims_but_preserves_mutation() {
        let mut queue_id_exhausted = PendingEngineRequests::new();
        queue_id_exhausted.queue_id = None;
        queue_id_exhausted.push_back(find(1));
        assert!(queue_id_exhausted.claim_head(1).is_none());
        queue_id_exhausted.push_back(clear(2));
        assert_eq!(queue_id_exhausted.len(), 2);
        assert_eq!(front_target(&queue_id_exhausted), Some(target(1)));

        let mut claim_id_exhausted = PendingEngineRequests::new();
        claim_id_exhausted.next_claim_id = Some(u64::MAX);
        claim_id_exhausted.push_back(find(3));
        let final_claim = claim_id_exhausted.claim_head(1).expect("last unique claim");
        assert!(claim_id_exhausted.commit(&final_claim, 1).is_some());
        claim_id_exhausted.push_back(clear(4));
        assert!(claim_id_exhausted.claim_head(1).is_none());
        assert_eq!(front_target(&claim_id_exhausted), Some(target(4)));

        let mut revision_exhausted = PendingEngineRequests::new();
        revision_exhausted.revision = Some(u64::MAX);
        revision_exhausted.push_back(find(5));
        assert!(revision_exhausted.claim_head(1).is_none());
        revision_exhausted.clear();
        assert!(revision_exhausted.is_empty());
    }

    #[test]
    fn pop_front_invalidates_claim() {
        let mut queue = PendingEngineRequests::new();
        queue.push_back(find(1));
        queue.push_back(clear(2));
        let claim = queue.claim_head(1).unwrap();
        assert_eq!(
            queue.pop_front().and_then(|r| r.captured_target()),
            Some(target(1))
        );
        assert!(queue.commit(&claim, 1).is_none());
        assert_eq!(front_target(&queue), Some(target(2)));
    }
}
