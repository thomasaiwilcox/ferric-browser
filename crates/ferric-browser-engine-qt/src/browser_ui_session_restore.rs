use super::{
    ContextMember, CxxQtType, Event, JourneyEdgeKind, Pin, QString, ValidatedUrl, current_target,
    is_safe_history_url, qobject, sanitize_untrusted_title,
};

impl qobject::BrowserUi {
    /// Materializes the safe portion of a saved context membership into the
    /// current profile window. Runtime tab IDs are deliberately not reused;
    /// only the bounded descriptors are replayed, and inactive tabs remain
    /// unloaded in QML until selected.
    pub(super) fn restore_context_membership(
        mut self: Pin<&mut Self>,
        member: &ContextMember,
    ) -> Result<usize, String> {
        let descriptors = member
            .tab_descriptors
            .iter()
            .filter(|descriptor| {
                descriptor
                    .safe_restore_url
                    .as_deref()
                    .is_none_or(is_safe_history_url)
            })
            .cloned()
            .collect::<Vec<_>>();
        if descriptors.is_empty() {
            self.set_session_restore_values(QString::default());
            return Ok(0);
        }
        let Some(window) = self.as_ref().rust().window else {
            return Err("No context restore window".into());
        };
        let (existing_tabs, existing_active) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Some(state) = rust.state.as_ref() else {
                return Err("core state unavailable".into());
            };
            let Some(window_state) = state.windows().get(&window) else {
                return Err("context window is unavailable".into());
            };
            (window_state.tabs.clone(), window_state.active_tab)
        };
        let append = existing_tabs.len() > 1;
        let selected_index = member
            .selected_index
            .filter(|index| *index < descriptors.len())
            .unwrap_or(0);
        let mut tab_ids = if append {
            existing_tabs
        } else {
            vec![existing_active.ok_or_else(|| "context window has no active tab".to_owned())?]
        };
        let mut selected_tab = None;
        let mut selected_effects = None;
        let mut restore_lines = Vec::with_capacity(descriptors.len());

        for (index, descriptor) in descriptors.iter().enumerate() {
            let tab = if !append && index == 0 {
                tab_ids[0]
            } else {
                self.as_mut()
                    .reduce_event(Event::OpenTab { window })
                    .map_err(|error| error.to_string())?;
                let tab = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
                    .ok_or_else(|| "context restore tab creation failed".to_owned())?;
                tab_ids.push(tab);
                tab
            };

            self.as_mut()
                .reduce_event(Event::SetTabPinned {
                    tab,
                    pinned: descriptor.pinned,
                })
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .reduce_event(Event::SetTabMuted {
                    tab,
                    muted: descriptor.muted,
                })
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .reduce_event(Event::SetTabZoom {
                    tab,
                    zoom_hundredths: (descriptor.zoom * 100.0).round() as u32,
                })
                .map_err(|error| error.to_string())?;

            let url = descriptor
                .safe_restore_url
                .as_deref()
                .unwrap_or("about:blank");
            if let Ok(url) = ValidatedUrl::parse(url)
                && let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
            {
                let effects = self
                    .as_mut()
                    .reduce_event(Event::StartNavigation { target, url })
                    .map_err(|error| error.to_string())?;
                self.as_mut().mark_journey_transition(
                    &effects,
                    JourneyEdgeKind::SessionRestore,
                    "session-restore",
                );
                if index == selected_index {
                    selected_effects = Some(effects);
                }
            }
            if index == selected_index {
                selected_tab = Some(tab);
            }
            restore_lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                url,
                descriptor.pinned,
                descriptor.muted,
                descriptor.zoom,
                sanitize_untrusted_title(&descriptor.title)
            ));
        }

        let selected_tab =
            selected_tab.ok_or_else(|| "context restore selection failed".to_owned())?;
        self.as_mut()
            .reduce_event(Event::ActivateTab {
                window,
                tab: selected_tab,
            })
            .map_err(|error| error.to_string())?;
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids = tab_ids;
            this.popup_tab_ids.clear();
            this.popup_journey_targets.clear();
            this.tab = Some(selected_tab);
            this.pending_engine_action = None;
            this.pending_engine_actions.clear();
            this.pending_journey_traversal = None;
            this.pending_journey_traversals.clear();
        }
        self.as_mut()
            .set_session_restore_values(QString::from(format!(
                "{}\n{}",
                if append { "append" } else { "replace" },
                restore_lines.join("\n")
            )));
        self.as_mut().sync_tab_order_from_core();
        if let Some(effects) = selected_effects {
            self.as_mut().mark_journey_transition(
                &effects,
                JourneyEdgeKind::SessionRestore,
                "session-restore",
            );
            self.as_mut().set_pending_engine_action(&effects);
        }
        Ok(descriptors.len())
    }
}
