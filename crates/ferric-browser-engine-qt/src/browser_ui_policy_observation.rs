//! Eligibility of actual engine observations. A rejected navigation request is
//! different: no view changed, so request validation must leave this untouched.
use super::{CxxQtType, Mode, Pin, qobject};

impl qobject::BrowserUi {
    pub(super) fn policy_observation_identity_is_current(
        &self,
        index: i32,
        tab: &str,
        document: &str,
        epoch: i32,
    ) -> bool {
        self.target_for_index(index).is_some_and(|target| {
            tab == target.tab.to_string()
                && epoch == self.rust().input_epoch
                && !document.is_empty()
                && self
                    .rust()
                    .navigation
                    .page_document_identities
                    .get(target.tab)
                    .is_none_or(|known| known == document)
        })
    }

    pub(super) fn recover_policy_observation_from_probe(
        mut self: Pin<&mut Self>,
        index: i32,
        tab: &str,
        url: &str,
        document: &str,
        epoch: i32,
    ) {
        let current = self
            .as_ref()
            .policy_observation_identity_is_current(index, tab, document, epoch)
            && self.as_ref().rust().active_tab_index == index
            && self.as_ref().rust().current_url.to_string() == url
            && self
                .as_ref()
                .get_ref()
                .target_for_index(index)
                .is_some_and(|target| {
                    self.as_ref().rust().tab == Some(target.tab)
                        && self
                            .as_ref()
                            .rust()
                            .navigation
                            .page_document_identities
                            .proves(target.tab, document)
                });
        if current && self.as_mut().accept_observed_policy_context_for(index) {
            self.request_interaction_policy_refresh();
        }
    }

    pub(super) fn reject_observed_policy_context_for(mut self: Pin<&mut Self>, index: i32) {
        let Some(target) = self.as_ref().get_ref().target_for_index(index) else {
            return;
        };
        let affected = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let (Some(state), Some(window)) = (this.state.as_mut(), this.window) else {
                return;
            };
            let Some(observation) = state.capture_site_policy_observation(window, target) else {
                return;
            };
            let affected = state.reject_observed_interaction_context(&observation);
            if affected {
                this.interaction.compiled_interaction_revision = None;
                this.interaction.interaction_policy_refresh_pending = false;
                this.replace_mixed_policy(
                    super::mixed_policy_runtime::MixedPolicyState::Unavailable(
                        "the selected engine policy context was rejected".into(),
                    ),
                );
            }
            if this
                .navigation
                .pending_same_document_pass_through
                .as_ref()
                .is_some_and(|pending| pending.tab == target.tab)
            {
                this.navigation.pending_same_document_pass_through = None;
            }
            affected
        };
        self.as_mut().retire_native_document_work(target);
        self.as_mut().clear_focus_observation_state(index);
        if affected {
            if self.as_ref().rust().core_mode == Mode::Grid {
                self.as_mut()
                    .spatial_invalidated(&super::QString::from("target-changed"));
            }
            self.as_mut().reset_editing_session();
            self.note_hint_configuration_change();
        }
    }

    /// Called only for validated actual engine events or a fully current
    /// document probe. Tab selection and requested addresses cannot release it.
    pub(super) fn accept_observed_policy_context_for(mut self: Pin<&mut Self>, index: i32) -> bool {
        let Some(target) = self.as_ref().get_ref().target_for_index(index) else {
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let (Some(state), Some(window)) = (this.state.as_mut(), this.window) else {
            return false;
        };
        let Some(observation) = state.capture_site_policy_observation(window, target) else {
            return false;
        };
        state.accept_observed_interaction_context(&observation)
    }
}
