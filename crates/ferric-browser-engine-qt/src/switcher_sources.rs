//! Readiness of the saved catalogs a Switcher query actually needs.
//!
//! The storage coordinator retains request scheduling. This state describes
//! admission/completion for the current profile, not a second data cache.

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum CatalogReadState {
    #[default]
    Unrequested,
    Queued,
    Pending,
    Ready,
    Failed(String),
}

impl CatalogReadState {
    pub(super) const fn is_in_flight(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub(super) const fn is_pending(&self) -> bool {
        matches!(self, Self::Queued | Self::Pending)
    }

    /// A failed read waits for a deliberate refresh or a successful mutation's
    /// normal refresh path. Typing does not continually retry failed I/O.
    pub(super) const fn permits_query_request(&self) -> bool {
        matches!(self, Self::Unrequested | Self::Queued | Self::Ready)
    }

    fn detail(&self) -> Option<&str> {
        if let Self::Failed(detail) = self {
            Some(detail)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceProfile {
    Opening,
    Durable,
    Private,
    Ephemeral,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SourceNeeds {
    pub(super) library: bool,
    pub(super) sessions: bool,
}

impl SourceNeeds {
    pub(super) fn for_scope(scope: &str) -> Option<Self> {
        Some(match scope {
            "all" | "" => Self {
                library: true,
                sessions: true,
            },
            "history" | "marks" | "downloads" => Self {
                library: true,
                sessions: false,
            },
            "sessions" => Self {
                library: false,
                sessions: true,
            },
            "tabs" | "windows" | "contexts" | "commands" | "closed" | "actions" => Self::default(),
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SwitcherAvailability {
    pub(super) pending: bool,
    pub(super) failed: bool,
    pub(super) retryable: bool,
    pub(super) notice: String,
    pub(super) detail: String,
}

impl SwitcherAvailability {
    pub(super) fn for_sources(
        scope: &str,
        profile: SourceProfile,
        library: &CatalogReadState,
        sessions: &CatalogReadState,
    ) -> Option<Self> {
        let needs = SourceNeeds::for_scope(scope)?;
        if profile == SourceProfile::Opening {
            return Some(Self {
                pending: true,
                notice: "Opening the profile…".into(),
                ..Self::default()
            });
        }
        if !needs.library && !needs.sessions {
            return Some(Self::default());
        }
        if matches!(profile, SourceProfile::Private | SourceProfile::Ephemeral) {
            let session = if profile == SourceProfile::Private {
                "private"
            } else {
                "temporary"
            };
            let notice = match scope {
                "marks" => {
                    format!("Bookmarks and quickmarks aren't available in this {session} session.")
                }
                "sessions" => format!("Saved sessions aren't available in this {session} session."),
                "downloads" => format!(
                    "Download history isn't saved in {session} sessions. Use :downloads for current transfers."
                ),
                _ => String::new(), // In-memory history and eligible live items remain searchable.
            };
            return Some(Self {
                notice,
                ..Self::default()
            });
        }
        if profile == SourceProfile::Unavailable {
            return Some(Self {
                failed: true,
                notice:
                    "Saved items aren't available in this window. Reopen the profile to try again."
                        .into(),
                ..Self::default()
            });
        }
        let mut result = Self::default();
        let mut details = Vec::new();
        for (wanted, state, label) in [
            (needs.library, library, "Saved library"),
            (needs.sessions, sessions, "Saved sessions"),
        ] {
            if !wanted {
                continue;
            }
            result.pending |= matches!(state, CatalogReadState::Unrequested) || state.is_pending();
            if let Some(detail) = state.detail() {
                result.failed = true;
                details.push(format!("{label}: {detail}"));
            }
        }
        result.retryable = result.failed && !result.pending;
        result.detail = details.join("\n\n");
        result.notice = match (result.pending, result.failed) {
            (true, true) => "Some saved items couldn't be loaded. Other items are still loading.",
            (true, false) => match scope {
                "history" => "Loading history…",
                "marks" => "Loading bookmarks and quickmarks…",
                "downloads" => "Loading download history…",
                "sessions" => "Loading saved sessions…",
                _ => "Loading saved items…",
            },
            (false, true) => "Some saved items couldn't be loaded. Refresh to try again.",
            (false, false) => "",
        }
        .into();
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CatalogReadState as Read, SourceNeeds, SourceProfile as Profile,
        SwitcherAvailability as Status,
    };

    #[test]
    fn live_scopes_are_independent_of_saved_catalogs() {
        for scope in [
            "tabs", "windows", "contexts", "commands", "closed", "actions",
        ] {
            let status = Status::for_sources(
                scope,
                Profile::Durable,
                &Read::Pending,
                &Read::Failed("saved sessions unavailable".into()),
            )
            .unwrap();
            assert_eq!(status, Status::default(), "{scope}");
            assert_eq!(SourceNeeds::for_scope(scope), Some(SourceNeeds::default()));
        }
        assert!(SourceNeeds::for_scope("unknown").is_none());
    }

    #[test]
    fn no_saved_search_concludes_empty_before_its_read_finishes() {
        for scope in ["all", "history", "marks", "downloads", "sessions"] {
            for state in [Read::Unrequested, Read::Queued, Read::Pending] {
                let status = Status::for_sources(scope, Profile::Durable, &state, &state).unwrap();
                assert!(
                    status.pending && !status.failed && !status.retryable,
                    "{scope}: {state:?}"
                );
                assert_ne!(status.notice, "");
            }
            let status =
                Status::for_sources(scope, Profile::Durable, &Read::Ready, &Read::Ready).unwrap();
            assert_eq!(status, Status::default(), "{scope}");
        }
    }

    #[test]
    fn only_relevant_failures_and_pending_reads_affect_a_scope() {
        let failed = Read::Failed("owned catalog error <b>literal</b>".into());
        let marks = Status::for_sources("marks", Profile::Durable, &Read::Ready, &failed).unwrap();
        assert_eq!(marks, Status::default());
        let sessions =
            Status::for_sources("sessions", Profile::Durable, &Read::Pending, &failed).unwrap();
        assert!(sessions.failed && sessions.retryable && !sessions.pending);
        assert!(sessions.detail.contains("<b>literal</b>"));
        assert!(!sessions.notice.contains("<b>literal</b>"));
    }

    #[test]
    fn partial_failures_do_not_hide_another_pending_source() {
        let status = Status::for_sources(
            "all",
            Profile::Durable,
            &Read::Failed("library error".into()),
            &Read::Pending,
        )
        .unwrap();
        assert!(status.pending && status.failed && !status.retryable);
        assert!(status.notice.contains("still loading"));
        assert!(status.detail.contains("library error"));
        let complete = Status::for_sources(
            "all",
            Profile::Durable,
            &Read::Failed("library error".into()),
            &Read::Ready,
        )
        .unwrap();
        assert!(complete.failed && complete.retryable && !complete.pending);
    }

    #[test]
    fn transient_windows_do_not_wait_for_or_reveal_durable_failures() {
        let failed = Read::Failed("regular profile secret path".into());
        for profile in [Profile::Private, Profile::Ephemeral] {
            for scope in ["all", "history", "tabs", "marks", "sessions", "downloads"] {
                let status = Status::for_sources(scope, profile, &failed, &Read::Pending).unwrap();
                assert!(
                    !status.pending && !status.failed && !status.retryable,
                    "{scope}"
                );
                assert_eq!(status.detail, "");
                if matches!(scope, "marks" | "sessions" | "downloads") {
                    assert_ne!(status.notice, "");
                }
            }
        }
    }

    #[test]
    fn opening_and_unavailable_profiles_are_not_empty_catalogs() {
        let opening = Status::for_sources(
            "marks",
            Profile::Opening,
            &Read::Unrequested,
            &Read::Unrequested,
        )
        .unwrap();
        assert!(opening.pending && !opening.failed);
        let unavailable = Status::for_sources(
            "marks",
            Profile::Unavailable,
            &Read::Unrequested,
            &Read::Unrequested,
        )
        .unwrap();
        assert!(unavailable.failed && !unavailable.retryable && !unavailable.pending);
        let tabs = Status::for_sources(
            "tabs",
            Profile::Unavailable,
            &Read::Unrequested,
            &Read::Unrequested,
        )
        .unwrap();
        assert_eq!(tabs, Status::default());
    }

    #[test]
    fn admission_completion_and_retry_remain_distinct() {
        assert!(Read::Unrequested.permits_query_request());
        assert!(Read::Queued.is_pending() && !Read::Queued.is_in_flight());
        assert!(Read::Queued.permits_query_request());
        assert!(Read::Pending.is_in_flight() && !Read::Pending.permits_query_request());
        let failed = Read::Failed("read failed".into());
        assert!(!failed.is_pending() && !failed.permits_query_request());
        assert!(Read::Ready.permits_query_request());
    }
}
