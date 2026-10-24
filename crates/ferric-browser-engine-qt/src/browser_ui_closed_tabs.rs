//! Closed-tab reopening and proof that its automatic native fallback is unused.

use super::{
    BrowserUiRust, CxxQtType, Event, ExistenceState, LoadingState, Pin, QString, ResourceLifecycle,
    Target, ValidatedUrl, current_target, qobject,
};

pub(super) fn reusable_undo_fallback(rust: &BrowserUiRust) -> Option<Target> {
    let target = rust.undo_fallback?;
    let state = rust.state.as_ref()?;
    let tab = state.tabs().get(&target.tab)?;
    let history = rust.navigation.content_histories.get(&target.tab)?;
    let (entries, cursor) = history.snapshot();
    (rust.undo_fallback_ready
        && current_target(Some(state), Some(target.tab)) == Some(target)
        && Some(tab.window) == rust.window
        && rust.tab == Some(target.tab)
        && rust.tab_ids == [target.tab]
        && state.windows().get(&tab.window).is_some_and(|window| {
            window.tabs == [target.tab] && window.active_tab == Some(target.tab)
        })
        && tab.existence == ExistenceState::Live
        && tab.resources == ResourceLifecycle::Active
        && tab.url.as_deref() == Some("rb://start")
        && tab.loading == LoadingState::Complete
        && !tab.pinned
        && !tab.muted
        && (tab.zoom - 1.0).abs() < f64::EPSILON
        && cursor == 0
        && entries.len() == 1
        && entries[0].url == "rb://start"
        && rust.pending_engine_action.is_none()
        && rust.pending_engine_actions.is_empty())
    .then_some(target)
}

impl qobject::BrowserUi {
    pub(super) fn report_undo_fallback(mut self: Pin<&mut Self>, id: &QString, ready: bool) {
        let Some(target) = self.as_ref().rust().undo_fallback else {
            return;
        };
        if id.to_string() != target.tab.to_string()
            || current_target(self.as_ref().rust().state.as_ref(), Some(target.tab)) != Some(target)
        {
            return;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .undo_fallback_ready = ready;
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn reopen_closed_tab(mut self: Pin<&mut Self>, id: &str) -> bool {
        let Some(closed) = self
            .as_ref()
            .rust()
            .closed_tabs
            .iter()
            .find(|closed| closed.id.to_string() == id)
            .cloned()
        else {
            self.set_status_text(QString::from(
                "That closed tab is no longer available to reopen.",
            ));
            return false;
        };
        let Some(window) = self.as_ref().rust().window else {
            return false;
        };
        let Ok(url) = ValidatedUrl::parse(closed.url.clone()) else {
            self.set_status_text(QString::from("Closed tab URL is no longer safe to reopen"));
            return false;
        };
        let reused = reusable_undo_fallback(self.as_ref().rust()).is_some();
        if reused {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if let Some(target) = this.undo_fallback.take() {
                // This single native entry belongs to the automatic fallback,
                // not to the safe-GET page being reopened in its place.
                this.navigation.content_histories.remove(&target.tab);
            }
            this.undo_fallback_ready = false;
        } else if self
            .as_mut()
            .reduce_event(Event::OpenTab { window })
            .is_err()
        {
            return false;
        }
        let Some(tab) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.windows().get(&window))
            .and_then(|window| window.active_tab)
        else {
            return false;
        };
        let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(tab)) else {
            return false;
        };
        // Restore presentation before publishing the tab to QML so its
        // first renderer already has the requested mute and zoom settings.
        for event in [
            Event::SetTabPinned {
                tab,
                pinned: closed.pinned,
            },
            Event::SetTabMuted {
                tab,
                muted: closed.muted,
            },
            Event::SetTabZoom {
                tab,
                zoom_hundredths: closed.zoom_hundredths,
            },
        ] {
            if self.as_mut().reduce_event(event).is_err() {
                return false;
            }
        }
        if let Some(group_index) = closed.group_index {
            let group_start = if closed.pinned {
                0
            } else {
                self.as_ref().rust().state.as_ref().map_or(0, |state| {
                    state.windows().get(&window).map_or(0, |window| {
                        window
                            .tabs
                            .iter()
                            .filter(|id| state.tabs().get(*id).is_some_and(|tab| tab.pinned))
                            .count()
                    })
                })
            };
            if self
                .as_mut()
                .reduce_event(Event::MoveTab {
                    tab,
                    to_window: window,
                    index: Some(group_start.saturating_add(group_index)),
                })
                .is_err()
            {
                return false;
            }
        }
        let Ok(effects) = self
            .as_mut()
            .reduce_event(Event::StartNavigation { target, url })
        else {
            return false;
        };
        // A reused native view already exists. Apply its restored zoom before
        // navigation, using the new document target captured by StartNavigation.
        if reused
            && self
                .as_mut()
                .queue_view_command_for(
                    ferric_browser_application::ViewCommand::Zoom {
                        hundredths: closed.zoom_hundredths,
                    },
                    tab,
                )
                .is_err()
        {
            return false;
        }
        self.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&effects);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .closed_tabs
                .retain(|candidate| candidate.id != closed.id);
        }
        self.set_status_text(QString::from("Closed tab reopened"));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::BrowserApplication;
    use ferric_browser_config::Config;
    use ferric_browser_core::{ContentHistory, PrivacyKind};
    use ferric_browser_runtime::RuntimeInput;

    fn apply(rust: &mut BrowserUiRust, event: Event) {
        rust.state
            .as_mut()
            .unwrap()
            .dispatch_runtime(RuntimeInput::EngineFact(event))
            .unwrap();
    }

    fn fallback_fixture() -> BrowserUiRust {
        let (mut application, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .unwrap();
        let target = application.capture_target(tab).unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CloseTab { tab }))
            .unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ViewClosed {
                tab,
                generation: target.generation,
            }))
            .unwrap();
        let tab = application.windows()[&window].active_tab.unwrap();
        let target = application.capture_target(tab).unwrap();
        let mut rust = BrowserUiRust {
            state: Some(application),
            window: Some(window),
            tab: Some(tab),
            tab_ids: vec![tab],
            undo_fallback: Some(target),
            undo_fallback_ready: true,
            ..BrowserUiRust::default()
        };
        let mut history = ContentHistory::default();
        history.record("rb://start".into(), 0, false);
        rust.navigation.content_histories.insert(tab, history);
        rust
    }

    #[test]
    fn undo_reuse_requires_captured_ownership_and_native_surface_proof() {
        let mut rust = fallback_fixture();
        let expected = rust.undo_fallback;
        assert_eq!(reusable_undo_fallback(&rust), expected);
        rust.undo_fallback_ready = false;
        assert_eq!(reusable_undo_fallback(&rust), None);
        rust.undo_fallback_ready = true;
        rust.undo_fallback = None;
        assert_eq!(reusable_undo_fallback(&rust), None);
        rust.undo_fallback = expected;
        let window = rust.window.take();
        assert_eq!(reusable_undo_fallback(&rust), None);
        rust.window = window;
        rust.pending_engine_action = Some("tab-open\thttps://example.test/new-work".into());
        assert_eq!(reusable_undo_fallback(&rust), None);
        rust.pending_engine_action = None;
        assert_eq!(reusable_undo_fallback(&rust), expected);
    }

    #[test]
    fn undo_reuse_rejects_a_new_document_even_at_the_same_native_address() {
        let mut rust = fallback_fixture();
        let target = rust.undo_fallback.unwrap();
        apply(
            &mut rust,
            Event::StartNavigation {
                target,
                url: ValidatedUrl::parse("rb://start").unwrap(),
            },
        );
        let replacement = rust
            .state
            .as_ref()
            .unwrap()
            .capture_target(target.tab)
            .unwrap();
        apply(
            &mut rust,
            Event::CommitNavigation {
                target: replacement,
                url: ValidatedUrl::parse("rb://start").unwrap(),
                title: "Start page".into(),
            },
        );
        apply(
            &mut rust,
            Event::CompleteNavigation {
                target: replacement,
            },
        );
        assert_ne!(replacement, target);
        assert_eq!(reusable_undo_fallback(&rust), None);
    }

    #[test]
    fn undo_reuse_preserves_native_history_and_additional_tabs() {
        let mut rust = fallback_fixture();
        let tab = rust.tab.unwrap();
        let history = rust.navigation.content_histories.get_mut(&tab).unwrap();
        history.record("rb://blank".into(), 0, false);
        history.record("rb://start".into(), 0, false);
        assert_eq!(reusable_undo_fallback(&rust), None);
        let mut rust = fallback_fixture();
        let window = rust.window.unwrap();
        apply(&mut rust, Event::OpenTab { window });
        assert_eq!(reusable_undo_fallback(&rust), None);
    }

    #[test]
    fn undo_reuse_keeps_changed_pin_mute_and_zoom_settings() {
        let mut rust = fallback_fixture();
        let tab = rust.tab.unwrap();
        for (change, revert) in [
            (
                Event::SetTabPinned { tab, pinned: true },
                Event::SetTabPinned { tab, pinned: false },
            ),
            (
                Event::SetTabMuted { tab, muted: true },
                Event::SetTabMuted { tab, muted: false },
            ),
            (
                Event::SetTabZoom {
                    tab,
                    zoom_hundredths: 150,
                },
                Event::SetTabZoom {
                    tab,
                    zoom_hundredths: 100,
                },
            ),
        ] {
            apply(&mut rust, change);
            assert_eq!(reusable_undo_fallback(&rust), None);
            apply(&mut rust, revert);
            assert_eq!(reusable_undo_fallback(&rust), rust.undo_fallback);
        }
    }
}
