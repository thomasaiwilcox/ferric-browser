//! Native manager feedback follows worker completion, independently of the
//! transient browser status line. Receipt IDs let a form retain a newer draft.

use super::{CxxQtType, Pin, QString, QStringList, Uuid, qobject};

#[derive(Clone, Copy)]
pub(super) enum FeedbackLane {
    Profile,
    SessionSave,
    SessionDelete,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum FeedbackState {
    Idle,
    Pending,
    Succeeded,
    Failed,
}

impl qobject::BrowserUi {
    pub(super) fn publish_management_feedback(
        mut self: Pin<&mut Self>,
        lane: FeedbackLane,
        state: FeedbackState,
        message: &str,
        detail: &str,
    ) {
        let id = match state {
            FeedbackState::Idle => QString::default(),
            FeedbackState::Pending => QString::from(Uuid::new_v4().to_string()),
            _ => match lane {
                FeedbackLane::Profile => self.as_ref().rust().profile_action_id.clone(),
                FeedbackLane::SessionSave => self.as_ref().rust().session_save_id.clone(),
                FeedbackLane::SessionDelete => self.as_ref().rust().session_delete_id.clone(),
            },
        };
        let state = QString::from(match state {
            FeedbackState::Idle => "idle",
            FeedbackState::Pending => "pending",
            FeedbackState::Succeeded => "succeeded",
            FeedbackState::Failed => "failed",
        });
        let message = QString::from(message);
        let detail = QString::from(detail.chars().take(4096).collect::<String>());
        // Publish state last: its observer sees a complete receipt.
        match lane {
            FeedbackLane::Profile => {
                self.as_mut().set_profile_action_id(id);
                self.as_mut().set_profile_action_message(message);
                self.as_mut().set_profile_action_detail(detail);
                self.as_mut().set_profile_action_state(state);
            }
            FeedbackLane::SessionSave => {
                self.as_mut().set_session_save_id(id);
                self.as_mut().set_session_save_message(message);
                self.as_mut().set_session_save_detail(detail);
                self.as_mut().set_session_save_state(state);
            }
            FeedbackLane::SessionDelete => {
                self.as_mut().set_session_delete_id(id);
                self.as_mut().set_session_delete_message(message);
                self.as_mut().set_session_delete_detail(detail);
                self.as_mut().set_session_delete_state(state);
            }
        }
    }

    pub(super) fn reset_management_feedback(mut self: Pin<&mut Self>) {
        for lane in [
            FeedbackLane::Profile,
            FeedbackLane::SessionSave,
            FeedbackLane::SessionDelete,
        ] {
            self.as_mut()
                .publish_management_feedback(lane, FeedbackState::Idle, "", "");
        }
        self.as_mut().set_profile_list_error(QString::default());
        self.as_mut().set_profile_preview_error(QString::default());
        self.as_mut()
            .set_session_names_values(QStringList::default());
        self.as_mut().set_session_names_pending(false);
        self.as_mut().set_session_names_error(QString::default());
    }

    pub(super) fn invalidate_session_catalog(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.session_names_dirty.set(true);
        this.session_names_refresh_again |= this.session_names_pending;
    }

    pub(super) fn refresh_profiles_after_mutation(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().profile_values_pending {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .profile_list_refresh_again = true;
        } else {
            self.request_profile_list();
        }
    }
}
