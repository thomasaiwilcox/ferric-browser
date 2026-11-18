//! A single-use observer of the existing configuration writer. This owns no
//! writer, publication permission, or `QObject` callback.
use ferric_browser_application::ConfigurationRequestCapture;
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub(super) struct SiteDoctorSubject {
    pub(super) id: String,
    pub(super) experiment: String,
    pub(super) origin: String,
    pub(super) before: Vec<String>,
    pub(super) after: Vec<String>,
    pub(super) owner: ConfigurationRequestCapture,
}

impl SiteDoctorSubject {
    pub(super) fn matches(&self, proposal: Option<&Value>) -> bool {
        proposal.is_some_and(|proposal| {
            proposal.get("id").and_then(Value::as_str) == Some(self.id.as_str())
                && proposal.get("experiment_id").and_then(Value::as_str)
                    == Some(self.experiment.as_str())
                && proposal.get("origin").and_then(Value::as_str) == Some(self.origin.as_str())
                && proposal.get("kind").and_then(Value::as_str) == Some("blocker-exception")
                && proposal.pointer("/mutation/key").and_then(Value::as_str)
                    == Some("blocking.bypass_sites")
                && proposal.pointer("/mutation/layer").and_then(Value::as_str)
                    == Some("runtime-overrides")
                && proposal.pointer("/mutation/before") == Some(&serde_json::json!(self.before))
                && proposal.pointer("/mutation/after") == Some(&serde_json::json!(self.after))
        })
    }
}

pub(super) use super::configuration_completion::ConfigurationSaveOutcome as SiteDoctorSaveOutcome;

type Receipt = Arc<Mutex<Option<SiteDoctorSaveOutcome>>>;

pub(super) fn proposal_projection(rust: &crate::BrowserUiRust) -> Option<Value> {
    let mut proposal = rust.last_site_doctor_result.clone()?;
    if proposal.get("state").and_then(Value::as_str) == Some("pending-confirmation")
        && proposal
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| {
                crate::browser_ui_configuration_preparation::site_doctor_is_preparing(rust, id)
            })
    {
        proposal["state"] = Value::String("preparing".into());
    }
    Some(proposal)
}

pub(super) struct SiteDoctorFix {
    pub(super) subject: Arc<SiteDoctorSubject>,
    receipt: Receipt,
    pub(super) policy: Option<(u64, ConfigurationRequestCapture, bool)>,
}

/// Intentionally not Clone: exactly one phase owns completion at a time.
pub(super) struct SiteDoctorCompletion {
    pub(super) subject: Arc<SiteDoctorSubject>,
    receipt: Receipt,
    unresolved: bool,
    admitted: bool,
}

impl SiteDoctorFix {
    pub(super) fn new(subject: SiteDoctorSubject) -> (Self, SiteDoctorCompletion) {
        let subject = Arc::new(subject);
        let receipt = Arc::new(Mutex::new(None));
        (
            Self {
                subject: subject.clone(),
                receipt: receipt.clone(),
                policy: None,
            },
            SiteDoctorCompletion {
                subject,
                receipt,
                unresolved: true,
                admitted: false,
            },
        )
    }

    pub(super) fn take_receipt(&mut self) -> Option<SiteDoctorSaveOutcome> {
        self.receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

impl SiteDoctorCompletion {
    pub(super) const fn is_admitted(&self) -> bool {
        self.admitted
    }

    pub(super) fn admitted(&mut self) {
        self.admitted = true;
    }

    pub(super) fn is_current(&self, rust: &crate::BrowserUiRust) -> bool {
        rust.view_alive
            && !rust.ipc_shutdown_gate
            && rust.profile_persistence.is_durable()
            && self.subject.matches(rust.last_site_doctor_result.as_ref())
            && rust.state.as_ref().is_some_and(|state| {
                state.configuration_request_is_current(&self.subject.owner)
                    && state.configuration().blocking.bypass_sites == self.subject.before
            })
    }

    pub(super) fn finish(mut self, outcome: SiteDoctorSaveOutcome) {
        self.unresolved = false;
        *self
            .receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(outcome);
    }
}

impl Drop for SiteDoctorCompletion {
    fn drop(&mut self) {
        if self.unresolved {
            *self
                .receipt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(if self.admitted {
                SiteDoctorSaveOutcome::Abandoned
            } else {
                SiteDoctorSaveOutcome::Failed
            });
        }
    }
}

use crate::{CxxQtType, Pin, QString, qobject};

impl qobject::BrowserUi {
    pub(super) fn poll_site_doctor_fix(mut self: Pin<&mut Self>) {
        let Some(mut fix) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .site_doctor_fix
            .take()
        else {
            return;
        };
        if !fix
            .subject
            .matches(self.as_ref().rust().last_site_doctor_result.as_ref())
        {
            return;
        }
        if let Some((generation, owner, uncertain)) = fix.policy.as_ref() {
            let current = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .is_some_and(|state| state.configuration_request_is_current(owner));
            let pending = self
                .as_ref()
                .rust()
                .configuration_io
                .network_policy_request
                .as_ref()
                .is_some_and(|request| request.generation == *generation);
            if !current || !pending {
                self.publish_site_doctor_fix(&fix.subject, "saved-policy-inactive", Some(true), *uncertain,
                    "Site Doctor fix saved; its blocker update is no longer current. Review the settings before reloading blocking rules.");
                return;
            }
            self.as_mut().rust_mut().as_mut().get_mut().site_doctor_fix = Some(fix);
            return;
        }
        let Some(receipt) = fix.take_receipt() else {
            self.as_mut().rust_mut().as_mut().get_mut().site_doctor_fix = Some(fix);
            return;
        };
        match receipt {
            SiteDoctorSaveOutcome::Published { owner, durability_uncertain } => {
                let current = self.as_ref().rust().state.as_ref().is_some_and(|state| {
                    state.configuration_request_is_current(&owner)
                        && state.configuration().blocking.bypass_sites == fix.subject.after
                });
                if !current {
                    self.publish_site_doctor_fix(&fix.subject, "saved-policy-inactive", Some(true), durability_uncertain,
                        "Site Doctor fix saved; the active configuration changed before its blocker update.");
                    return;
                }
                match self.as_mut().request_network_policy(true) {
                    Ok(generation) => {
                        fix.policy = Some((generation, *owner, durability_uncertain));
                        let subject = fix.subject.clone();
                        self.as_mut().rust_mut().as_mut().get_mut().site_doctor_fix = Some(fix);
                        self.as_mut().runtime_work_available();
                        self.publish_site_doctor_fix(&subject, "saved-policy-pending", Some(true), durability_uncertain,
                            "Site Doctor fix saved; blocker update pending.");
                    }
                    Err(_) => self.publish_site_doctor_fix(&fix.subject, "saved-policy-failed", Some(true), durability_uncertain,
                        "Site Doctor fix saved; blocking rules could not be updated. Review the settings before reloading blocking rules."),
                }
            }
            SiteDoctorSaveOutcome::SavedInactive { durability_uncertain } => self.publish_site_doctor_fix(
                &fix.subject, "saved-configuration-inactive", Some(true), durability_uncertain,
                "Site Doctor fix saved; its configuration could not be activated. Review and reload the configuration before testing this fix."),
            SiteDoctorSaveOutcome::Failed => self.publish_site_doctor_fix(&fix.subject, "failed", Some(false), false,
                "Site Doctor fix failed before a successful save receipt. Current blocking rules were retained."),
            SiteDoctorSaveOutcome::Abandoned => self.publish_site_doctor_fix(&fix.subject, "completion-lost", None, false,
                "Site Doctor fix completion is unavailable. Check the saved configuration before trying again."),
        }
    }

    pub(super) fn finish_site_doctor_policy(
        mut self: Pin<&mut Self>,
        generation: u64,
        installed: bool,
    ) {
        let matches = self
            .as_ref()
            .rust()
            .site_doctor_fix
            .as_ref()
            .is_some_and(|fix| {
                fix.policy
                    .as_ref()
                    .is_some_and(|(pending, _, _)| *pending == generation)
            });
        if !matches {
            return;
        }
        let Some(fix) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .site_doctor_fix
            .take()
        else {
            return;
        };
        let Some((_, owner, uncertain)) = fix.policy else {
            return;
        };
        let current = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| state.configuration_request_is_current(&owner));
        if installed && current {
            self.publish_site_doctor_fix(
                &fix.subject,
                "applied",
                Some(true),
                uncertain,
                "Site Doctor blocker exception saved; matching blocking rules are active.",
            );
        } else {
            self.publish_site_doctor_fix(&fix.subject, "saved-policy-failed", Some(true), uncertain,
                "Site Doctor fix saved; its blocker update failed or was replaced. Review the settings before reloading blocking rules.");
        }
    }

    fn publish_site_doctor_fix(
        mut self: Pin<&mut Self>,
        subject: &SiteDoctorSubject,
        state: &str,
        saved: Option<bool>,
        uncertain: bool,
        message: &str,
    ) {
        // No observable calls between the identity check and JSON publication.
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !subject.matches(this.last_site_doctor_result.as_ref())
                || !this.state.as_ref().is_some_and(|state| {
                    state.configuration_request_owner_is_current(&subject.owner)
                })
            {
                return;
            }
            let Some(object) = this
                .last_site_doctor_result
                .as_mut()
                .and_then(Value::as_object_mut)
            else {
                return;
            };
            object.insert("state".into(), Value::String(state.into()));
            object.insert("saved".into(), saved.map_or(Value::Null, Value::Bool));
            object.insert("durability_uncertain".into(), Value::Bool(uncertain));
        }
        let message = if uncertain {
            format!("{message} Saving may not survive a system crash.")
        } else {
            message.into()
        };
        self.set_status_text(QString::from(message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> SiteDoctorSubject {
        let rust = crate::BrowserUiRust::default();
        SiteDoctorSubject {
            id: "fix".into(),
            experiment: "test".into(),
            origin: "https://example.test".into(),
            before: vec![],
            after: vec!["example.test".into()],
            owner: rust
                .state
                .as_ref()
                .unwrap()
                .capture_configuration_request()
                .unwrap(),
        }
    }

    #[test]
    fn delayed_receipt_is_consumed_once_and_drop_cannot_replace_it() {
        let (mut fix, completion) = SiteDoctorFix::new(subject());
        assert!(fix.take_receipt().is_none());
        completion.finish(SiteDoctorSaveOutcome::SavedInactive {
            durability_uncertain: true,
        });
        assert!(matches!(
            fix.take_receipt(),
            Some(SiteDoctorSaveOutcome::SavedInactive {
                durability_uncertain: true
            })
        ));
        assert!(fix.take_receipt().is_none());
    }

    #[test]
    fn dropped_validation_reports_pre_write_failure() {
        let (mut fix, completion) = SiteDoctorFix::new(subject());
        drop(completion);
        assert!(matches!(
            fix.take_receipt(),
            Some(SiteDoctorSaveOutcome::Failed)
        ));
        assert!(fix.take_receipt().is_none());
    }

    #[test]
    fn admitted_drop_preserves_uncertainty() {
        let (mut fix, mut completion) = SiteDoctorFix::new(subject());
        completion.admitted();
        drop(completion);
        assert!(matches!(
            fix.take_receipt(),
            Some(SiteDoctorSaveOutcome::Abandoned)
        ));
    }

    #[test]
    fn proposal_identity_includes_experiment_and_exact_mutation() {
        let subject = subject();
        let mut proposal = serde_json::json!({"id":"fix", "experiment_id":"test",
            "kind":"blocker-exception", "origin":"https://example.test", "mutation":{
            "layer":"runtime-overrides", "key":"blocking.bypass_sites", "before":[], "after":["example.test"]}});
        assert!(subject.matches(Some(&proposal)));
        proposal["experiment_id"] = serde_json::json!("new");
        assert!(!subject.matches(Some(&proposal)));
        proposal["experiment_id"] = serde_json::json!("test");
        proposal["mutation"]["after"] = serde_json::json!(["other.test"]);
        assert!(!subject.matches(Some(&proposal)));
    }
}
