//! Typed tab and window-context projection for QML-facing Qt properties.

use super::{CxxQtType, Pin, QString, Target, current_target, qobject};

impl qobject::BrowserUi {
    pub(super) fn target_for_index(&self, index: i32) -> Option<Target> {
        current_target(self.rust().state.as_ref(), self.tab_for_index(index))
    }

    pub(super) fn sync_active_tab_properties(mut self: Pin<&mut Self>) {
        let (index, tab) = {
            let rust = self.as_ref().get_ref().rust();
            let Some(state) = rust.state.as_ref() else {
                return;
            };
            let Some(window) = rust.window else {
                return;
            };
            let Some(tab) = state
                .windows()
                .get(&window)
                .and_then(|window| window.active_tab)
            else {
                return;
            };
            let Some(index) = rust.tab_ids.iter().position(|candidate| *candidate == tab) else {
                return;
            };
            (i32::try_from(index).unwrap_or(i32::MAX), tab)
        };
        self.as_mut().rust_mut().as_mut().get_mut().tab = Some(tab);
        self.as_mut().set_active_tab_properties(index, tab);
    }

    pub(super) fn sync_core_tabs(mut self: Pin<&mut Self>) {
        let active_tab = self.as_ref().get_ref().rust().window.and_then(|window| {
            self.as_ref()
                .get_ref()
                .rust()
                .state
                .as_ref()?
                .windows()
                .get(&window)?
                .active_tab
        });
        let Some(active_tab) = active_tab else {
            self.as_mut().sync_context_metadata();
            return;
        };
        let (index, count) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !this.tab_ids.contains(&active_tab) && !this.popup_tab_ids.contains(&active_tab) {
                this.tab_ids.push(active_tab);
            }
            let Some(index) = this.tab_ids.iter().position(|tab| *tab == active_tab) else {
                return;
            };
            this.tab = Some(active_tab);
            (
                i32::try_from(index).unwrap_or(i32::MAX),
                i32::try_from(this.tab_ids.len()).unwrap_or(i32::MAX),
            )
        };
        self.as_mut().set_tab_count(count);
        self.as_mut().set_active_tab_properties(index, active_tab);
        self.as_mut().sync_context_metadata();
    }

    pub(super) fn sync_tab_order_from_core(mut self: Pin<&mut Self>) {
        let (tab_ids, active_tab) = {
            let rust = self.as_ref().get_ref().rust();
            let Some(window) = rust.window else {
                return;
            };
            let Some(window_state) = rust
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
            else {
                return;
            };
            (window_state.tabs.clone(), window_state.active_tab)
        };
        let active_index = i32::try_from(
            active_tab
                .and_then(|tab| tab_ids.iter().position(|candidate| *candidate == tab))
                .unwrap_or(0),
        )
        .unwrap_or(i32::MAX);
        let tab_count = i32::try_from(tab_ids.len()).unwrap_or(i32::MAX);
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids = tab_ids;
            this.tab = active_tab;
            this.focus_observations.clear();
            this.focus_suppressions.clear();
        }
        self.as_mut().set_tab_count(tab_count);
        if let Some(tab) = active_tab {
            self.as_mut().set_active_tab_properties(active_index, tab);
        } else {
            self.as_mut().set_active_tab_index(0);
        }
        self.as_mut().sync_context_metadata();
    }

    pub(super) fn sync_context_metadata(mut self: Pin<&mut Self>) {
        let (name, label, workspace, accent) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let context = rust
                .window
                .and_then(|window_id| {
                    rust.state
                        .as_ref()?
                        .windows()
                        .get(&window_id)?
                        .context
                        .as_deref()
                })
                .and_then(|name| {
                    rust.contexts
                        .as_ref()?
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                });
            context.map_or_else(
                || (String::new(), String::new(), String::new(), String::new()),
                |context| {
                    (
                        context.name.clone(),
                        context.label.clone(),
                        context.workspace.clone().unwrap_or_default(),
                        context.accent.clone().unwrap_or_default(),
                    )
                },
            )
        };
        self.as_mut().set_context_name(QString::from(name));
        self.as_mut().set_context_label(QString::from(label));
        self.as_mut()
            .set_context_workspace(QString::from(workspace));
        self.as_mut().set_context_accent(QString::from(accent));
    }
}
