//! Storage-backed candidates for the command switcher.
//!
//! Building this index deliberately depends only on a storage snapshot and a
//! cancellation flag, so it remains independent of Qt and the browser bridge.

use ferric_browser_storage::ProfileLibrarySnapshot;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug)]
pub(super) struct SwitcherLibraryCandidate {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) label: String,
    pub(super) secondary: String,
    pub(super) recency: i64,
    pub(super) fields: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SwitcherLibraryIndex {
    pub(super) profile_name: String,
    pub(super) candidates: Vec<SwitcherLibraryCandidate>,
}

#[derive(Clone, Debug)]
pub(super) struct SwitcherLibraryIndexResult {
    pub(super) revision: i64,
    pub(super) index: Option<SwitcherLibraryIndex>,
}

pub(super) fn build_switcher_library_index(
    snapshot: &ProfileLibrarySnapshot,
    profile_name: String,
    cancelled: &AtomicBool,
) -> Option<SwitcherLibraryIndex> {
    let mut candidates = Vec::with_capacity(
        snapshot.history.len()
            + snapshot.bookmarks.len()
            + snapshot.quickmarks.len()
            + snapshot.downloads.len(),
    );
    let mut add = |kind: &str,
                   id: String,
                   label: String,
                   secondary: String,
                   recency: i64,
                   fields: Vec<String>| {
        candidates.push(SwitcherLibraryCandidate {
            kind: kind.to_owned(),
            id,
            label,
            secondary,
            recency,
            fields,
        });
    };
    for page in &snapshot.history {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        add(
            "history",
            page.id.to_string(),
            if page.title.is_empty() {
                page.url.clone()
            } else {
                page.title.clone()
            },
            page.url.clone(),
            page.last_visit,
            vec![page.title.clone(), page.url.clone()],
        );
    }
    for mark in &snapshot.bookmarks {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        add(
            "bookmark",
            mark.id.clone(),
            if mark.title.is_empty() {
                mark.url.clone()
            } else {
                mark.title.clone()
            },
            mark.url.clone(),
            mark.updated_at,
            vec![mark.title.clone(), mark.url.clone()],
        );
    }
    for mark in &snapshot.quickmarks {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        add(
            "quickmark",
            mark.name.clone(),
            mark.name.clone(),
            mark.url.clone(),
            0,
            vec![mark.name.clone(), mark.url.clone()],
        );
    }
    for download in &snapshot.downloads {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        add(
            "download",
            download.id.clone(),
            download.destination.clone(),
            download.source_url.clone(),
            download.created_at,
            vec![download.destination.clone(), download.source_url.clone()],
        );
    }
    Some(SwitcherLibraryIndex {
        profile_name,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::build_switcher_library_index;
    use ferric_browser_storage::{ProfileLibrarySnapshot, Quickmark};
    use std::sync::atomic::AtomicBool;

    #[test]
    fn index_is_profile_scoped_and_cancellation_aware() {
        let snapshot = ProfileLibrarySnapshot {
            history: Vec::new(),
            bookmarks: Vec::new(),
            quickmarks: vec![Quickmark {
                name: "docs".into(),
                url: "https://example.test/docs".into(),
            }],
            downloads: Vec::new(),
            permissions: Vec::new(),
        };
        let index = build_switcher_library_index(&snapshot, "work".into(), &AtomicBool::new(false))
            .expect("uncancelled index builds");
        assert_eq!(index.profile_name, "work");
        assert_eq!(index.candidates.len(), 1);
        assert_eq!(index.candidates[0].kind, "quickmark");
        assert_eq!(index.candidates[0].fields[1], "https://example.test/docs");
        assert!(
            build_switcher_library_index(&snapshot, "work".into(), &AtomicBool::new(true))
                .is_none()
        );
    }
}
