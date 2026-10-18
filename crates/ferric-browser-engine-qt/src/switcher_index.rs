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

impl SwitcherLibraryCandidate {
    pub(super) fn matches_scope(&self, scope: &str) -> bool {
        match self.kind.as_str() {
            "history" => matches!(scope, "all" | "history"),
            "bookmark" | "quickmark" => matches!(scope, "all" | "marks"),
            "download" => matches!(scope, "all" | "downloads"),
            _ => false,
        }
    }
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
    fn indexed_sources_do_not_escape_the_requested_scope() {
        use ferric_browser_storage::{
            BookmarkRecord, DownloadRecord, DownloadState, HistoryRecord,
        };

        let snapshot = ProfileLibrarySnapshot {
            history: vec![HistoryRecord {
                id: 1,
                url: "https://example.test/page".into(),
                title: "Page".into(),
                visit_count: 1,
                last_visit: 1,
            }],
            command_history: Vec::new(),
            bookmarks: vec![BookmarkRecord {
                id: "bookmark".into(),
                url: "https://example.test/mark".into(),
                title: "Bookmark".into(),
                updated_at: 1,
            }],
            quickmarks: vec![Quickmark {
                name: "docs".into(),
                url: "https://example.test/docs".into(),
            }],
            downloads: vec![DownloadRecord {
                id: "download".into(),
                source_url: "https://example.test/file".into(),
                destination: "/owned-fixture/file".into(),
                state: DownloadState::Completed,
                bytes_received: 1,
                created_at: 1,
                completed_at: Some(1),
            }],
            permissions: Vec::new(),
            permissions_complete: true,
        };
        let index = build_switcher_library_index(&snapshot, "work".into(), &AtomicBool::new(false))
            .expect("uncancelled index builds");
        for (scope, expected) in [
            ("all", vec!["history", "bookmark", "quickmark", "download"]),
            ("history", vec!["history"]),
            ("marks", vec!["bookmark", "quickmark"]),
            ("downloads", vec!["download"]),
        ] {
            let actual: Vec<_> = index
                .candidates
                .iter()
                .filter(|candidate| candidate.matches_scope(scope))
                .map(|candidate| candidate.kind.as_str())
                .collect();
            assert_eq!(actual, expected, "scope {scope}");
        }
        for scope in [
            "tabs", "windows", "contexts", "commands", "sessions", "closed", "actions", "unknown",
        ] {
            assert!(
                !index
                    .candidates
                    .iter()
                    .any(|candidate| candidate.matches_scope(scope)),
                "library entry escaped into {scope}"
            );
        }
    }

    #[test]
    fn index_is_profile_scoped_and_cancellation_aware() {
        let snapshot = ProfileLibrarySnapshot {
            history: Vec::new(),
            command_history: Vec::new(),
            bookmarks: Vec::new(),
            quickmarks: vec![Quickmark {
                name: "docs".into(),
                url: "https://example.test/docs".into(),
            }],
            downloads: Vec::new(),
            permissions: Vec::new(),
            permissions_complete: true,
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
