//! One completion slot follows the existing validation/writer pipeline.
use ferric_browser_application::{
    ConfigurationRequestCapture, SitePolicyApplyOutcome, SitePolicyDraftApply,
};

pub(super) enum ConfigurationSaveOutcome {
    Published {
        owner: Box<ConfigurationRequestCapture>,
        durability_uncertain: bool,
    },
    SavedInactive {
        durability_uncertain: bool,
    },
    Failed,
    Abandoned,
}

pub(super) enum ConfigurationCompletion {
    SiteDoctor(super::site_doctor_completion::SiteDoctorCompletion),
    SitePolicy {
        observer: Box<SitePolicyDraftApply>,
        operation_id: Option<String>,
    },
}

impl ConfigurationCompletion {
    pub(super) fn is_admitted(&self) -> bool {
        match self {
            Self::SiteDoctor(observer) => observer.is_admitted(),
            Self::SitePolicy { observer, .. } => observer.is_admitted(),
        }
    }
    pub(super) fn admitted(&mut self) {
        match self {
            Self::SiteDoctor(observer) => observer.admitted(),
            Self::SitePolicy { observer, .. } => observer.admitted(),
        }
    }
    pub(super) fn is_current(&self, rust: &crate::BrowserUiRust) -> bool {
        match self {
            Self::SiteDoctor(observer) => observer.is_current(rust),
            Self::SitePolicy { observer, .. } => {
                rust.view_alive
                    && !rust.ipc_shutdown_gate
                    && rust
                        .state
                        .as_ref()
                        .is_some_and(|state| observer.is_current(state))
            }
        }
    }
    fn finish(
        self,
        rust: &crate::BrowserUiRust,
        outcome: ConfigurationSaveOutcome,
    ) -> Option<super::browser_ui_site_policy::PendingSitePolicyActivation> {
        match self {
            Self::SiteDoctor(observer) => {
                observer.finish(outcome);
                None
            }
            Self::SitePolicy {
                observer,
                operation_id,
            } => {
                let state = rust.state.as_ref();
                let outcome = match outcome {
                    ConfigurationSaveOutcome::Published {
                        owner,
                        durability_uncertain,
                    } if state
                        .is_some_and(|state| state.configuration_request_is_current(&owner)) =>
                    {
                        SitePolicyApplyOutcome::Published {
                            durability_uncertain,
                        }
                    }
                    ConfigurationSaveOutcome::Published {
                        durability_uncertain,
                        ..
                    }
                    | ConfigurationSaveOutcome::SavedInactive {
                        durability_uncertain,
                    } => SitePolicyApplyOutcome::SavedInactive {
                        durability_uncertain,
                    },
                    ConfigurationSaveOutcome::Failed => SitePolicyApplyOutcome::Failed,
                    ConfigurationSaveOutcome::Abandoned => {
                        drop(observer);
                        return None;
                    }
                };
                observer.finish(state, outcome).map(|observer| {
                    super::browser_ui_site_policy::PendingSitePolicyActivation {
                        observer,
                        operation_id,
                    }
                })
            }
        }
    }
}

use crate::{CxxQtType, Pin, qobject};
impl qobject::BrowserUi {
    pub(super) fn finish_configuration_completion(
        mut self: Pin<&mut Self>,
        completion: ConfigurationCompletion,
        outcome: ConfigurationSaveOutcome,
    ) {
        let activation = completion.finish(self.as_ref().rust(), outcome);
        if let Some(activation) = activation {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .site_policy_activation = Some(activation);
            self.runtime_work_available();
        }
    }
}
