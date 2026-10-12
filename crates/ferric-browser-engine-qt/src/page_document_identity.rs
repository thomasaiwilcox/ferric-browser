//! Short-lived `ApplicationWorld` document identities used for same-document
//! navigation qualification. This registry is keyed by the owning tab and is
//! cleared whenever that `WebEngine` document can be replaced.

use super::{Mode, TabId};
use std::collections::BTreeMap;

const MAX_DOCUMENT_ID_BYTES: usize = 128;

pub(super) const fn can_suspend_user_pass_through(mode: Mode, same_document_proven: bool) -> bool {
    matches!(mode, Mode::PassThrough) && same_document_proven
}

#[derive(Default)]
pub(super) struct PageDocumentIdentities {
    by_tab: BTreeMap<TabId, String>,
}

impl PageDocumentIdentities {
    pub(super) fn establish(&mut self, tab: TabId, document_id: &str) -> bool {
        if !valid_document_id(document_id) {
            return false;
        }
        self.by_tab.insert(tab, document_id.to_owned());
        true
    }

    pub(super) fn proves(&self, tab: TabId, document_id: &str) -> bool {
        valid_document_id(document_id)
            && self
                .by_tab
                .get(&tab)
                .is_some_and(|known| known == document_id)
    }

    pub(super) fn get(&self, tab: TabId) -> Option<&str> {
        self.by_tab.get(&tab).map(String::as_str)
    }

    pub(super) fn invalidate(&mut self, tab: TabId) {
        self.by_tab.remove(&tab);
    }

    pub(super) fn clear(&mut self) {
        self.by_tab.clear();
    }
}

#[derive(Clone, Debug)]
pub(super) struct PendingSameDocumentPassThrough {
    pub(super) tab: TabId,
    pub(super) document_id: String,
    pub(super) url: String,
    pub(super) event_input_epoch: i32,
    pub(super) suspended_input_epoch: i32,
}

fn valid_document_id(document_id: &str) -> bool {
    !document_id.is_empty()
        && document_id.len() <= MAX_DOCUMENT_ID_BYTES
        && !document_id.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_proof_is_exact_and_scoped_to_one_tab() {
        let first_tab = TabId::from_display("tabid-1").unwrap();
        let second_tab = TabId::from_display("tabid-2").unwrap();
        let mut identities = PageDocumentIdentities::default();

        assert!(identities.establish(first_tab, "document-one"));
        assert!(identities.proves(first_tab, "document-one"));
        assert!(!identities.proves(first_tab, "document-two"));
        assert!(!identities.proves(second_tab, "document-one"));

        identities.invalidate(first_tab);
        assert!(!identities.proves(first_tab, "document-one"));
    }

    #[test]
    fn document_id_input_is_bounded_and_rejects_controls() {
        let tab = TabId::from_display("tabid-1").unwrap();
        let mut identities = PageDocumentIdentities::default();

        assert!(!identities.establish(tab, ""));
        assert!(!identities.establish(tab, "bad\nidentity"));
        assert!(!identities.establish(tab, &"x".repeat(MAX_DOCUMENT_ID_BYTES + 1)));
        assert!(identities.establish(tab, "document-one"));
        assert!(identities.proves(tab, "document-one"));
        identities.clear();
        assert!(!identities.proves(tab, &"x".repeat(MAX_DOCUMENT_ID_BYTES + 1)));
    }

    #[test]
    fn only_proven_persistent_pass_through_can_be_restored() {
        assert!(can_suspend_user_pass_through(Mode::PassThrough, true));
        assert!(!can_suspend_user_pass_through(Mode::PassThrough, false));
        assert!(!can_suspend_user_pass_through(Mode::PassThroughOnce, true));
        assert!(!can_suspend_user_pass_through(Mode::Normal, true));
    }
}
