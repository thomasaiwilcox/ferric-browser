use super::{
    CxxQtType, Event, Mode, Pin, QString, SpatialCancelReason, current_target,
    finish_pending_selection_operation, qobject,
};

impl qobject::BrowserUi {
    pub(super) fn back(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self
                .as_mut()
                .reduce_event(Event::TraverseHistory { target, offset: -1 });
            self.set_status_text(QString::from("Back requested"));
        }
    }

    pub(super) fn forward(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self
                .as_mut()
                .reduce_event(Event::TraverseHistory { target, offset: 1 });
            self.set_status_text(QString::from("Forward requested"));
        }
    }

    pub(super) fn reload(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self.as_mut().reduce_event(Event::Reload {
                target,
                bypass_cache: false,
            });
            self.set_status_text(QString::from("Reload requested"));
        }
    }

    pub(super) fn stop(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self.as_mut().reduce_event(Event::Stop { target });
            self.set_status_text(QString::from("Stop requested"));
        }
    }

    pub(super) fn escape(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().core_mode == Mode::Grid {
            self.as_mut()
                .cancel_spatial_navigation(SpatialCancelReason::User);
            return;
        }
        self.as_mut().set_binding_overlay(QString::default());
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
        }
        let is_search = self.as_ref().rust().core_mode == Mode::Search;
        let is_hint = self.as_ref().rust().core_mode == Mode::Hint;
        self.as_mut().suppress_current_focus_observations();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            finish_pending_selection_operation(this, "cancelled");
            this.pending_caret = None;
            this.pending_spawn = None;
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states
                    .insert(pending.operation_id, "cancelled".into());
            }
            this.pending_download = None;
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states
                    .insert(pending.operation_id, "cancelled".into());
            }
        }
        self.as_mut().set_caret_selecting(false);
        if is_hint {
            self.as_mut().clear_hint_session_state();
        }
        if is_search {
            let target = current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            );
            if let Some(target) = target
                && let Ok(effects) = self.as_mut().reduce_event(Event::EndSearch { target })
            {
                self.as_mut().set_pending_engine_action(&effects);
            }
        }
        let window = self.as_ref().rust().window;
        if let Some(window) = window {
            let _ = self.as_mut().reduce_event(Event::Escape { window });
        }
        self.as_mut().set_core_mode(Mode::Normal);
        self.as_mut().set_mode(QString::from("normal"));
        self.as_mut().clear_completion();
        if is_search {
            self.as_mut().set_search_text(QString::default());
        }
        self.set_status_text(QString::from("Normal mode"));
    }

    pub(super) fn enter_command(mut self: Pin<&mut Self>) {
        let window = self.as_ref().rust().window;
        if let Some(window) = window {
            let _ = self.as_mut().reduce_event(Event::PushMode {
                window,
                mode: Mode::Command,
            });
        }
        self.as_mut().set_core_mode(Mode::Command);
        self.as_mut().set_mode(QString::from("command"));
        self.set_status_text(QString::from("Command mode"));
    }

    pub(super) fn enter_insert(mut self: Pin<&mut Self>) {
        let window = self.as_ref().rust().window;
        if let Some(window) = window {
            let _ = self.as_mut().reduce_event(Event::PushMode {
                window,
                mode: Mode::Insert,
            });
        }
        self.as_mut().set_core_mode(Mode::Insert);
        self.as_mut().set_mode(QString::from("insert"));
        self.set_status_text(QString::from("Insert mode"));
    }

    pub(super) fn enter_caret(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().core_mode == Mode::Caret {
            return;
        }
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("No active window"));
            return;
        };
        if self
            .as_mut()
            .reduce_event(Event::PushMode {
                window,
                mode: Mode::Caret,
            })
            .is_err()
        {
            self.set_status_text(QString::from("Caret mode rejected"));
            return;
        }
        self.as_mut().set_core_mode(Mode::Caret);
        self.as_mut().set_mode(QString::from("caret"));
        self.as_mut().set_caret_selecting(false);
        self.set_status_text(QString::from("Caret mode"));
    }
}
