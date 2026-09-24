//! Read-only projections of reducer effects and session restore entries.

use ferric_browser_core::{Effect, EngineEffect};
use ferric_browser_storage::RestorePlanEntry;

pub(super) fn restore_entry_line(entry: &RestorePlanEntry) -> String {
    let (scroll_x, scroll_y) = entry.scroll_position.unwrap_or((-1.0, -1.0));
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        entry.url.as_deref().unwrap_or("about:blank"),
        entry.pinned,
        entry.muted,
        entry.zoom,
        scroll_x,
        scroll_y,
    )
}

pub(super) fn engine_action_name(effect: &Effect) -> Option<&'static str> {
    match effect {
        Effect::Engine(EngineEffect::Navigate { .. }) => Some("navigate"),
        Effect::Engine(EngineEffect::Reload { .. }) => Some("reload"),
        Effect::Engine(EngineEffect::Stop { .. }) => Some("stop"),
        Effect::Engine(EngineEffect::TraverseHistory { offset, .. }) if *offset < 0 => Some("back"),
        Effect::Engine(EngineEffect::TraverseHistory { .. }) => Some("forward"),
        Effect::Engine(EngineEffect::FindText { .. }) => Some("find"),
        Effect::Engine(EngineEffect::ClearFindText { .. }) => Some("clear-find"),
        Effect::Engine(EngineEffect::CloseView { .. }) => Some("close"),
        Effect::Engine(EngineEffect::SetTabLifecycle { .. }) => Some("tab-lifecycle"),
        Effect::Engine(EngineEffect::ReparentView { .. }) => Some("reparent"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_storage::RestoreMethod;
    use uuid::Uuid;

    #[test]
    fn restore_projection_uses_explicit_placeholder_scroll_coordinates() {
        let entry = RestorePlanEntry {
            tab_id: Uuid::new_v4(),
            selected: true,
            url: None,
            method: RestoreMethod::Placeholder,
            placeholder_reason: Some("unsafe".into()),
            pinned: true,
            muted: false,
            zoom: 1.25,
            scroll_position: None,
        };

        assert_eq!(
            restore_entry_line(&entry),
            "about:blank\ttrue\tfalse\t1.25\t-1\t-1"
        );
    }
}
