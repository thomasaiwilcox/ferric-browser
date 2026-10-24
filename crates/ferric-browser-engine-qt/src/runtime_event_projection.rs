//! Wire projection of facts captured at the reduction boundary.
//!
//! Serialization never reads later application state or page data.

use ferric_browser_core::{Event, Mode, TabId, Target, WindowId};
use ferric_browser_runtime::RuntimeObservation;
use serde_json::Value;

#[must_use]
pub fn ipc_event_type(event: &Event) -> &'static str {
    match event {
        Event::CreateProfile { .. } => "profile.changed",
        Event::CreateWindow { .. } | Event::FocusWindow { .. } | Event::SetActiveWindow { .. } => {
            "window.changed"
        }
        Event::SetWindowContext { .. } | Event::ClearWindowContexts { .. } => "context.changed",
        Event::OpenTab { .. }
        | Event::OpenTabWithNavigation { .. }
        | Event::OpenPopup { .. }
        | Event::MoveTab { .. }
        | Event::TransferTabOut { .. }
        | Event::TransferTabIn { .. }
        | Event::SetTabPinned { .. }
        | Event::SetTabMuted { .. }
        | Event::SetTabZoom { .. }
        | Event::RestoreTabAddress { .. }
        | Event::PageTitleChanged { .. }
        | Event::ActivateTab { .. }
        | Event::CloseTab { .. }
        | Event::SuspendTab { .. }
        | Event::DiscardTab { .. }
        | Event::ResumeTab { .. }
        | Event::ViewClosed { .. } => "tab.changed",
        Event::StartNavigation { .. }
        | Event::NavigationStarted { .. }
        | Event::CommitNavigation { .. }
        | Event::CommitNavigationWithTransition { .. }
        | Event::CommitNavigationAtVisit { .. }
        | Event::SameDocumentNavigation { .. }
        | Event::CompleteNavigation { .. }
        | Event::CancelNavigation { .. }
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
        Event::ClipboardEdit { .. } => "clipboard.edit-requested",
        Event::RequestShutdown => "shutdown.requested",
    }
}

#[must_use]
pub fn ipc_event_payload(observation: &RuntimeObservation) -> Value {
    let event = &observation.event;
    match event {
        Event::ClipboardEdit { target, operation } => {
            serde_json::json!({"tab": target.tab.to_string(), "operation": operation.as_str(), "status": "requested"})
        }
        Event::CreateProfile { privacy, .. } => serde_json::json!({ "privacy": privacy.name() }),
        Event::CreateWindow { profile } => serde_json::json!({
            "profile_id": profile.to_string(), "window_count": observation.window_count
        }),
        Event::SetWindowContext { window, .. } => serde_json::json!({
            "window_id": window.to_string(), "context_changed": true
        }),
        Event::ClearWindowContexts { windows } => cleared_context_payload(windows),
        Event::OpenTab { window } | Event::OpenTabWithNavigation { window, .. } => {
            opened_tab_payload(observation, *window)
        }
        Event::FocusWindow { window } | Event::SetActiveWindow { window } => serde_json::json!({
            "window_id": window.to_string(), "active": observation.active_window == Some(*window)
        }),
        Event::ActivateTab { window, tab } => serde_json::json!({
            "window_id": window.to_string(), "tab": tab_payload(observation, *tab)
        }),
        Event::OpenPopup {
            opener,
            user_gesture,
            ..
        } => serde_json::json!({
            "opener": target_payload(observation, *opener), "user_gesture": user_gesture
        }),
        Event::MoveTab {
            tab,
            to_window,
            index,
        } => serde_json::json!({
            "tab": tab_payload(observation, *tab), "to_window_id": to_window.to_string(), "index": index
        }),
        Event::TransferTabIn { window, .. } => {
            serde_json::json!({ "window_id": window.to_string() })
        }
        Event::SetTabPinned { tab, pinned } => serde_json::json!({
            "tab": tab_payload(observation, *tab), "pinned": pinned
        }),
        Event::SetTabMuted { tab, muted } => serde_json::json!({
            "tab": tab_payload(observation, *tab), "muted": muted
        }),
        Event::SetTabZoom {
            tab,
            zoom_hundredths,
        } => serde_json::json!({
            "tab": tab_payload(observation, *tab), "zoom_hundredths": zoom_hundredths
        }),
        Event::PageTitleChanged { target, .. } => serde_json::json!({
            "target": target_payload(observation, *target), "title_changed": true
        }),
        Event::RestoreTabAddress { target, .. } => serde_json::json!({
            "target": target_payload(observation, *target), "restore_address_changed": true
        }),
        Event::StartNavigation { target, .. }
        | Event::NavigationStarted { target }
        | Event::CommitNavigation { target, .. }
        | Event::CommitNavigationWithTransition { target, .. }
        | Event::CommitNavigationAtVisit { target, .. }
        | Event::SameDocumentNavigation { target, .. }
        | Event::CompleteNavigation { target }
        | Event::CancelNavigation { target }
        | Event::FailNavigation { target }
        | Event::Reload { target, .. }
        | Event::Stop { target }
        | Event::TraverseHistory { target, .. } => {
            serde_json::json!({ "target": target_payload(observation, *target) })
        }
        Event::StartSearch {
            target,
            backward,
            case,
            ..
        } => serde_json::json!({
            "target": target_payload(observation, *target), "backward": backward,
            "case": format!("{case:?}").to_ascii_lowercase(), "query_changed": true
        }),
        Event::SearchNext { target, backward } => serde_json::json!({
            "target": target_payload(observation, *target), "backward": backward
        }),
        Event::EndSearch { target } => serde_json::json!({
            "target": target_payload(observation, *target), "search_active": false
        }),
        Event::TransferTabOut { tab }
        | Event::CloseTab { tab }
        | Event::SuspendTab { tab }
        | Event::DiscardTab { tab }
        | Event::ResumeTab { tab } => serde_json::json!({ "tab": tab_payload(observation, *tab) }),
        Event::ViewClosed { tab, generation } => serde_json::json!({
            "tab_id": tab.to_string(), "generation": generation
        }),
        Event::PushMode { window, mode } => serde_json::json!({
            "window_id": window.to_string(), "mode": mode_name(*mode)
        }),
        Event::PopMode { window } | Event::Escape { window } => serde_json::json!({
            "window_id": window.to_string(),
            "mode": mode_name(observation.mode)
        }),
        Event::RendererTerminated { target } => serde_json::json!({
            "target": target_payload(observation, *target), "renderer": "terminated"
        }),
        Event::ClearJourney { since, origin } => {
            serde_json::json!({ "since": since, "origin": origin })
        }
        Event::RequestShutdown => serde_json::json!({ "shutdown": "requested" }),
    }
}

fn opened_tab_payload(observation: &RuntimeObservation, window: WindowId) -> Value {
    serde_json::json!({"window_id": window.to_string(),
        "tab": observation.tab.as_ref().map_or(Value::Null, |tab| tab_payload(observation, tab.id))})
}

fn target_payload(observation: &RuntimeObservation, target: Target) -> Value {
    let mut payload = serde_json::json!({"tab_id": target.tab.to_string(),
        "generation": target.generation, "document_id": target.document.to_string()});
    if let Some(tab) = observation.tab.as_ref().filter(|tab| tab.id == target.tab) {
        payload["window_id"] = serde_json::json!(tab.window.to_string());
        payload["profile_id"] = serde_json::json!(tab.profile.to_string());
    }
    payload
}

fn tab_payload(observation: &RuntimeObservation, id: TabId) -> Value {
    let Some(tab) = observation.tab.as_ref().filter(|tab| tab.id == id) else {
        return serde_json::json!({"tab_id": id.to_string()});
    };
    serde_json::json!({
        "tab_id": tab.id.to_string(), "window_id": tab.window.to_string(),
        "profile_id": tab.profile.to_string(), "generation": tab.generation,
        "document_id": tab.document.to_string(),
        "existence": format!("{:?}", tab.existence).to_ascii_lowercase(),
        "loading": format!("{:?}", tab.loading).to_ascii_lowercase(),
        "renderer": format!("{:?}", tab.renderer).to_ascii_lowercase(),
        "resources": format!("{:?}", tab.resources).to_ascii_lowercase(),
        "selected": tab.selected, "pinned": tab.pinned, "muted": tab.muted
    })
}

const fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Insert => "insert",
        Mode::InsertLocked => "insert-locked",
        Mode::Command => "command",
        Mode::Search => "search",
        Mode::Hint => "hint",
        Mode::Grid => "grid",
        Mode::Caret => "caret",
        Mode::PassThrough => "pass-through",
        Mode::PassThroughOnce => "pass-through-once",
    }
}

fn cleared_context_payload(windows: &[WindowId]) -> Value {
    serde_json::json!({
        "window_ids": windows.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "context_changed": true,
    })
}
