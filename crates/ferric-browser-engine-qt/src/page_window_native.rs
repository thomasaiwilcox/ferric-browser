//! Mapping-time identity read from the actual Qt window, never presentation data.
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

/// Actual mapping-time host witness, including platforms without xdg tagging.
/// It is never deserialized or supplied by QML/document data.
#[derive(Clone)]
pub(crate) struct NativePageHost {
    pub(crate) tag: String,
    generation: Option<u64>,
    last_generation: u64,
}

impl NativePageHost {
    pub(crate) fn capture(ui: &crate::qobject::BrowserUi) -> Option<Self> {
        let tag = crate::qobject::ferric_browser_page_window_marker(ui).to_string();
        let uuid = crate::Uuid::parse_str(tag.strip_prefix("ferric-page-")?).ok()?;
        let snapshot = crate::qobject::ferric_browser_native_page_window_identity(ui);
        if uuid.is_nil() || tag != format!("ferric-page-{uuid}") {
            return None;
        }
        let authority = authority().lock().ok()?;
        Some(Self {
            last_generation: authority.0.get(&tag).map_or(0, |state| state.last),
            tag,
            generation: snapshot.valid.then_some(snapshot.generation),
        })
    }
    pub(crate) fn current(&self) -> bool {
        authority()
            .lock()
            .is_ok_and(|authority| self.matches(&authority, false))
    }
    /// An admitted hidden target may create exactly its first successor role
    /// during `show()`. The predecessor is recorded in the synchronous native
    /// callback, so reentrant creation/destruction cannot hide an intervening role.
    pub(crate) fn current_after_show(&self) -> bool {
        authority()
            .lock()
            .is_ok_and(|authority| self.matches(&authority, true))
    }
    fn matches(&self, authority: &Authority, after_show: bool) -> bool {
        if let Some(generation) = self.generation {
            return authority.current(&self.tag, generation, false);
        }
        let state = authority.0.get(&self.tag).copied().unwrap_or_default();
        if state.current == 0 {
            state.last == self.last_generation
        } else {
            after_show && state.predecessor == self.last_generation
        }
    }
    /// Capture freshness when a new focus lease is admitted. The directory's
    /// logical marker can outlive a hidden or recreated native role.
    pub(crate) fn current_snapshot(&self) -> Option<Self> {
        let authority = authority().lock().ok()?;
        let state = authority.0.get(&self.tag).copied().unwrap_or_default();
        Some(Self {
            tag: self.tag.clone(),
            generation: (state.current != 0).then_some(state.current),
            last_generation: state.last,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeWindowWitness {
    pub(crate) identity: NativeWindowIdentity,
    pub(crate) generation: u64,
}

impl NativeWindowWitness {
    pub(crate) fn current(&self) -> bool {
        native_current(&self.identity.tag, self.generation, true)
    }
}

#[derive(Clone, Copy, Default)]
struct NativeState {
    current: u64,
    last: u64,
    predecessor: u64,
    submitted: bool,
}
#[derive(Default)]
struct Authority(BTreeMap<String, NativeState>);
impl Authority {
    fn current(&self, tag: &str, generation: u64, require_tag: bool) -> bool {
        generation != 0
            && self.0.get(tag).is_some_and(|state| {
                state.current == generation && (!require_tag || state.submitted)
            })
    }
    fn changed(&mut self, tag: &str, generation: u64, submitted: bool) {
        if generation == 0 {
            return;
        }
        let state = self.0.entry(tag.into()).or_default();
        if state.current == generation {
            state.submitted = submitted;
        } else if generation > state.last {
            state.predecessor = state.last;
            state.current = generation;
            state.last = generation;
            state.submitted = submitted;
        }
    }
    fn invalidate(&mut self, tag: &str, generation: u64) {
        if generation != 0
            && let Some(state) = self.0.get_mut(tag)
            && state.current == generation
        {
            state.current = 0;
            state.submitted = false;
        }
    }
}
fn authority() -> &'static Mutex<Authority> {
    static AUTHORITY: OnceLock<Mutex<Authority>> = OnceLock::new();
    AUTHORITY.get_or_init(Default::default)
}
fn native_current(tag: &str, generation: u64, require_tag: bool) -> bool {
    authority()
        .lock()
        .is_ok_and(|authority| authority.current(tag, generation, require_tag))
}
pub(crate) fn native_window_changed(tag: &crate::QString, generation: u64, tag_submitted: bool) {
    if generation != 0
        && let Ok(mut authority) = authority().lock()
    {
        authority.changed(&tag.to_string(), generation, tag_submitted);
    }
}
pub(crate) fn native_window_invalidated(tag: &crate::QString, generation: u64) {
    let tag = tag.to_string();
    if let Ok(mut authority) = authority().lock() {
        authority.invalidate(&tag, generation);
    }
    crate::page_group_runtime::invalidate_native(&tag, generation);
}

/// Logical retirement releases the bounded history as well as native eligibility.
pub(crate) fn native_window_retired(tag: &crate::QString) {
    let tag = tag.to_string();
    let retired = authority()
        .lock()
        .ok()
        .and_then(|mut authority| authority.0.remove(&tag));
    if let Some(state) = retired {
        crate::page_group_runtime::invalidate_native(&tag, state.last);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct NativeWindowIdentity {
    pub(super) tag: String,
    pub(super) app_id: String,
    pub(super) pid: u32,
}

impl NativeWindowIdentity {
    pub(super) fn capture(ui: &crate::qobject::BrowserUi) -> Option<Self> {
        NativeWindowPresentation::capture(ui).map(|presentation| presentation.identity)
    }

    pub(super) fn checked(tag: &str, app_id: &str, pid: u32) -> Option<Self> {
        let uuid = crate::Uuid::parse_str(tag.strip_prefix("ferric-page-")?).ok()?;
        if uuid.is_nil()
            || tag.len() != 48
            || pid == 0
            || app_id.is_empty()
            || app_id.len() > 256
            || !app_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return None;
        }
        Some(Self {
            tag: tag.into(),
            app_id: app_id.into(),
            pid,
        })
    }
}

pub(crate) struct NativeWindowPresentation {
    pub(crate) identity: NativeWindowIdentity,
    pub(crate) generation: u64,
    pub(crate) foreground: bool,
    pub(crate) active: bool,
}

impl NativeWindowPresentation {
    pub(crate) fn capture(ui: &crate::qobject::BrowserUi) -> Option<Self> {
        let snapshot = crate::qobject::ferric_browser_native_page_window_identity(ui);
        if !snapshot.valid || !snapshot.tag_submitted || snapshot.generation == 0 {
            return None;
        }
        Some(Self {
            identity: NativeWindowIdentity::checked(
                &snapshot.tag,
                &snapshot.app_id,
                std::process::id(),
            )?,
            generation: snapshot.generation,
            foreground: snapshot.foreground,
            active: snapshot.active,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_focus_target_rejects_intervening_and_reentrant_roles() {
        let marker = NativePageHost {
            tag: "target".into(),
            generation: None,
            last_generation: 0,
        };
        let mut authority = Authority::default();
        assert!(marker.matches(&authority, false));
        authority.changed("target", 7, false);
        assert!(!marker.matches(&authority, false));
        assert!(marker.matches(&authority, true));
        authority.invalidate("target", 7);
        // A role appearing and disappearing before permission use is visible
        // even though the target has no role again.
        assert!(!marker.matches(&authority, false));
        assert!(!marker.matches(&authority, true));
        authority.changed("target", 8, false);
        // This also fails if both transitions happened inside show(), before
        // the first phase-2 validation had a chance to observe generation 7.
        assert!(!marker.matches(&authority, true));
        assert!(
            NativePageHost {
                tag: "target".into(),
                generation: Some(8),
                last_generation: 8
            }
            .matches(&authority, false)
        );
        // Retirement releases all per-logical-window native history.
        authority.0.remove("target");
        assert!(authority.0.is_empty());
    }
    #[test]
    fn stale_participant_and_unsubmitted_tag_cannot_retain_authority() {
        let mut authority = Authority::default();
        authority.changed("source", 10, true);
        authority.changed("participant", 20, true);
        let admitted = [("source", 10), ("participant", 20)];
        assert!(
            admitted
                .iter()
                .all(|&(tag, generation)| authority.current(tag, generation, true))
        );
        authority.invalidate("participant", 20);
        assert!(
            !admitted
                .iter()
                .all(|&(tag, generation)| authority.current(tag, generation, true))
        );
        authority.changed("participant", 21, true);
        assert!(
            !admitted
                .iter()
                .all(|&(tag, generation)| authority.current(tag, generation, true))
        );
        authority.invalidate("participant", 20);
        assert!(authority.current("participant", 21, true));
        authority.changed("participant", 21, false);
        assert!(!authority.current("participant", 21, true));
        assert!(authority.current("participant", 21, false));
        assert!(!authority.current("source", 0, false));
    }
    #[test]
    fn only_bounded_native_marker_and_desktop_id_shapes_are_admitted() {
        let tag = format!("ferric-page-{}", crate::Uuid::new_v4());
        assert!(
            NativeWindowIdentity::checked(&tag, "io.github.ferricbrowser.FerricBrowser", 7)
                .is_some()
        );
        assert!(NativeWindowIdentity::checked(&tag, "application_tests", 7).is_some());
        for invalid in [
            "",
            "page-title",
            "ferric-page-00000000-0000-0000-0000-000000000000",
            "ferric-page-7",
        ] {
            assert!(NativeWindowIdentity::checked(invalid, "application_tests", 7).is_none());
        }
        assert!(NativeWindowIdentity::checked(&tag, "bad\nidentity", 7).is_none());
        assert!(NativeWindowIdentity::checked(&tag, "app; execute()", 7).is_none());
        assert!(NativeWindowIdentity::checked(&tag, "app", 0).is_none());
    }
}
