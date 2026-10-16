//! Typed userscript inventory projection for QML settings components.

use super::background_workers::UserscriptMutationOutcome;
use super::{CxxQtType, Pin, QString, QStringList, qobject, userscript, userscript_catalog};

impl qobject::BrowserUi {
    pub(super) fn set_userscript_inventory_rows(
        mut self: Pin<&mut Self>,
        scripts: &[userscript::InstalledScript],
    ) {
        self.as_mut().set_userscript_names(
            scripts
                .iter()
                .map(|script| QString::from(&script.name))
                .collect(),
        );
        self.as_mut().set_userscript_enabled_values(
            scripts
                .iter()
                .map(|script| QString::from(script.enabled.to_string()))
                .collect(),
        );
        self.as_mut().set_userscript_page_world_values(
            scripts
                .iter()
                .map(|script| QString::from(script.page_world.to_string()))
                .collect(),
        );
        self.as_mut().set_userscript_action_counts(
            scripts
                .iter()
                .map(|script| QString::from(script.actions.to_string()))
                .collect(),
        );
        self.as_mut().set_userscript_inventory_ready(true);
        self.as_mut()
            .set_userscript_inventory_error(QString::default());
    }

    pub(super) fn userscript_update_failed(
        mut self: Pin<&mut Self>,
        notice: &str,
        detail: &str,
    ) -> bool {
        self.as_mut()
            .set_userscript_install_state(QString::from(format!("error:{detail}")));
        self.set_status_text(QString::from(notice));
        false
    }

    pub(super) fn userscript_inventory_failed(mut self: Pin<&mut Self>, detail: &str) -> bool {
        self.as_mut().set_userscript_inventory_pending(false);
        self.as_mut()
            .set_userscript_inventory_error(QString::from(detail));
        self.set_status_text(QString::from(
            "Installed scripts couldn't be loaded. Refresh in Settings to try again.",
        ));
        false
    }

    pub(super) fn finish_userscript_change(
        mut self: Pin<&mut Self>,
        outcome: UserscriptMutationOutcome,
        state: &str,
        notice: &str,
    ) {
        match outcome {
            UserscriptMutationOutcome::Failed(error) => {
                self.userscript_update_failed(
                    "The script couldn't be updated. See Settings for details.",
                    &error,
                );
            }
            UserscriptMutationOutcome::Applied { inventory } => {
                let notice = match inventory {
                    Ok(scripts) => {
                        self.as_mut().set_userscript_inventory_rows(&scripts);
                        notice.to_owned()
                    }
                    Err(error) => {
                        self.as_mut().userscript_inventory_failed(&error);
                        format!("{notice}. Refresh the list in Settings.")
                    }
                };
                self.as_mut()
                    .set_userscript_install_state(QString::from(state));
                self.set_status_text(QString::from(notice));
            }
        }
    }

    pub(super) fn clear_userscript_inventory_rows(mut self: Pin<&mut Self>) {
        self.as_mut().set_userscript_names(QStringList::default());
        self.as_mut()
            .set_userscript_enabled_values(QStringList::default());
        self.as_mut()
            .set_userscript_page_world_values(QStringList::default());
        self.as_mut()
            .set_userscript_action_counts(QStringList::default());
    }

    pub(super) fn select_userscript_action_subject(
        mut self: Pin<&mut Self>,
        subject: &QString,
    ) -> bool {
        let subject = subject.to_string();
        let values = userscript_catalog::action_presentations(
            self.as_ref()
                .rust()
                .userscript_roots
                .as_ref()
                .map(|roots| roots.config.as_path()),
            &subject,
            self.as_ref().rust().state.is_some(),
            self.as_ref().rust().profile_persistence.is_durable(),
            self.as_ref().active_profile_is_transient(),
        );
        match values {
            Ok(values) => {
                self.as_mut().set_userscript_action_ids(
                    values
                        .iter()
                        .map(|action| QString::from(&action.id))
                        .collect(),
                );
                self.as_mut().set_userscript_action_labels(
                    values
                        .iter()
                        .map(|action| QString::from(&action.label))
                        .collect(),
                );
                self.as_mut().set_userscript_action_availability(
                    values
                        .iter()
                        .map(|action| QString::from(action.available.to_string()))
                        .collect(),
                );
                true
            }
            Err(error) => {
                self.as_mut()
                    .set_userscript_action_ids(QStringList::default());
                self.as_mut()
                    .set_userscript_action_labels(QStringList::default());
                self.as_mut()
                    .set_userscript_action_availability(QStringList::default());
                self.set_status_text(QString::from(format!(
                    "Userscript actions unavailable: {error}"
                )));
                false
            }
        }
    }
}
