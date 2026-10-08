//! Translation between runtime effects and the Qt adapter's effect executor.
//!
//! This module deliberately owns no browser state. It gives the adapter its
//! sole mutation path into the runtime and converts only the effects that Qt
//! must execute after the reducer has accepted an input.

use super::{
    BrowserApplication, CxxQtType, Diagnostic, Effect, EngineEffect, Event, EventNotification, Pin,
    RuntimeCommand, RuntimeDispatch, RuntimeEffect, RuntimeInput, elapsed_ms, ipc_instance_id,
    publish_event, qobject,
};

pub(super) fn checkpoint_required(effects: &[RuntimeEffect]) -> bool {
    !effects.iter().any(|effect| {
        matches!(
            effect,
            RuntimeEffect::Diagnostic(Diagnostic::IgnoredStaleTarget { .. })
        )
    }) && effects.iter().any(|effect| {
        matches!(effect, RuntimeEffect::AppliedEvent(event) if !matches!(event, Event::VersioningRequest(_)))
    })
}

impl qobject::BrowserUi {
    pub(super) fn reduce_event(
        mut self: Pin<&mut Self>,
        event: Event,
    ) -> Result<Vec<Effect>, String> {
        self.as_mut()
            .handle_runtime_input(RuntimeInput::EngineFact(event))
    }

    pub(super) fn dispatch_runtime(
        mut self: Pin<&mut Self>,
        dispatch: RuntimeDispatch,
    ) -> Result<Vec<Effect>, String> {
        self.as_mut()
            .handle_runtime_input(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                dispatch,
            ))))
    }

    pub(super) fn handle_runtime_input(
        mut self: Pin<&mut Self>,
        input: RuntimeInput,
    ) -> Result<Vec<Effect>, String> {
        let (previous_revision, result) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let previous_revision = this.state.as_ref().map_or(0, BrowserApplication::revision);
            let result = this
                .state
                .as_mut()
                .ok_or_else(|| "Core state unavailable".to_owned())
                .and_then(|runtime| {
                    runtime
                        .dispatch_runtime(input)
                        .map_err(|error| error.user_message().to_owned())
                });
            (previous_revision, result)
        };
        result.map(|effects| {
            self.as_mut()
                .accept_runtime_effects(previous_revision, effects)
        })
    }

    /// Delivers effects already produced through an application-owned method
    /// to the same checkpoint, IPC and Qt execution path as ordinary runtime
    /// inputs. The adapter must capture the pre-mutation revision first.
    pub(super) fn accept_runtime_effects(
        mut self: Pin<&mut Self>,
        previous_revision: u64,
        effects: Vec<RuntimeEffect>,
    ) -> Vec<Effect> {
        if checkpoint_required(&effects) {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.checkpoint.dirty = true;
            if this.checkpoint.dirty_since_ms.is_none() {
                this.checkpoint.dirty_since_ms = Some(elapsed_ms(this.binding_clock));
            }
            for (event_type, payload) in effects.iter().filter_map(|effect| {
                if let RuntimeEffect::IpcEvent {
                    event_type,
                    payload,
                } = effect
                {
                    Some((*event_type, payload))
                } else {
                    None
                }
            }) {
                this.ipc_sequence = this.ipc_sequence.max(previous_revision).saturating_add(1);
                if let Some(instance_id) = ipc_instance_id() {
                    publish_event(&EventNotification {
                        instance_id: instance_id.to_owned(),
                        sequence: this.ipc_sequence,
                        event_type: event_type.into(),
                        payload: payload.clone(),
                    });
                }
            }
        }
        for effect in &effects {
            if let RuntimeEffect::Engine(EngineEffect::Versioning(versioning)) = effect {
                self.as_mut().present_versioning_effect(versioning);
            }
        }
        effects
            .into_iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::Engine(effect) => Some(Effect::Engine(effect)),
                RuntimeEffect::Storage(effect) => Some(Effect::Persist(effect)),
                RuntimeEffect::Diagnostic(effect) => Some(Effect::Diagnostic(effect)),
                RuntimeEffect::AppliedEvent(_)
                | RuntimeEffect::IpcEvent { .. }
                | RuntimeEffect::UiModelUpdated(_)
                | RuntimeEffect::WorkerObserved { .. } => None,
            })
            .collect()
    }
}
