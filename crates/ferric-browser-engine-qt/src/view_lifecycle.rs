//! `WebEngine` view lifecycle callbacks and their scoped cleanup.

use super::{CxxQtType, Pin, QString, qobject};

impl qobject::BrowserUi {
    pub(super) fn view_closed(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().core_mode == super::Mode::Grid {
            self.as_mut()
                .spatial_invalidated(&QString::from("view-changed"));
        }
        self.as_mut().clear_hint_session_state();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.focus_observations.clear();
            this.focus_suppressions.clear();
        }
        if let Some(id) = self
            .as_ref()
            .rust()
            .active_site_experiment
            .as_ref()
            .map(|experiment| experiment.id.clone())
        {
            let _ = self
                .as_mut()
                .finish_site_doctor_experiment(&QString::from(id), false);
        }
        if self.as_ref().rust().checkpoint.restore_pending.is_none()
            && let Some(path) = self.as_ref().rust().session_path.clone()
        {
            let _ = self
                .as_mut()
                .request_session_snapshot_save("last-session", path, true);
        }
        self.as_mut().set_view_alive(false);
        self.set_load_state(QString::from("closed"));
    }
}
