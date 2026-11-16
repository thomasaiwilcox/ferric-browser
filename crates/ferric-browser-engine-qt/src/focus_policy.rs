//! Pure policy for translating page-focus facts into core mode transitions.

use ferric_browser_core::{Mode, Target};

#[derive(Clone, Debug)]
pub(super) struct PageKeyFocus {
    pub target: Target,
    pub focused: bool,
    pub kind: String,
    pub receiver: PageKeyReceiver,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageKeyReceiver {
    pub key: String,
    pub origin: String,
    pub top: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FocusProbeReceipt {
    pub target: FocusProbeTarget,
    pub state: Option<FocusProbeState>,
    pub receiver: Option<PageKeyReceiver>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FocusProbeTarget {
    pub tab_id: String,
    pub generation: String,
    pub document_id: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FocusProbeState {
    pub sequence: i32,
    pub context: i32,
    pub editable: bool,
    pub user_activated: bool,
    pub kind: String,
    pub focused: bool,
}

#[derive(Clone, Debug)]
pub(super) struct FocusObservation {
    pub(super) url: String,
    pub(super) sequence: i32,
    pub(super) epoch: i32,
    pub(super) context: i32,
    pub(super) editable: bool,
    pub(super) user_activated: bool,
    pub(super) page_key: Option<PageKeyFocus>,
    pub(super) frame_document: Option<String>,
}

impl FocusObservation {
    pub(super) fn ends_transient_context(
        &self,
        mode: Mode,
        epoch: i32,
        context: i32,
        editable: bool,
    ) -> bool {
        mode == Mode::Insert
            && self.editable
            && (!editable || (self.epoch == epoch && self.context != context))
    }
}

// Entry is based on aggregate frame facts. Exit is tied to an observed element,
// so an unfocused parent document cannot end editing in a child frame.
pub(super) fn focus_mode_transition(
    mode: Mode,
    focused_editable: bool,
    user_activated: bool,
    site_entry_mode: Option<&str>,
) -> Option<Mode> {
    let should_insert = (focused_editable && user_activated) || site_entry_mode == Some("insert");
    match (mode, should_insert) {
        (Mode::Normal, true) => Some(Mode::Insert),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_entry_only_changes_normal_mode() {
        assert_eq!(
            focus_mode_transition(Mode::Normal, true, true, None),
            Some(Mode::Insert)
        );
        assert_eq!(focus_mode_transition(Mode::Normal, true, false, None), None);
        assert_eq!(
            focus_mode_transition(Mode::Normal, false, false, Some("insert")),
            Some(Mode::Insert)
        );
        assert_eq!(
            focus_mode_transition(Mode::Insert, false, false, None),
            None
        );
        assert_eq!(focus_mode_transition(Mode::Caret, true, true, None), None);
    }
}

#[cfg(test)]
mod insert_tests {
    use super::*;

    #[test]
    fn transient_context_tracks_the_element_while_locked_tracks_the_session() {
        let observation = FocusObservation {
            url: "https://example.test".into(),
            sequence: 1,
            epoch: 7,
            context: 42,
            editable: true,
            user_activated: true,
            page_key: None,
            frame_document: None,
        };
        assert!(!observation.ends_transient_context(Mode::Insert, 7, 42, true));
        assert!(observation.ends_transient_context(Mode::Insert, 7, 43, true));
        assert!(observation.ends_transient_context(Mode::Insert, 7, 0, false));
        assert!(!observation.ends_transient_context(Mode::InsertLocked, 7, 43, true));
        assert!(!observation.ends_transient_context(Mode::Insert, 8, 43, true));
        assert!(observation.ends_transient_context(Mode::Insert, 8, 0, false));
        assert!(!observation.ends_transient_context(Mode::Normal, 7, 0, false));
    }

    #[test]
    fn locked_insert_survives_editable_focus_changes_and_blur() {
        for editable in [false, true] {
            for activated in [false, true] {
                assert_eq!(
                    focus_mode_transition(Mode::InsertLocked, editable, activated, None),
                    None
                );
            }
        }
        assert_eq!(focus_mode_transition(Mode::Insert, true, true, None), None);
        assert_eq!(
            focus_mode_transition(Mode::Insert, false, false, None),
            None
        );
    }
}
