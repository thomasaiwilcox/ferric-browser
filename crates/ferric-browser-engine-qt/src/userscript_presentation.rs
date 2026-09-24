//! Typed userscript inventory projection for QML settings components.

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
