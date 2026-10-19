//! Saved-source admission and completion for the invoking profile's Switcher.

use super::{
    CatalogReadState, CxxQtType, Pin, PrivacyKind, QString, SourceNeeds, SourceProfile,
    SwitcherAvailability, qobject,
};

impl qobject::BrowserUi {
    pub(super) fn switcher_availability(&self, scope: &str) -> Option<SwitcherAvailability> {
        let rust = self.rust();
        let profile = if rust.profile_bootstrap_pending {
            SourceProfile::Opening
        } else {
            match self.active_profile_privacy() {
                Some(PrivacyKind::Private) => SourceProfile::Private,
                Some(PrivacyKind::Ephemeral) => SourceProfile::Ephemeral,
                Some(PrivacyKind::Normal) if rust.profile_persistence.is_durable() => {
                    SourceProfile::Durable
                }
                _ => SourceProfile::Unavailable,
            }
        };
        SwitcherAvailability::for_sources(
            scope,
            profile,
            &rust.library.storage_library_read_state,
            &rust.session_catalog_read_state,
        )
    }

    pub(super) fn switcher_availability_value(&self, scope: &str) -> Option<serde_json::Value> {
        let status = self.switcher_availability(scope)?;
        Some(serde_json::json!({
            "pending": status.pending,
            "failed": status.failed,
            "retryable": status.retryable,
            "notice": status.notice,
            "detail": status.detail.chars().take(4096).collect::<String>(),
        }))
    }

    pub(super) fn prepare_switcher_sources(mut self: Pin<&mut Self>, scope: &str) -> bool {
        let Some(needs) = SourceNeeds::for_scope(scope) else {
            return false;
        };
        if self.as_ref().rust().profile_bootstrap_pending
            || !self.as_ref().rust().profile_persistence.is_durable()
            || self.as_ref().active_profile_is_transient()
        {
            return true;
        }
        // Failed reads wait for an explicit retry. Query edits must not create
        // an I/O retry loop or erase an actionable failure with a loading state.
        if needs.library
            && self
                .as_ref()
                .rust()
                .library
                .storage_library_read_state
                .permits_query_request()
        {
            self.as_mut().request_storage_library();
        }
        if needs.sessions
            && self
                .as_ref()
                .rust()
                .session_catalog_read_state
                .permits_query_request()
        {
            self.as_mut().request_session_names();
        }
        true
    }

    pub(super) fn publish_switcher_availability(mut self: Pin<&mut Self>, scope: &str) {
        let Some(status) = self.as_ref().switcher_availability(scope) else {
            return;
        };
        self.as_mut().set_switcher_sources_pending(status.pending);
        self.as_mut().set_switcher_sources_failed(status.failed);
        self.as_mut()
            .set_switcher_sources_retryable(status.retryable);
        self.as_mut()
            .set_switcher_sources_notice(QString::from(status.notice));
        self.set_switcher_sources_detail(QString::from(
            status.detail.chars().take(4096).collect::<String>(),
        ));
    }

    pub(super) fn refresh_switcher_sources(mut self: Pin<&mut Self>, scope: &QString) -> bool {
        let scope = scope.to_string();
        let Some(needs) = SourceNeeds::for_scope(&scope) else {
            return false;
        };
        if !self.as_ref().rust().profile_persistence.is_durable()
            || self.as_ref().active_profile_is_transient()
        {
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if needs.library && !this.library.storage_library_read_state.is_pending() {
                this.library.storage_library_dirty.set(true);
                this.library.storage_library_read_state = CatalogReadState::Unrequested;
            }
            if needs.sessions && !this.session_catalog_read_state.is_pending() {
                this.library.session_names_dirty.set(true);
                this.session_catalog_read_state = CatalogReadState::Unrequested;
            }
        }
        self.as_mut().prepare_switcher_sources(&scope);
        self.publish_switcher_availability(&scope);
        true
    }

    pub(super) fn retry_queued_switcher_sources(mut self: Pin<&mut Self>) -> bool {
        // At most one intent per catalog is retained when the coordinator's
        // bounded admission queue is full. It is not another request queue.
        let library =
            self.as_ref().rust().library.storage_library_read_state == CatalogReadState::Queued;
        let sessions = self.as_ref().rust().session_catalog_read_state == CatalogReadState::Queued;
        if library {
            self.as_mut().request_storage_library();
        }
        if sessions {
            self.as_mut().request_session_names();
        }
        (library
            && self.as_ref().rust().library.storage_library_read_state != CatalogReadState::Queued)
            || (sessions
                && self.as_ref().rust().session_catalog_read_state != CatalogReadState::Queued)
    }
}
