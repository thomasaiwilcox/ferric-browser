//! Pure policy for translating page-focus facts into core mode transitions.

use ferric_browser_core::Mode;

#[derive(Clone, Debug)]
pub(super) struct FocusObservation {
    pub(super) url: String,
    pub(super) sequence: i32,
    pub(super) editable: bool,
    pub(super) user_activated: bool,
}

pub(super) fn focus_mode_transition(
    mode: Mode,
    focused_editable: bool,
    user_activated: bool,
    site_entry_mode: Option<&str>,
) -> Option<Mode> {
    let should_insert = (focused_editable && user_activated) || site_entry_mode == Some("insert");
    match (mode, should_insert) {
        (Mode::Normal, true) => Some(Mode::Insert),
        (Mode::Insert, false) => Some(Mode::Normal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changes_normal_and_insert_modes() {
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
            Some(Mode::Normal)
        );
        assert_eq!(focus_mode_transition(Mode::Caret, true, true, None), None);
    }
}
