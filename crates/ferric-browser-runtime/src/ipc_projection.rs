//! Bounded IPC observation data projected at the runtime boundary.
//!
//! The Qt adapter transports these values but does not decide their shape.

use ferric_browser_core::{ApplicationState, Event, Mode, TabId, Target};
use serde_json::Value;

#[must_use]
pub fn ipc_event_type(event: &Event) -> &'static str {
    match event {
        Event::CreateProfile { .. } => "profile.changed",
        Event::CreateWindow { .. } | Event::FocusWindow { .. } | Event::SetActiveWindow { .. } => {
            "window.changed"
        }
        Event::SetWindowContext { .. } => "context.changed",
        Event::OpenTab { .. }
        | Event::OpenTabWithNavigation { .. }
        | Event::OpenPopup { .. }
        | Event::MoveTab { .. }
        | Event::TransferTabOut { .. }
        | Event::TransferTabIn { .. }
        | Event::SetTabPinned { .. }
        | Event::SetTabMuted { .. }
        | Event::SetTabZoom { .. }
        | Event::ActivateTab { .. }
        | Event::CloseTab { .. }
        | Event::SuspendTab { .. }
        | Event::DiscardTab { .. }
        | Event::ResumeTab { .. }
        | Event::ViewClosed { .. } => "tab.changed",
        Event::StartNavigation { .. }
        | Event::CommitNavigation { .. }
        | Event::CommitNavigationWithTransition { .. }
        | Event::SameDocumentNavigation { .. }
        | Event::CompleteNavigation { .. }
        | Event::FailNavigation { .. }
        | Event::Reload { .. }
        | Event::Stop { .. }
        | Event::TraverseHistory { .. } => "navigation.changed",
        Event::StartSearch { .. }
        | Event::SearchNext { .. }
        | Event::EndSearch { .. }
        | Event::PushMode { .. }
        | Event::PopMode { .. }
        | Event::Escape { .. } => "mode.changed",
        Event::RendererTerminated { .. } => "renderer.changed",
        Event::ClearJourney { .. } => "history.changed",
        Event::RequestShutdown => "shutdown.requested",
    }
}

#[must_use]
pub fn ipc_event_payload(event: &Event, state: &ApplicationState) -> Value {
    match event {
        Event::CreateProfile { privacy, .. } => serde_json::json!({ "privacy": privacy.name() }),
        Event::CreateWindow { profile } => serde_json::json!({
            "profile_id": profile.to_string(), "window_count": state.windows().len()
        }),
        Event::SetWindowContext { window, .. } => serde_json::json!({
            "window_id": window.to_string(), "context_changed": true
        }),
        Event::OpenTab { window } | Event::OpenTabWithNavigation { window, .. } => {
            opened_tab_payload(state, *window)
        }
        Event::FocusWindow { window } | Event::SetActiveWindow { window } => serde_json::json!({
            "window_id": window.to_string(), "active": state.active_window() == Some(*window)
        }),
        Event::ActivateTab { window, tab } => serde_json::json!({
            "window_id": window.to_string(), "tab": tab_payload(state, *tab)
        }),
        Event::OpenPopup {
            opener,
            user_gesture,
            ..
        } => serde_json::json!({
            "opener": target_payload(state, *opener), "user_gesture": user_gesture
        }),
        Event::MoveTab {
            tab,
            to_window,
            index,
        } => serde_json::json!({
            "tab": tab_payload(state, *tab), "to_window_id": to_window.to_string(), "index": index
        }),
        Event::TransferTabIn { window, .. } => {
            serde_json::json!({ "window_id": window.to_string() })
        }
        Event::SetTabPinned { tab, pinned } => serde_json::json!({
            "tab": tab_payload(state, *tab), "pinned": pinned
        }),
        Event::SetTabMuted { tab, muted } => serde_json::json!({
            "tab": tab_payload(state, *tab), "muted": muted
        }),
        Event::SetTabZoom {
            tab,
            zoom_hundredths,
        } => serde_json::json!({
            "tab": tab_payload(state, *tab), "zoom_hundredths": zoom_hundredths
        }),
        Event::StartNavigation { target, .. }
        | Event::CommitNavigation { target, .. }
        | Event::CommitNavigationWithTransition { target, .. }
        | Event::SameDocumentNavigation { target, .. }
        | Event::CompleteNavigation { target }
        | Event::FailNavigation { target }
        | Event::Reload { target, .. }
        | Event::Stop { target }
        | Event::TraverseHistory { target, .. } => {
            serde_json::json!({ "target": target_payload(state, *target) })
        }
        Event::StartSearch {
            target,
            backward,
            case,
            ..
        } => serde_json::json!({
            "target": target_payload(state, *target), "backward": backward,
            "case": format!("{case:?}").to_ascii_lowercase(), "query_changed": true
        }),
        Event::SearchNext { target, backward } => serde_json::json!({
            "target": target_payload(state, *target), "backward": backward
        }),
        Event::EndSearch { target } => serde_json::json!({
            "target": target_payload(state, *target), "search_active": false
        }),
        Event::TransferTabOut { tab }
        | Event::CloseTab { tab }
        | Event::SuspendTab { tab }
        | Event::DiscardTab { tab }
        | Event::ResumeTab { tab } => serde_json::json!({ "tab": tab_payload(state, *tab) }),
        Event::ViewClosed { tab, generation } => serde_json::json!({
            "tab_id": tab.to_string(), "generation": generation
        }),
        Event::PushMode { window, mode } => serde_json::json!({
            "window_id": window.to_string(), "mode": mode_name(*mode)
        }),
        Event::PopMode { window } | Event::Escape { window } => serde_json::json!({
            "window_id": window.to_string(),
            "mode": state.windows().get(window).and_then(|window| window.modes.last())
                .map_or("normal", |mode| mode_name(*mode))
        }),
        Event::RendererTerminated { target } => serde_json::json!({
            "target": target_payload(state, *target), "renderer": "terminated"
        }),
        Event::ClearJourney { since, origin } => {
            serde_json::json!({ "since": since, "origin": origin })
        }
        Event::RequestShutdown => serde_json::json!({ "shutdown": "requested" }),
    }
}

fn opened_tab_payload(state: &ApplicationState, window_id: ferric_browser_core::WindowId) -> Value {
    let active_tab = state
        .windows()
        .get(&window_id)
        .and_then(|window| window.active_tab);
    serde_json::json!({
        "window_id": window_id.to_string(),
        "tab": active_tab.map_or(Value::Null, |tab| tab_payload(state, tab))
    })
}

fn target_payload(state: &ApplicationState, target: Target) -> Value {
    let mut payload = serde_json::json!({
        "tab_id": target.tab.to_string(), "generation": target.generation,
        "document_id": target.document.to_string()
    });
    if let Some(tab) = state.tabs().get(&target.tab) {
        payload["window_id"] = serde_json::json!(tab.window.to_string());
        payload["profile_id"] = serde_json::json!(tab.profile.to_string());
    }
    payload
}

fn tab_payload(state: &ApplicationState, tab_id: TabId) -> Value {
    let Some(tab) = state.tabs().get(&tab_id) else {
        return serde_json::json!({ "tab_id": tab_id.to_string() });
    };
    serde_json::json!({
        "tab_id": tab.id.to_string(), "window_id": tab.window.to_string(),
        "profile_id": tab.profile.to_string(), "generation": tab.generation,
        "document_id": tab.document.to_string(),
        "existence": format!("{:?}", tab.existence).to_ascii_lowercase(),
        "loading": format!("{:?}", tab.loading).to_ascii_lowercase(),
        "renderer": format!("{:?}", tab.renderer).to_ascii_lowercase(),
        "resources": format!("{:?}", tab.resources).to_ascii_lowercase(),
        "selected": state.windows().get(&tab.window).and_then(|window| window.active_tab)
            .is_some_and(|active| active == tab.id),
        "pinned": tab.pinned, "muted": tab.muted
    })
}

const fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Insert => "insert",
        Mode::Command => "command",
        Mode::Search => "search",
        Mode::Hint => "hint",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{PrivacyKind, ValidatedUrl, reduce};

    #[test]
    fn payloads_identify_targets_without_navigation_secrets() {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "Normal".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = *state.profiles().keys().next().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = *state.tabs().keys().next().unwrap();
        let target = state.capture_target(tab).unwrap();
        let payload = ipc_event_payload(
            &Event::CommitNavigation {
                target,
                url: ValidatedUrl::parse("https://example.test/private?token=secret").unwrap(),
                title: "private title".into(),
            },
            &state,
        );
        assert_eq!(payload["target"]["tab_id"], tab.to_string());
        assert!(payload.get("url").is_none());
        assert!(!payload.to_string().contains("secret"));
    }
}
