//! Short-lived owner-bound leases for observable Qt/native prompt lifecycles.
//!
//! This module stores no durable policy. QML reports prompt entry and completion
//! from real surface lifecycle signals; the lease only decides whether a saved
//! mode can safely return after the prompt releases input.

use ferric_browser_core::Mode;

pub(super) const MAX_NATIVE_PROMPT_DEPTH: usize = 8;

const PROMPT_CATEGORIES: &[&str] = &[
    "file-chooser",
    "permission",
    "page-dialog",
    "authentication",
    "client-certificate",
    "certificate-error",
    "webauthn",
    "context-menu",
    "qt-menu",
    "desktop-media",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PromptOwner {
    pub(super) window: Option<String>,
    pub(super) tab: String,
    pub(super) view: Option<String>,
    pub(super) document: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PromptFocusIdentity {
    pub(super) frame_path: String,
    pub(super) context: i32,
}

#[derive(Clone, Debug)]
pub(super) struct NativePromptLease {
    pub(super) id: u64,
    pub(super) category: &'static str,
    pub(super) owner: PromptOwner,
    pub(super) saved_mode: Option<Mode>,
    pub(super) expected_input_epoch: i32,
    pub(super) config_revision: u64,
    pub(super) policy_revision: u64,
    pub(super) focus_identity: Option<PromptFocusIdentity>,
    pub(super) completion_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Completion {
    Stale,
    OutOfOrder,
    Nested,
    AwaitFocus,
    Final,
}

#[derive(Debug)]
pub(super) struct NativePromptLeases {
    entries: Vec<NativePromptLease>,
    next_id: u64,
    last_result: &'static str,
}

impl Default for NativePromptLeases {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            last_result: "none",
        }
    }
}

impl NativePromptLeases {
    pub(super) fn depth(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn is_active(&self) -> bool {
        !self.entries.is_empty()
    }

    pub(super) fn active_category(&self) -> Option<&'static str> {
        self.entries.last().map(|entry| entry.category)
    }

    pub(super) fn top(&self) -> Option<&NativePromptLease> {
        self.entries.last()
    }

    pub(super) fn entry(&self, id: u64) -> Option<&NativePromptLease> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub(super) fn push(
        &mut self,
        category: &'static str,
        owner: PromptOwner,
        saved_mode: Option<Mode>,
        expected_input_epoch: i32,
        config_revision: u64,
        policy_revision: u64,
        focus_identity: Option<PromptFocusIdentity>,
    ) -> Option<u64> {
        if self.entries.len() >= MAX_NATIVE_PROMPT_DEPTH {
            self.clear("depth-limit");
            return None;
        }
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1).max(1);
        self.entries.push(NativePromptLease {
            id,
            category,
            owner,
            saved_mode,
            expected_input_epoch,
            config_revision,
            policy_revision,
            focus_identity,
            completion_pending: false,
        });
        self.last_result = "active";
        Some(id)
    }

    pub(super) fn begin_completion(&mut self, id: u64) -> Completion {
        let Some(position) = self.entries.iter().position(|entry| entry.id == id) else {
            return Completion::Stale;
        };
        if position + 1 != self.entries.len() {
            self.clear("out-of-order");
            return Completion::OutOfOrder;
        }
        if self.entries[position]
            .saved_mode
            .is_some_and(|mode| matches!(mode, Mode::Insert | Mode::InsertLocked))
        {
            self.entries[position].completion_pending = true;
            self.last_result = "awaiting-focus";
            return Completion::AwaitFocus;
        }
        self.entries.pop();
        if self.entries.is_empty() {
            self.last_result = "completed";
            Completion::Final
        } else {
            Completion::Nested
        }
    }

    pub(super) fn finish_pending(&mut self, id: u64) -> Option<NativePromptLease> {
        let top = self.entries.last()?;
        if top.id != id || !top.completion_pending {
            return None;
        }
        let lease = self.entries.pop()?;
        self.last_result = "focus-checked";
        Some(lease)
    }

    pub(super) fn clear(&mut self, reason: &'static str) {
        self.entries.clear();
        self.last_result = reason;
    }

    pub(super) fn set_last_result(&mut self, result: &'static str) {
        self.last_result = result;
    }

    pub(super) fn last_result(&self) -> &'static str {
        self.last_result
    }
}

pub(super) fn prompt_category(value: &str) -> Option<&'static str> {
    PROMPT_CATEGORIES
        .iter()
        .copied()
        .find(|category| *category == value)
}

pub(super) fn restorable_mode(mode: Mode, owner: &PromptOwner) -> Option<Mode> {
    if owner.window.is_none() || owner.tab.is_empty() || owner.view.is_none() {
        return None;
    }
    match mode {
        Mode::PassThrough if owner.document.is_some() => Some(mode),
        Mode::Insert | Mode::InsertLocked if owner.document.is_some() => Some(mode),
        _ => None,
    }
}

pub(super) fn owner_matches(saved: &PromptOwner, current: &PromptOwner) -> bool {
    saved == current
}

pub(super) fn suspension_matches(
    lease: &NativePromptLease,
    input_epoch: i32,
    config_revision: u64,
    policy_revision: u64,
    mode: Mode,
) -> bool {
    lease.expected_input_epoch == input_epoch
        && lease.config_revision == config_revision
        && lease.policy_revision == policy_revision
        && mode == Mode::Normal
}

pub(super) fn focus_matches(
    required: Option<&PromptFocusIdentity>,
    observed: Option<&PromptFocusIdentity>,
    editable: bool,
) -> bool {
    required.is_some() && required == observed && editable
}

#[cfg(test)]
mod tests {
    use super::{
        Completion, MAX_NATIVE_PROMPT_DEPTH, NativePromptLeases, PromptFocusIdentity, PromptOwner,
        focus_matches, owner_matches, prompt_category, restorable_mode, suspension_matches,
    };
    use ferric_browser_core::Mode;

    fn owner() -> PromptOwner {
        PromptOwner {
            window: Some("window-one".into()),
            tab: "tab-one".into(),
            view: Some("view-one".into()),
            document: Some("document-one".into()),
        }
    }

    fn push(leases: &mut NativePromptLeases, saved_mode: Option<Mode>) -> u64 {
        leases
            .push("permission", owner(), saved_mode, 5, 2, 3, None)
            .expect("lease within bounded depth")
    }

    #[test]
    fn nested_prompts_complete_in_lifo_order_and_only_outer_can_restore() {
        let mut leases = NativePromptLeases::default();
        let outer = push(&mut leases, Some(Mode::PassThrough));
        let inner = push(&mut leases, None);

        assert_eq!(leases.depth(), 2);
        assert_eq!(leases.begin_completion(inner), Completion::Nested);
        assert_eq!(leases.depth(), 1);
        assert_eq!(leases.begin_completion(outer), Completion::Final);
        assert_eq!(leases.depth(), 0);
    }

    #[test]
    fn out_of_order_completion_clears_the_stack_and_stale_completion_is_noop() {
        let mut leases = NativePromptLeases::default();
        let outer = push(&mut leases, Some(Mode::PassThrough));
        let _inner = push(&mut leases, None);

        assert_eq!(leases.begin_completion(outer), Completion::OutOfOrder);
        assert_eq!(leases.depth(), 0);
        assert_eq!(leases.last_result(), "out-of-order");
        assert_eq!(leases.begin_completion(outer), Completion::Stale);
    }

    #[test]
    fn stack_is_bounded_and_invalidates_when_depth_is_exceeded() {
        let mut leases = NativePromptLeases::default();
        for _ in 0..MAX_NATIVE_PROMPT_DEPTH {
            push(&mut leases, None);
        }
        assert!(
            leases
                .push("permission", owner(), None, 5, 2, 3, None)
                .is_none()
        );
        assert_eq!(leases.depth(), 0);
        assert_eq!(leases.last_result(), "depth-limit");
    }

    #[test]
    fn only_owner_bound_persistent_or_editing_modes_are_candidates() {
        let owner = owner();
        assert_eq!(
            restorable_mode(Mode::PassThrough, &owner),
            Some(Mode::PassThrough)
        );
        assert_eq!(restorable_mode(Mode::Insert, &owner), Some(Mode::Insert));
        assert_eq!(
            restorable_mode(Mode::InsertLocked, &owner),
            Some(Mode::InsertLocked)
        );
        for mode in [
            Mode::Normal,
            Mode::Command,
            Mode::Search,
            Mode::Hint,
            Mode::Grid,
            Mode::Caret,
            Mode::PassThroughOnce,
        ] {
            assert_eq!(restorable_mode(mode, &owner), None);
        }
        let mut unknown_document = owner.clone();
        unknown_document.document = None;
        assert_eq!(restorable_mode(Mode::PassThrough, &unknown_document), None);
    }

    #[test]
    fn editing_restore_requires_the_exact_observed_frame_context_and_editability() {
        let focus = PromptFocusIdentity {
            frame_path: "0.2".into(),
            context: 41,
        };
        assert!(focus_matches(Some(&focus), Some(&focus), true));
        assert!(!focus_matches(Some(&focus), None, true));
        assert!(!focus_matches(Some(&focus), Some(&focus), false));
        assert!(!focus_matches(
            Some(&focus),
            Some(&PromptFocusIdentity {
                frame_path: "0.1".into(),
                context: 41,
            }),
            true,
        ));
    }

    #[test]
    fn prompt_categories_are_closed_and_bounded() {
        assert_eq!(prompt_category("file-chooser"), Some("file-chooser"));
        assert_eq!(prompt_category("page-dialog"), Some("page-dialog"));
        assert_eq!(prompt_category("arbitrary"), None);
    }

    #[test]
    fn owner_epoch_configuration_and_policy_changes_reject_restore() {
        let mut leases = NativePromptLeases::default();
        let id = push(&mut leases, Some(Mode::PassThrough));
        let lease = leases.entry(id).expect("stored lease");
        assert!(owner_matches(&lease.owner, &owner()));
        assert!(!owner_matches(
            &lease.owner,
            &PromptOwner {
                tab: "tab-two".into(),
                ..owner()
            }
        ));
        assert!(suspension_matches(lease, 5, 2, 3, Mode::Normal));
        assert!(!suspension_matches(lease, 6, 2, 3, Mode::Normal));
        assert!(!suspension_matches(lease, 5, 4, 3, Mode::Normal));
        assert!(!suspension_matches(lease, 5, 2, 4, Mode::Normal));
        assert!(!suspension_matches(lease, 5, 2, 3, Mode::PassThrough));
    }
}
