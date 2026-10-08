//! Pure, bounded semantic comparison of immutable structural projections.
//! Runtime tab/window IDs and page contents never enter a diff event.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::Serialize;

use super::capture::{
    CaptureField, CaptureUnavailable, CoverageKind, SafePageUrl, ScrollPoint, StructuralPage,
    StructuralSnapshot, StructuralWindow,
};
use super::limits::{MAX_LIST_ITEMS, MAX_PAGES_PER_WINDOW_CAPTURE};
use super::{DiffKind, LogicalPageId, LogicalWindowId};

const MAX_STRUCTURAL_WINDOWS: usize = 256;
const MAX_STRUCTURAL_PAGES: usize = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralChange {
    WindowAdded,
    WindowRemoved,
    WindowReordered,
    ContextChanged,
    SelectedPageChanged,
    PageAdded,
    PageRemoved,
    PageMoved,
    PageReordered,
    UrlChanged,
    UrlAvailabilityChanged,
    PinnedChanged,
    MutedChanged,
    ZoomChanged,
    ScrollChanged,
    ScrollAvailabilityChanged,
    PageCoverageChanged,
    CookieCoverageChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralDiffEvent {
    pub window: LogicalWindowId,
    pub page: Option<LogicalPageId>,
    pub change: StructuralChange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_availability: Option<FieldAvailability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_availability: Option<FieldAvailability>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "kebab-case")]
pub enum FieldAvailability {
    Known,
    Absent,
    Unavailable(CaptureUnavailable),
    Coverage(CoverageKind),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralDiffPage {
    pub total_events: usize,
    pub next_offset: Option<usize>,
    pub events: Vec<StructuralDiffEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralDiffError {
    InvalidSnapshot,
    InvalidPageSize,
    InvalidOffset,
}

impl fmt::Display for StructuralDiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "structural diff rejected: {self:?}")
    }
}

impl std::error::Error for StructuralDiffError {}

struct IndexedSnapshot<'a> {
    windows: BTreeMap<LogicalWindowId, (usize, &'a StructuralWindow)>,
    pages: BTreeMap<LogicalPageId, (LogicalWindowId, usize, &'a StructuralPage)>,
}

fn index(snapshot: &StructuralSnapshot) -> Result<IndexedSnapshot<'_>, StructuralDiffError> {
    if snapshot.windows.len() > MAX_STRUCTURAL_WINDOWS {
        return Err(StructuralDiffError::InvalidSnapshot);
    }
    let mut windows = BTreeMap::new();
    let mut pages = BTreeMap::new();
    for (window_index, window) in snapshot.windows.iter().enumerate() {
        if window.pages.len() > MAX_PAGES_PER_WINDOW_CAPTURE
            || windows
                .insert(window.logical_id, (window_index, window))
                .is_some()
        {
            return Err(StructuralDiffError::InvalidSnapshot);
        }
        if window
            .selected_page
            .is_some_and(|selected| !window.pages.iter().any(|page| page.logical_id == selected))
        {
            return Err(StructuralDiffError::InvalidSnapshot);
        }
        for (page_index, page) in window.pages.iter().enumerate() {
            if !(25..=500).contains(&page.zoom_hundredths)
                || page.lineage_parent == Some(page.logical_id)
                || matches!(&page.scroll, CaptureField::Known(point)
                    if point.x_hundredths > 100_000_000 || point.y_hundredths > 100_000_000)
            {
                return Err(StructuralDiffError::InvalidSnapshot);
            }
            if pages
                .insert(page.logical_id, (window.logical_id, page_index, page))
                .is_some()
            {
                return Err(StructuralDiffError::InvalidSnapshot);
            }
        }
        if pages.len() > MAX_STRUCTURAL_PAGES {
            return Err(StructuralDiffError::InvalidSnapshot);
        }
    }
    Ok(IndexedSnapshot { windows, pages })
}

fn emit(
    events: &mut Vec<StructuralDiffEvent>,
    window: LogicalWindowId,
    page: Option<LogicalPageId>,
    change: StructuralChange,
) {
    events.push(StructuralDiffEvent {
        window,
        page,
        change,
        before_availability: None,
        after_availability: None,
    });
}

fn emit_availability(
    events: &mut Vec<StructuralDiffEvent>,
    window: LogicalWindowId,
    page: LogicalPageId,
    change: StructuralChange,
    before: FieldAvailability,
    after: FieldAvailability,
) {
    events.push(StructuralDiffEvent {
        window,
        page: Some(page),
        change,
        before_availability: Some(before),
        after_availability: Some(after),
    });
}

fn url_availability(url: &SafePageUrl) -> FieldAvailability {
    url.unavailable_reason()
        .map_or(FieldAvailability::Known, FieldAvailability::Unavailable)
}

fn scroll_availability(scroll: &CaptureField<ScrollPoint>) -> FieldAvailability {
    match scroll {
        CaptureField::Known(_) => FieldAvailability::Known,
        CaptureField::Absent => FieldAvailability::Absent,
        CaptureField::Unavailable(reason) => FieldAvailability::Unavailable(*reason),
    }
}

fn common_window_rank(
    index: &IndexedSnapshot<'_>,
    other: &IndexedSnapshot<'_>,
    id: LogicalWindowId,
) -> Option<usize> {
    let (position, _) = index.windows.get(&id)?;
    Some(
        index
            .windows
            .iter()
            .filter(|(candidate, (candidate_position, _))| {
                *candidate_position < *position && other.windows.contains_key(candidate)
            })
            .count(),
    )
}

fn common_page_rank(
    index: &IndexedSnapshot<'_>,
    other: &IndexedSnapshot<'_>,
    id: LogicalPageId,
) -> Option<usize> {
    let (window, _, _) = index.pages.get(&id)?;
    let (_, parent) = index.windows.get(window)?;
    parent
        .pages
        .iter()
        .filter(|page| {
            other
                .pages
                .get(&page.logical_id)
                .is_some_and(|(other_window, _, _)| other_window == window)
        })
        .position(|page| page.logical_id == id)
}

/// Compares logical user state in a deterministic order and returns only one
/// bounded page of change classifications. URLs and other values are never
/// copied into the event projection.
///
/// # Errors
/// Rejects malformed/oversized snapshots and invalid pagination parameters.
#[allow(clippy::too_many_lines)]
pub fn diff_structural(
    before: &StructuralSnapshot,
    after: &StructuralSnapshot,
    offset: usize,
    limit: usize,
) -> Result<StructuralDiffPage, StructuralDiffError> {
    diff_structural_kind(before, after, None, offset, limit)
}

/// Compares one explicit semantic category before applying bounded
/// pagination. Cookie values and identities are never part of this diff;
/// the cookie category reports only capture-coverage changes.
///
/// # Errors
/// Rejects malformed/oversized snapshots and invalid pagination parameters.
#[allow(clippy::too_many_lines)]
pub fn diff_structural_kind(
    before: &StructuralSnapshot,
    after: &StructuralSnapshot,
    kind: Option<DiffKind>,
    offset: usize,
    limit: usize,
) -> Result<StructuralDiffPage, StructuralDiffError> {
    if !(1..=MAX_LIST_ITEMS).contains(&limit) {
        return Err(StructuralDiffError::InvalidPageSize);
    }
    let left = index(before)?;
    let right = index(after)?;
    let mut events = Vec::new();
    let window_ids: BTreeSet<_> = left
        .windows
        .keys()
        .chain(right.windows.keys())
        .copied()
        .collect();
    for id in window_ids {
        match (left.windows.get(&id), right.windows.get(&id)) {
            (None, Some(_)) => emit(&mut events, id, None, StructuralChange::WindowAdded),
            (Some(_), None) => emit(&mut events, id, None, StructuralChange::WindowRemoved),
            (Some((_, old)), Some((_, new))) => {
                if common_window_rank(&left, &right, id) != common_window_rank(&right, &left, id) {
                    emit(&mut events, id, None, StructuralChange::WindowReordered);
                }
                if old.context_ref != new.context_ref {
                    emit(&mut events, id, None, StructuralChange::ContextChanged);
                }
                if old.selected_page != new.selected_page {
                    emit(&mut events, id, None, StructuralChange::SelectedPageChanged);
                }
            }
            (None, None) => unreachable!("union contains this window"),
        }
    }
    let page_ids: BTreeSet<_> = left
        .pages
        .keys()
        .chain(right.pages.keys())
        .copied()
        .collect();
    for id in page_ids {
        match (left.pages.get(&id), right.pages.get(&id)) {
            (None, Some((window, _, _))) => {
                emit(&mut events, *window, Some(id), StructuralChange::PageAdded);
            }
            (Some((window, _, _)), None) => {
                emit(
                    &mut events,
                    *window,
                    Some(id),
                    StructuralChange::PageRemoved,
                );
            }
            (Some((old_window, _, old)), Some((new_window, _, new))) => {
                if old_window != new_window {
                    emit(
                        &mut events,
                        *new_window,
                        Some(id),
                        StructuralChange::PageMoved,
                    );
                } else if common_page_rank(&left, &right, id) != common_page_rank(&right, &left, id)
                {
                    emit(
                        &mut events,
                        *new_window,
                        Some(id),
                        StructuralChange::PageReordered,
                    );
                }
                let window = *new_window;
                let page = Some(id);
                if old.url != new.url {
                    let change = if old.url.replay_url().is_some() && new.url.replay_url().is_some()
                    {
                        StructuralChange::UrlChanged
                    } else {
                        StructuralChange::UrlAvailabilityChanged
                    };
                    emit_availability(
                        &mut events,
                        window,
                        id,
                        change,
                        url_availability(&old.url),
                        url_availability(&new.url),
                    );
                }
                for (different, change) in [
                    (old.pinned != new.pinned, StructuralChange::PinnedChanged),
                    (old.muted != new.muted, StructuralChange::MutedChanged),
                    (
                        old.zoom_hundredths != new.zoom_hundredths,
                        StructuralChange::ZoomChanged,
                    ),
                ] {
                    if different {
                        emit(&mut events, window, page, change);
                    }
                }
                for (before_coverage, after_coverage, change) in [
                    (
                        old.page_coverage,
                        new.page_coverage,
                        StructuralChange::PageCoverageChanged,
                    ),
                    (
                        old.cookie_coverage,
                        new.cookie_coverage,
                        StructuralChange::CookieCoverageChanged,
                    ),
                ] {
                    if before_coverage != after_coverage {
                        emit_availability(
                            &mut events,
                            window,
                            id,
                            change,
                            FieldAvailability::Coverage(before_coverage.kind()),
                            FieldAvailability::Coverage(after_coverage.kind()),
                        );
                    }
                }
                if old.scroll != new.scroll {
                    let change = if matches!(old.scroll, CaptureField::Unavailable(_))
                        || matches!(new.scroll, CaptureField::Unavailable(_))
                    {
                        StructuralChange::ScrollAvailabilityChanged
                    } else {
                        StructuralChange::ScrollChanged
                    };
                    emit_availability(
                        &mut events,
                        window,
                        id,
                        change,
                        scroll_availability(&old.scroll),
                        scroll_availability(&new.scroll),
                    );
                }
            }
            (None, None) => unreachable!("union contains this page"),
        }
    }
    let events = events
        .into_iter()
        .filter(|event| match kind {
            None => true,
            Some(DiffKind::Browser) => matches!(
                event.change,
                StructuralChange::WindowAdded
                    | StructuralChange::WindowRemoved
                    | StructuralChange::WindowReordered
                    | StructuralChange::ContextChanged
                    | StructuralChange::SelectedPageChanged
                    | StructuralChange::PageAdded
                    | StructuralChange::PageRemoved
                    | StructuralChange::PageMoved
                    | StructuralChange::PageReordered
            ),
            Some(DiffKind::Page) => matches!(
                event.change,
                StructuralChange::UrlChanged
                    | StructuralChange::UrlAvailabilityChanged
                    | StructuralChange::PinnedChanged
                    | StructuralChange::MutedChanged
                    | StructuralChange::ZoomChanged
                    | StructuralChange::ScrollChanged
                    | StructuralChange::ScrollAvailabilityChanged
                    | StructuralChange::PageCoverageChanged
            ),
            Some(DiffKind::Cookies) => event.change == StructuralChange::CookieCoverageChanged,
        })
        .collect::<Vec<_>>();
    let total_events = events.len();
    if offset > total_events {
        return Err(StructuralDiffError::InvalidOffset);
    }
    let end = offset.saturating_add(limit).min(total_events);
    Ok(StructuralDiffPage {
        total_events,
        next_offset: (end < total_events).then_some(end),
        events: events.into_iter().skip(offset).take(limit).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::capture::Coverage;
    use super::*;

    fn page(id: LogicalPageId, url: &str) -> StructuralPage {
        StructuralPage {
            logical_id: id,
            lineage_parent: None,
            url: SafePageUrl::from_observed(Some(url), true),
            pinned: false,
            muted: false,
            zoom_hundredths: 100,
            scroll: CaptureField::Unavailable(CaptureUnavailable::NotObserved),
            page_coverage: Coverage::from_counts(None, None).unwrap(),
            cookie_coverage: Coverage::from_counts(None, None).unwrap(),
        }
    }

    fn snapshot(
        window: LogicalWindowId,
        pages: Vec<StructuralPage>,
        selected: Option<LogicalPageId>,
    ) -> StructuralSnapshot {
        StructuralSnapshot {
            application_revision: 17,
            windows: vec![StructuralWindow {
                logical_id: window,
                context_ref: None,
                selected_page: selected,
                pages,
            }],
        }
    }

    fn changes(before: &StructuralSnapshot, after: &StructuralSnapshot) -> Vec<StructuralChange> {
        diff_structural(before, after, 0, MAX_LIST_ITEMS)
            .unwrap()
            .events
            .into_iter()
            .map(|event| event.change)
            .collect()
    }

    #[test]
    fn equal_urls_do_not_collapse_distinct_logical_pages() {
        let window = LogicalWindowId::fresh();
        let first = LogicalPageId::fresh();
        let second = LogicalPageId::fresh();
        let before = snapshot(
            window,
            vec![page(first, "https://example.test/same")],
            Some(first),
        );
        let after = snapshot(
            window,
            vec![page(second, "https://example.test/same")],
            Some(second),
        );
        let found = changes(&before, &after);
        assert_eq!(found.len(), 3);
        assert!(found.contains(&StructuralChange::SelectedPageChanged));
        assert!(found.contains(&StructuralChange::PageRemoved));
        assert!(found.contains(&StructuralChange::PageAdded));
        assert!(!found.contains(&StructuralChange::UrlChanged));
    }

    #[test]
    fn ordering_selection_and_supported_fields_are_classified() {
        let window = LogicalWindowId::fresh();
        let ids = [LogicalPageId::fresh(), LogicalPageId::fresh()];
        let before = snapshot(
            window,
            vec![
                page(ids[0], "https://example.test/a"),
                page(ids[1], "https://example.test/b"),
            ],
            Some(ids[0]),
        );
        let mut after = snapshot(
            window,
            vec![
                page(ids[1], "https://example.test/b"),
                page(ids[0], "https://example.test/new"),
            ],
            Some(ids[1]),
        );
        after.windows[0].pages[1].pinned = true;
        after.windows[0].pages[1].muted = true;
        after.windows[0].pages[1].zoom_hundredths = 125;
        after.windows[0].pages[1].scroll = CaptureField::Known(ScrollPoint::new(8.0, 3.0).unwrap());
        after.windows[0].pages[1].page_coverage = Coverage::from_counts(Some(1), Some(0)).unwrap();
        after.windows[0].pages[1].cookie_coverage =
            Coverage::from_counts(Some(0), Some(0)).unwrap();
        let forward = changes(&before, &after);
        for expected in [
            StructuralChange::SelectedPageChanged,
            StructuralChange::PageReordered,
            StructuralChange::UrlChanged,
            StructuralChange::PinnedChanged,
            StructuralChange::MutedChanged,
            StructuralChange::ZoomChanged,
            StructuralChange::ScrollAvailabilityChanged,
            StructuralChange::PageCoverageChanged,
            StructuralChange::CookieCoverageChanged,
        ] {
            assert!(forward.contains(&expected), "missing {expected:?}");
        }
        assert_eq!(
            forward
                .iter()
                .filter(|change| **change == StructuralChange::PageReordered)
                .count(),
            2
        );
        let json =
            serde_json::to_string(&diff_structural(&before, &after, 0, 100).unwrap()).unwrap();
        assert!(!json.contains("example.test"));
        let events = diff_structural(&before, &after, 0, 100).unwrap().events;
        let page_coverage = events
            .iter()
            .find(|event| event.change == StructuralChange::PageCoverageChanged)
            .unwrap();
        assert_eq!(
            page_coverage.before_availability,
            Some(FieldAvailability::Coverage(CoverageKind::Unknown))
        );
        assert_eq!(
            page_coverage.after_availability,
            Some(FieldAvailability::Coverage(CoverageKind::Unavailable))
        );
        assert!(json.contains("unknown"));
        assert!(json.contains("unavailable"));
    }

    #[test]
    fn unavailable_is_never_compared_as_absent_or_a_known_value() {
        let window = LogicalWindowId::fresh();
        let id = LogicalPageId::fresh();
        let before = snapshot(window, vec![page(id, "https://example.test/a")], Some(id));
        let mut after = before.clone();
        after.windows[0].pages[0].url = SafePageUrl::placeholder(CaptureUnavailable::NotObserved);
        after.windows[0].pages[0].scroll = CaptureField::Absent;
        let found = changes(&before, &after);
        assert!(found.contains(&StructuralChange::UrlAvailabilityChanged));
        assert!(found.contains(&StructuralChange::ScrollAvailabilityChanged));
        assert!(!found.contains(&StructuralChange::UrlChanged));
        assert!(!found.contains(&StructuralChange::ScrollChanged));
        let events = diff_structural(&before, &after, 0, 100).unwrap().events;
        let url = events
            .iter()
            .find(|event| event.change == StructuralChange::UrlAvailabilityChanged)
            .unwrap();
        assert_eq!(url.before_availability, Some(FieldAvailability::Known));
        assert_eq!(
            url.after_availability,
            Some(FieldAvailability::Unavailable(
                CaptureUnavailable::NotObserved
            ))
        );
        let scroll = events
            .iter()
            .find(|event| event.change == StructuralChange::ScrollAvailabilityChanged)
            .unwrap();
        assert_eq!(
            scroll.before_availability,
            Some(FieldAvailability::Unavailable(
                CaptureUnavailable::NotObserved
            ))
        );
        assert_eq!(scroll.after_availability, Some(FieldAvailability::Absent));
    }

    #[test]
    fn logical_window_order_and_page_moves_ignore_runtime_identity() {
        let windows = [LogicalWindowId::fresh(), LogicalWindowId::fresh()];
        let pages = [LogicalPageId::fresh(), LogicalPageId::fresh()];
        let mut before = snapshot(
            windows[0],
            vec![page(pages[0], "https://example.test/a")],
            Some(pages[0]),
        );
        before.windows.push(StructuralWindow {
            logical_id: windows[1],
            context_ref: None,
            selected_page: Some(pages[1]),
            pages: vec![page(pages[1], "https://example.test/b")],
        });
        let mut after = before.clone();
        after.windows.swap(0, 1);
        let moved = after.windows[1].pages.remove(0);
        after.windows[1].selected_page = None;
        after.windows[0].pages.push(moved);
        let found = changes(&before, &after);
        assert_eq!(
            found
                .iter()
                .filter(|change| **change == StructuralChange::WindowReordered)
                .count(),
            2
        );
        assert_eq!(
            found
                .iter()
                .filter(|change| **change == StructuralChange::PageMoved)
                .count(),
            1
        );
        assert!(found.contains(&StructuralChange::SelectedPageChanged));
        assert_eq!(
            found
                .iter()
                .filter(|change| **change == StructuralChange::PageAdded
                    || **change == StructuralChange::PageRemoved)
                .count(),
            0
        );
        let reverse = changes(&after, &before);
        assert_eq!(
            reverse
                .iter()
                .filter(|change| **change == StructuralChange::PageMoved)
                .count(),
            1
        );
    }

    #[test]
    fn symmetry_classifications_hold_for_small_exhaustive_page_sets() {
        let window = LogicalWindowId::fresh();
        let ids = [
            LogicalPageId::fresh(),
            LogicalPageId::fresh(),
            LogicalPageId::fresh(),
        ];
        let orderings = [
            vec![],
            vec![ids[0]],
            vec![ids[1]],
            vec![ids[0], ids[1]],
            vec![ids[1], ids[0]],
            vec![ids[0], ids[2]],
            vec![ids[2], ids[1], ids[0]],
        ];
        for left_order in &orderings {
            for right_order in &orderings {
                let left = snapshot(
                    window,
                    left_order
                        .iter()
                        .map(|id| page(*id, "https://example.test/same"))
                        .collect(),
                    left_order.first().copied(),
                );
                let right = snapshot(
                    window,
                    right_order
                        .iter()
                        .map(|id| page(*id, "https://example.test/same"))
                        .collect(),
                    right_order.first().copied(),
                );
                let forward = changes(&left, &right);
                let reverse = changes(&right, &left);
                for change in [
                    StructuralChange::PageReordered,
                    StructuralChange::SelectedPageChanged,
                    StructuralChange::PageMoved,
                    StructuralChange::UrlChanged,
                ] {
                    assert_eq!(
                        forward.iter().filter(|item| **item == change).count(),
                        reverse.iter().filter(|item| **item == change).count()
                    );
                }
                assert_eq!(
                    forward
                        .iter()
                        .filter(|item| **item == StructuralChange::PageAdded)
                        .count(),
                    reverse
                        .iter()
                        .filter(|item| **item == StructuralChange::PageRemoved)
                        .count()
                );
                assert_eq!(
                    forward
                        .iter()
                        .filter(|item| **item == StructuralChange::PageRemoved)
                        .count(),
                    reverse
                        .iter()
                        .filter(|item| **item == StructuralChange::PageAdded)
                        .count()
                );
            }
        }
    }

    #[test]
    fn pagination_and_validation_keep_output_bounded() {
        let window = LogicalWindowId::fresh();
        let ids: Vec<_> = (0..MAX_PAGES_PER_WINDOW_CAPTURE)
            .map(|_| LogicalPageId::fresh())
            .collect();
        let before = snapshot(
            window,
            ids.iter()
                .map(|id| page(*id, "https://example.test/a"))
                .collect(),
            Some(ids[0]),
        );
        let mut after = before.clone();
        for page in &mut after.windows[0].pages {
            page.pinned = true;
            page.muted = true;
            page.zoom_hundredths = 110;
        }
        let first = diff_structural(&before, &after, 0, 7).unwrap();
        assert_eq!(first.events.len(), 7);
        assert_eq!(first.next_offset, Some(7));
        let mut count = 0;
        let mut offset = 0;
        loop {
            let result = diff_structural(&before, &after, offset, 7).unwrap();
            count += result.events.len();
            match result.next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }
        assert_eq!(count, first.total_events);
        assert_eq!(
            diff_structural(&before, &after, 0, 0),
            Err(StructuralDiffError::InvalidPageSize)
        );
        let mut invalid = before.clone();
        invalid.windows[0].pages[1].logical_id = ids[0];
        assert_eq!(
            diff_structural(&invalid, &after, 0, 7),
            Err(StructuralDiffError::InvalidSnapshot)
        );
    }

    #[test]
    fn semantic_kind_is_filtered_before_bounded_pagination() {
        let window = LogicalWindowId::fresh();
        let id = LogicalPageId::fresh();
        let before = snapshot(
            window,
            vec![page(id, "https://example.test/before")],
            Some(id),
        );
        let mut after = before.clone();
        after.windows[0].pages[0].url =
            SafePageUrl::from_observed(Some("https://example.test/after"), true);
        after.windows[0].pages[0].pinned = true;
        after.windows[0].pages[0].cookie_coverage =
            Coverage::from_counts(Some(1), Some(0)).expect("valid coverage");

        let browser = diff_structural_kind(&before, &after, Some(DiffKind::Browser), 0, 1)
            .expect("browser page");
        assert_eq!(browser.total_events, 0);
        let page = diff_structural_kind(&before, &after, Some(DiffKind::Page), 0, 1)
            .expect("one bounded page change");
        assert_eq!(page.total_events, 2);
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.next_offset, Some(1));
        let cookies = diff_structural_kind(&before, &after, Some(DiffKind::Cookies), 0, 1)
            .expect("cookie coverage page");
        assert_eq!(cookies.total_events, 1);
        assert_eq!(
            cookies.events[0].change,
            StructuralChange::CookieCoverageChanged
        );
    }
}
