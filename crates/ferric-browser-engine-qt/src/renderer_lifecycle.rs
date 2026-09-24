//! Renderer-process failure and recovery facts at the `WebEngine` boundary.

use super::{CxxQtType, Event, Pin, QString, current_target, qobject};

impl qobject::BrowserUi {
    pub(super) fn note_renderer_process_terminated(
        mut self: Pin<&mut Self>,
        tab_index: i32,
    ) -> bool {
        let tab = if tab_index >= 0 {
            self.as_ref().tab_for_index(tab_index)
        } else {
            self.as_ref().rust().tab
        };
        let Some(target) = current_target(self.as_ref().rust().state.as_ref(), tab) else {
            self.set_status_text(QString::from("Renderer termination target is stale"));
            return false;
        };
        match self
            .as_mut()
            .reduce_event(Event::RendererTerminated { target })
        {
            Ok(_) => {
                self.set_status_text(QString::from("Renderer terminated; recovery required"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Renderer termination was not recorded: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn prepare_renderer_recovery(mut self: Pin<&mut Self>, tab_index: i32) -> bool {
        let tab = if tab_index >= 0 {
            self.as_ref().tab_for_index(tab_index)
        } else {
            self.as_ref().rust().tab
        };
        let Some(target) = current_target(self.as_ref().rust().state.as_ref(), tab) else {
            self.set_status_text(QString::from("Renderer recovery target is stale"));
            return false;
        };
        match self.as_mut().reduce_event(Event::Reload {
            target,
            bypass_cache: false,
        }) {
            Ok(_) => true,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Renderer recovery was not prepared: {error}"
                )));
                false
            }
        }
    }
}
