//! `WebEngine` navigation facts translated into reducer inputs and presentation.

use super::{
    CxxQtType, Event, Pin, QString, ValidatedUrl, bounded_navigation_failure_detail,
    current_target, normalized_navigation_failure_kind, qobject, safe_ipc_url,
};

impl qobject::BrowserUi {
    pub(super) fn navigation_completed(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self
                .as_mut()
                .reduce_event(Event::CompleteNavigation { target });
        }
        self.as_mut().set_load_state(QString::from("complete"));
        self.set_status_text(QString::from("Ready"));
    }

    pub(super) fn navigation_completed_for(mut self: Pin<&mut Self>, index: i32) {
        if let Some(target) = self.as_ref().get_ref().target_for_index(index) {
            let _ = self
                .as_mut()
                .reduce_event(Event::CompleteNavigation { target });
        }
        if self.as_ref().get_ref().tab_for_index(index)
            == self.as_ref().rust().checkpoint.restore_pending
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .checkpoint
                .restore_pending = None;
        }
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut().set_load_state(QString::from("complete"));
            self.set_status_text(QString::from("Ready"));
        }
    }

    pub(super) fn navigation_failed(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let _ = self.as_mut().take_journey_redirect(target);
            let _ = self.as_mut().take_journey_transition(target);
            let _ = self.as_mut().take_journey_parent(target);
            let _ = self.as_mut().reduce_event(Event::FailNavigation { target });
        }
        self.as_mut().set_load_state(QString::from("failed"));
        self.set_status_text(QString::from("Load failed"));
    }

    pub(super) fn navigation_failed_for(mut self: Pin<&mut Self>, index: i32) {
        if let Some(target) = self.as_ref().get_ref().target_for_index(index) {
            let _ = self.as_mut().take_journey_redirect(target);
            let _ = self.as_mut().take_journey_transition(target);
            let _ = self.as_mut().take_journey_parent(target);
            let _ = self.as_mut().reduce_event(Event::FailNavigation { target });
        }
        if self.as_ref().get_ref().tab_for_index(index)
            == self.as_ref().rust().checkpoint.restore_pending
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.checkpoint.restore_pending = None;
            this.checkpoint.dirty = false;
            this.checkpoint.dirty_since_ms = None;
        }
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut().set_load_state(QString::from("failed"));
            self.set_status_text(QString::from("Load failed"));
        }
    }

    pub(super) fn navigation_failed_with_details(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        kind: &QString,
        detail: &QString,
    ) {
        self.as_mut().navigation_failed_for(index);
        if index != self.as_ref().rust().active_tab_index {
            return;
        }
        let safe_url = ValidatedUrl::parse(url.to_string())
            .map_or_else(|_| "[unavailable]".into(), |url| safe_ipc_url(url.as_str()));
        let kind = normalized_navigation_failure_kind(&kind.to_string());
        let detail = bounded_navigation_failure_detail(&detail.to_string());
        self.as_mut()
            .set_navigation_failure_kind(QString::from(kind));
        self.as_mut()
            .set_navigation_failure_url(QString::from(safe_url));
        self.as_mut()
            .set_navigation_failure_detail(QString::from(&detail));
        self.as_mut().set_navigation_failure_visible(true);
        let status = if detail.is_empty() {
            format!("{} failure", kind.to_ascii_uppercase())
        } else {
            format!("{} failure: {detail}", kind.to_ascii_uppercase())
        };
        self.set_status_text(QString::from(status));
    }
}
