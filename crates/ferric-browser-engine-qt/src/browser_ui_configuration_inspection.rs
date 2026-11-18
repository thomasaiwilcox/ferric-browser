//! Captures authoritative staged inputs without polling or borrowing the
//! mutation preparation slot. Called only by the queued read or IPC timer.
use crate::browser_ui_state::PendingSelectorAction;
use crate::{BrowserUiRust, CxxQtType, IpcQueryError, qobject};
use ferric_browser_application::{
    ConfigurationInspectionCandidate, SettingInspection, SettingInspectionPending,
    SettingInspectionRequest,
};
use ferric_browser_config::ConfigurationApplyBoundary;

fn selector_current(
    rust: &BrowserUiRust,
    pending: &crate::browser_ui_state::PendingSelectorValidation,
) -> bool {
    let io = &rust.configuration_io;
    rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.ipc_shutdown_gate
        && pending.generation == io.selector_generation
        && pending.expected_revision == io.selector_config_revision
        && pending.owner.as_ref().is_some_and(|owner| {
            rust.state
                .as_ref()
                .is_some_and(|state| state.configuration_request_is_current(owner))
        })
        && pending.operation_id.as_ref().is_none_or(|id| {
            rust.operation(id)
                .is_some_and(|operation| !operation.state.is_terminal())
        })
        && pending.origin.as_ref().map_or(
            io.selector_origin_generation == pending.generation,
            |origin| origin.is_current(rust),
        )
        && pending.started_at.is_none_or(|started| {
            started.elapsed() < crate::browser_ui_selector_config::SELECTOR_VALIDATION_TIMEOUT
        })
}

fn candidate(rust: &BrowserUiRust) -> Result<Option<ConfigurationInspectionCandidate>, String> {
    let state = rust
        .state
        .as_ref()
        .ok_or("configuration owner is unavailable")?;
    if state.configuration_write_pending() {
        return state
            .configuration_write_inspection_candidate()
            .map(Some)
            .ok_or_else(|| "the staged writer belongs to a retired configuration".into());
    }
    if let Some(pending) = &rust.configuration_io.pending_context_reload {
        return pending.inspection_candidate(state).map(Some);
    }
    let Some(pending) = &rust.configuration_io.pending_selector_validation else {
        return Ok(None);
    };
    let (layers, boundary) = match &pending.action {
        PendingSelectorAction::CommittedPeer { prepared }
        | PendingSelectorAction::RuntimeConfig { prepared, .. } => {
            if !selector_current(rust, pending) {
                return Err("staged shared settings validation is no longer current".into());
            }
            let candidate = prepared.inspection_candidate();
            return state
                .configuration_inspection_candidate_is_current(&candidate)
                .then_some(Some(candidate))
                .ok_or_else(|| "staged shared settings belong to a retired configuration".into());
        }
        PendingSelectorAction::Reload { publication } => {
            (&publication.layers, ConfigurationApplyBoundary::Reload)
        }
        PendingSelectorAction::Prepare(_)
        | PendingSelectorAction::Check { .. }
        | PendingSelectorAction::Profile { .. } => return Ok(None),
    };
    if !selector_current(rust, pending) {
        return Err("staged selector validation is no longer current".into());
    }
    state
        .capture_configuration_inspection_candidate(
            pending
                .owner
                .as_ref()
                .ok_or("staged validation has no captured owner")?,
            layers,
            boundary,
        )
        .map(Some)
}

impl qobject::BrowserUi {
    pub(super) fn inspect_configuration_interaction(
        &self,
        request: &ferric_browser_application::InteractionInspectionRequest,
    ) -> Result<ferric_browser_application::InteractionInspection, String> {
        let rust = self.rust();
        if !rust.view_alive || rust.profile_bootstrap_pending || rust.ipc_shutdown_gate {
            return Err("configuration owner is not ready".into());
        }
        let state = rust.state.as_ref().ok_or("application is unavailable")?;
        let inputs = state.configuration_inspection_inputs();
        let captured = candidate(rust);
        let mut report = match &captured {
            Ok(Some(candidate)) => inputs.inspect_interaction_with_candidate(request, candidate)?,
            Ok(None) => inputs.inspect_interaction(request)?,
            Err(error) => {
                let mut report = inputs.inspect_interaction(request)?;
                report.desired = Err(error.clone());
                report.pending = SettingInspectionPending::Unknown;
                report
            }
        };
        if !state.configuration_inspection_is_current(&inputs) {
            return Err("configuration changed during inspection".into());
        }
        if matches!(captured, Ok(Some(_))) && candidate(rust).is_err() {
            report.desired = Err("staged configuration expired during inspection".into());
            report.pending = SettingInspectionPending::Unknown;
        }
        let desired = captured
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map_or(inputs.sources(), ConfigurationInspectionCandidate::sources);
        for (value, layers) in [
            (&mut report.active, inputs.sources().reload_layers()),
            (&mut report.desired, desired.desired_layers()),
        ] {
            let selectors = crate::selector_validation::selector_validation_items(layers)
                .into_iter()
                .filter(|item| item.family == "interaction")
                .map(|item| item.selector)
                .collect::<Vec<_>>();
            let receipts = &rust.configuration_io.selector_receipts;
            let receipt = receipts.matching(state, layers);
            if let Ok(value) = value
                && receipts.proves(receipt.as_ref(), state, layers, &selectors)
            {
                value.validation.engine_css =
                    ferric_browser_application::InspectionValidationStage::Passed;
            }
        }
        Ok(report)
    }

    pub(super) fn inspect_configuration_setting(
        &self,
        request: &SettingInspectionRequest,
    ) -> Result<SettingInspection, IpcQueryError> {
        let rust = self.rust();
        if !rust.view_alive || rust.profile_bootstrap_pending || rust.ipc_shutdown_gate {
            return Err(IpcQueryError::Busy(
                "configuration owner is not ready".into(),
            ));
        }
        let state = rust
            .state
            .as_ref()
            .ok_or_else(|| IpcQueryError::Unavailable("application is unavailable".into()))?;
        let inputs = state.configuration_inspection_inputs();
        if !state.configuration_inspection_is_current(&inputs) {
            return Err(IpcQueryError::Busy(
                "configuration owner is not current".into(),
            ));
        }
        let captured = candidate(rust);
        let mut report = match &captured {
            Ok(Some(candidate)) => inputs.inspect_setting_with_candidate(request, candidate),
            Ok(None) => inputs.inspect_setting(request),
            Err(error) => {
                let mut report = inputs
                    .inspect_setting(request)
                    .map_err(IpcQueryError::Unavailable)?;
                report.desired = Err(error.clone());
                report.pending = SettingInspectionPending::Unknown;
                apply_css_validation(rust, state, &inputs, None, &mut report);
                return Ok(report);
            }
        }
        .map_err(IpcQueryError::Busy)?;
        // Composition can cross the CSS timeout while the UI thread is busy.
        // Do not certify an expired slot, and never mutate it from a read.
        if !state.configuration_write_pending()
            && rust.configuration_io.pending_context_reload.is_none()
            && let Some(pending) = &rust.configuration_io.pending_selector_validation
            && matches!(
                pending.action,
                PendingSelectorAction::RuntimeConfig { .. }
                    | PendingSelectorAction::Reload { .. }
                    | PendingSelectorAction::CommittedPeer { .. }
            )
            && !selector_current(rust, pending)
        {
            report.desired = Err("staged selector validation expired during inspection".into());
            report.pending = SettingInspectionPending::Unknown;
        }
        apply_css_validation(
            rust,
            state,
            &inputs,
            captured.as_ref().ok().and_then(Option::as_ref),
            &mut report,
        );
        Ok(report)
    }
}

fn apply_css_validation(
    rust: &BrowserUiRust,
    state: &ferric_browser_application::BrowserApplication,
    inputs: &ferric_browser_application::ConfigurationInspectionInputs,
    candidate: Option<&ConfigurationInspectionCandidate>,
    report: &mut SettingInspection,
) {
    // Only selector leaves can claim CSS syntax evidence. Ordinary settings,
    // absent/empty values and effective-combination failures remain unchanged.
    if ferric_browser_config::selector_setting_family(&report.key).is_none() {
        return;
    }
    let receipts = &rust.configuration_io.selector_receipts;
    let active_layers = inputs.sources().reload_layers();
    let desired_layers = candidate
        .map_or(inputs.sources(), ConfigurationInspectionCandidate::sources)
        .desired_layers();
    let active = receipts.matching(state, active_layers);
    let published_desired = receipts.matching(state, desired_layers);
    let staged = if state.configuration_write_pending() {
        rust.configuration_io
            .pending_configuration_publication
            .as_ref()
            .and_then(|pending| {
                (pending.layers == *desired_layers)
                    .then_some(pending.selectors.receipt.as_ref())
                    .flatten()
            })
    } else {
        rust.configuration_io
            .pending_context_reload
            .as_ref()
            .and_then(|pending| {
                let (layers, selectors) = pending.selectors();
                (*layers == *desired_layers)
                    .then_some(selectors.receipt.as_ref())
                    .flatten()
            })
    };
    for (value, layers, receipt) in [
        (&mut report.active, active_layers, active.as_ref()),
        (
            &mut report.desired,
            desired_layers,
            staged.or(published_desired.as_ref()),
        ),
    ] {
        if let Ok(value) = value
            && let Some(raw) = value.value.as_ref()
            && let Ok(policy) = raw
                .clone()
                .try_into::<ferric_browser_config::HintSelectorPolicy>()
            && receipts.proves(receipt, state, layers, &policy.selectors)
        {
            value.validation.engine_css =
                ferric_browser_application::InspectionValidationStage::Passed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_ui_state::PendingSelectorValidation;

    fn held() -> BrowserUiRust {
        let mut rust = BrowserUiRust::default();
        rust.view_alive = true;
        rust.profile_bootstrap_pending = false;
        rust.configuration_io.selector_generation = 8;
        rust.configuration_io.selector_origin_generation = 8;
        rust.admit_operation("held".into()).unwrap();
        rust.set_operation(
            "held".into(),
            crate::OperationUpdate::running("settings validation or persistence"),
        );
        rust.configuration_io.pending_selector_validation = Some(PendingSelectorValidation {
            completion: None,
            origin: None,
            operation_id: Some("held".into()),
            setting_feedback: None,
            generation: 8,
            expected_revision: rust.configuration_io.selector_config_revision,
            owner: rust.state.as_ref().unwrap().capture_configuration_request(),
            started_at: None,
            fingerprint: String::new(),
            validation: None,
            action: PendingSelectorAction::RuntimeConfig {
                post_action: None,
                prepared: Box::new(
                    rust.state
                        .as_ref()
                        .unwrap()
                        .prepare_configuration_update(
                            rust.state.as_ref().unwrap().configuration_layers().clone(),
                        )
                        .unwrap(),
                ),
                status: String::new(),
                persist: false,
            },
        });
        rust
    }

    #[test]
    fn legitimate_preparation_origin_and_css_generation_are_separate() {
        let mut rust = held();
        assert!(candidate(&rust).unwrap().is_some());
        rust.configuration_io.selector_origin_generation = 7;
        assert!(candidate(&rust).is_err());
        rust.configuration_io
            .pending_selector_validation
            .as_mut()
            .unwrap()
            .origin = Some(
            crate::browser_ui_configuration_preparation::ConfigurationRequestOrigin {
                generation: 7,
                operation_id: Some("held".into()),
                setting_feedback: None,
                handoff: None,
            },
        );
        assert!(candidate(&rust).unwrap().is_some());
        rust.configuration_io.selector_generation = 9;
        assert!(candidate(&rust).is_err());
    }

    #[test]
    fn stale_fingerprint_timeout_or_cancelled_operation_is_not_an_absent_candidate() {
        let mut rust = held();
        rust.configuration_io.selector_config_revision += 1;
        assert!(candidate(&rust).is_err());
        rust.configuration_io.selector_config_revision -= 1;
        rust.configuration_io
            .pending_selector_validation
            .as_mut()
            .unwrap()
            .started_at = std::time::Instant::now().checked_sub(std::time::Duration::from_secs(6));
        assert!(candidate(&rust).is_err());
        rust.configuration_io
            .pending_selector_validation
            .as_mut()
            .unwrap()
            .started_at = None;
        rust.state
            .as_mut()
            .unwrap()
            .operations_mut()
            .cancel("held")
            .unwrap();
        assert!(candidate(&rust).is_err());
        assert!(rust.configuration_io.pending_selector_validation.is_some());
    }
}
