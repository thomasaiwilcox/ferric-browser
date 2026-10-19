//! Translation between runtime effects and the Qt adapter's effect executor.
//!
//! This module deliberately owns no browser state. It gives the adapter its
//! sole mutation path into the runtime and converts only the effects that Qt
//! must execute after the reducer has accepted an input.

use super::{
    CxxQtType, Diagnostic, Effect, Event, Pin, RuntimeCommand, RuntimeDispatch, RuntimeEffect,
    RuntimeInput, elapsed_ms, publish_ipc_event, qobject,
};

impl qobject::BrowserUi {
    pub(super) fn reduce_event(
        mut self: Pin<&mut Self>,
        event: Event,
    ) -> Result<Vec<Effect>, String> {
        self.as_mut()
            .handle_runtime_input(RuntimeInput::EngineFact(event))
    }

    pub(super) fn dispatch_runtime(
        self: Pin<&mut Self>,
        dispatch: RuntimeDispatch,
    ) -> Result<Vec<Effect>, String> {
        self.dispatch_runtime_public(dispatch)
            .map_err(|error| error.to_string())
    }

    pub(super) fn dispatch_runtime_public(
        mut self: Pin<&mut Self>,
        dispatch: RuntimeDispatch,
    ) -> Result<Vec<Effect>, super::PublicError> {
        self.as_mut()
            .handle_runtime_input_public(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                dispatch,
            ))))
    }

    pub(super) fn handle_runtime_input(
        self: Pin<&mut Self>,
        input: RuntimeInput,
    ) -> Result<Vec<Effect>, String> {
        self.handle_runtime_input_public(input)
            .map_err(|error| error.to_string())
    }

    fn handle_runtime_input_public(
        mut self: Pin<&mut Self>,
        input: RuntimeInput,
    ) -> Result<Vec<Effect>, super::PublicError> {
        let result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or_else(|| {
                    super::PublicError::new(
                        ferric_browser_ipc::ErrorCode::NotFound,
                        "The browser state is unavailable.",
                        "runtime owner unavailable",
                    )
                })
                .and_then(|runtime| {
                    runtime.dispatch_runtime(input).map_err(|error| {
                        super::PublicError::new(
                            error.code().into(),
                            error.user_message(),
                            error.diagnostic_context(),
                        )
                    })
                })
        };
        match result {
            Ok(effects) => Ok(self.observe_runtime_effects(effects)),
            Err(error) => {
                self.as_mut().refresh_ipc_owner();
                Err(error)
            }
        }
    }

    /// Completes application-owned runtime changes through the same checkpoint
    /// and IPC observation path as ordinary reducer inputs.
    pub(super) fn observe_runtime_effects(
        mut self: Pin<&mut Self>,
        effects: Vec<RuntimeEffect>,
    ) -> Vec<Effect> {
        let accepted = !effects.iter().any(|effect| {
            matches!(
                effect,
                RuntimeEffect::Diagnostic(Diagnostic::IgnoredStaleTarget { .. })
            )
        }) && effects
            .iter()
            .any(|effect| matches!(effect, RuntimeEffect::AppliedEvent(_)));
        if accepted {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.checkpoint.mark_dirty(elapsed_ms(this.binding_clock));
            for effect in &effects {
                if let RuntimeEffect::Observed(observation) = effect {
                    publish_ipc_event(
                        &mut this.ipc_sequence,
                        super::runtime_event_projection::ipc_event_type(&observation.event),
                        super::runtime_event_projection::ipc_event_payload(observation),
                    );
                }
            }
        }
        self.as_mut().refresh_ipc_owner();
        effects
            .into_iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::Engine(effect) => Some(Effect::Engine(effect)),
                RuntimeEffect::Storage(effect) => Some(Effect::Persist(effect)),
                RuntimeEffect::Diagnostic(effect) => Some(Effect::Diagnostic(effect)),
                RuntimeEffect::AppliedEvent(_)
                | RuntimeEffect::Observed(_)
                | RuntimeEffect::UiModelChanged { .. }
                | RuntimeEffect::WorkerObserved { .. } => None,
            })
            .collect()
    }
}
