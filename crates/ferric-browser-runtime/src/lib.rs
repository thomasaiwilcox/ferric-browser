//! Qt-independent application orchestration for Ferric Browser.
//!
//! Adapters translate platform callbacks into [`RuntimeInput`] and execute the
//! returned [`RuntimeEffect`] values. They never receive mutable core state.

use std::{error::Error, fmt};

use ferric_browser_core::{
    ApplicationState, CommandContext, CommandInvocation, CommandRegistry, CommandSource,
    Diagnostic, DispatchError, DispatchTarget, Effect, EngineEffect, Event, JourneyGraph, Mode,
    NavigationContext, PersistEffect, ProfileId, ProfileState, ReduceError, ShutdownState, TabId,
    TabState, Target, ValidatedUrl, WindowId, WindowState, dispatch_command_for, reduce,
};
pub use ferric_browser_ipc::ErrorCode;

mod ipc_projection;
pub use ipc_projection::{ipc_event_payload, ipc_event_type};

#[derive(Debug)]
pub struct RuntimeError {
    code: ErrorCode,
    user_message: String,
    diagnostic_context: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl RuntimeError {
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub fn user_message(&self) -> &str {
        &self.user_message
    }

    #[must_use]
    pub fn diagnostic_context(&self) -> &str {
        &self.diagnostic_context
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.user_message)
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<ReduceError> for RuntimeError {
    fn from(error: ReduceError) -> Self {
        let code = match error {
            ReduceError::UnknownProfile(_)
            | ReduceError::UnknownWindow(_)
            | ReduceError::UnknownTab(_) => ErrorCode::NotFound,
            ReduceError::ShutdownInProgress => ErrorCode::Busy,
            ReduceError::WrongProfile
            | ReduceError::TabAlreadyInWindow
            | ReduceError::InvalidActivation
            | ReduceError::TabNotHidden
            | ReduceError::InvalidLifecycle
            | ReduceError::InvalidZoom => ErrorCode::InvalidArgument,
        };
        Self {
            code,
            user_message: "The browser could not apply that request.".into(),
            diagnostic_context: error.to_string(),
            source: Some(Box::new(error)),
        }
    }
}

impl From<DispatchError> for RuntimeError {
    fn from(error: DispatchError) -> Self {
        let code = match &error {
            DispatchError::Reduce(error) => return Self::from(error.clone()),
            DispatchError::NoActiveTab => ErrorCode::NotFound,
            DispatchError::Navigation(
                ferric_browser_core::NavigationError::ExternalSchemeRequiresConfirmation { .. },
            ) => ErrorCode::ConfirmationRequired,
            DispatchError::Command(_)
            | DispatchError::Navigation(_)
            | DispatchError::MissingArgument { .. }
            | DispatchError::UnexpectedArguments { .. }
            | DispatchError::CountNotSupported { .. }
            | DispatchError::CountTooLarge { .. }
            | DispatchError::UnsupportedCommand(_) => ErrorCode::InvalidArgument,
        };
        Self {
            code,
            user_message: "The browser could not execute that command.".into(),
            diagnostic_context: error.to_string(),
            source: Some(Box::new(error)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeCommand {
    /// Executes a registry-validated command against a captured target.
    Dispatch(Box<RuntimeDispatch>),
    Navigate {
        target: Target,
        url: ValidatedUrl,
    },
    OpenTab {
        window: WindowId,
        url: Option<ValidatedUrl>,
        background: bool,
    },
    CloseTab {
        tab: TabId,
    },
    EnterMode {
        window: WindowId,
        mode: Mode,
    },
    LeaveMode {
        window: WindowId,
    },
}

/// Typed dispatch request accepted by [`BrowserRuntime`].
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDispatch {
    pub invocation: CommandInvocation,
    pub navigation: NavigationContext,
    pub target: DispatchTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeInput {
    Command(RuntimeCommand),
    EngineFact(Event),
    WorkerCompletion { job_id: u64, outcome: WorkerOutcome },
    Lifecycle(Event),
    UserDecision(Event),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerOutcome {
    Completed,
    Failed { code: ErrorCode },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEffect {
    Engine(EngineEffect),
    Storage(PersistEffect),
    UiModelUpdated(RuntimeSnapshot),
    Diagnostic(Diagnostic),
    /// A reducer event accepted by the runtime.
    ///
    /// Platform adapters use this only for adapter-owned observations such as
    /// IPC publication and checkpoint scheduling. It is emitted after the
    /// event's reducer effects, so it cannot be used to mutate core state.
    AppliedEvent(Event),
    /// Public observation data captured immediately after an accepted event.
    /// The adapter transports this payload unchanged over its IPC mechanism.
    IpcEvent {
        event_type: &'static str,
        payload: serde_json::Value,
    },
    WorkerObserved {
        job_id: u64,
        outcome: WorkerOutcome,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    revision: u64,
    shutdown: ShutdownState,
    active_window: Option<WindowId>,
    windows: Vec<WindowSnapshot>,
}

impl RuntimeSnapshot {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn shutdown(&self) -> ShutdownState {
        self.shutdown
    }

    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    #[must_use]
    pub fn windows(&self) -> &[WindowSnapshot] {
        &self.windows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowSnapshot {
    pub id: WindowId,
    pub profile: ProfileId,
    pub active_tab: Option<TabId>,
    pub tabs: Vec<TabId>,
    pub mode: Mode,
}

/// Borrowed, immutable projection of runtime state.
///
/// Adapters may inspect domain state through this type, but cannot depend on
/// the reducer's owning [`ApplicationState`] or gain a mutation path to it.
#[derive(Clone, Copy)]
pub struct RuntimeView<'a> {
    state: &'a ApplicationState,
}

impl<'a> RuntimeView<'a> {
    #[must_use]
    pub fn profiles(&self) -> &'a std::collections::BTreeMap<ProfileId, ProfileState> {
        self.state.profiles()
    }

    #[must_use]
    pub fn windows(&self) -> &'a std::collections::BTreeMap<WindowId, WindowState> {
        self.state.windows()
    }

    #[must_use]
    pub fn tabs(&self) -> &'a std::collections::BTreeMap<TabId, TabState> {
        self.state.tabs()
    }

    #[must_use]
    pub fn journey(&self) -> &'a JourneyGraph {
        self.state.journey()
    }

    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.state.active_window()
    }

    #[must_use]
    pub const fn last_focused_window(&self) -> Option<WindowId> {
        self.state.last_focused_window()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.state.revision()
    }

    #[must_use]
    pub const fn shutdown(&self) -> ShutdownState {
        self.state.shutdown()
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&'a TabState> {
        self.state.active_tab()
    }

    #[must_use]
    pub fn capture_target(&self, tab: TabId) -> Option<Target> {
        self.state.capture_target(tab)
    }
}

pub struct BrowserRuntime {
    state: ApplicationState,
    registry: CommandRegistry,
}

impl Default for BrowserRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ApplicationState::new(),
            registry: CommandRegistry::default_v1(),
        }
    }

    /// Creates a runtime with the sole command registry used for dispatch.
    #[must_use]
    pub fn with_registry(registry: CommandRegistry) -> Self {
        Self {
            state: ApplicationState::new(),
            registry,
        }
    }

    /// Builds the initial profile, window, and tab through the same runtime
    /// input path used after startup.
    ///
    /// # Errors
    ///
    /// Returns a structured error if the reducer cannot establish the initial
    /// browser state.
    pub fn bootstrap(
        privacy: ferric_browser_core::PrivacyKind,
        profile_label: impl Into<String>,
    ) -> Result<(Self, WindowId, TabId), RuntimeError> {
        let mut runtime = Self::new();
        runtime.handle(RuntimeInput::Lifecycle(Event::CreateProfile {
            label: profile_label.into(),
            privacy,
        }))?;
        let profile = runtime
            .state
            .profiles()
            .keys()
            .next()
            .copied()
            .ok_or_else(|| Self::invariant("initial profile was not created"))?;
        runtime.handle(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))?;
        let window = runtime
            .state
            .active_window()
            .ok_or_else(|| Self::invariant("initial window was not created"))?;
        runtime.handle(RuntimeInput::Command(RuntimeCommand::OpenTab {
            window,
            url: None,
            background: false,
        }))?;
        let tab = runtime
            .state
            .active_tab()
            .map(|tab| tab.id)
            .ok_or_else(|| Self::invariant("initial tab was not created"))?;
        Ok((runtime, window, tab))
    }

    /// Returns a read-only projection without exposing the reducer owner.
    #[must_use]
    pub const fn view(&self) -> RuntimeView<'_> {
        RuntimeView { state: &self.state }
    }

    #[must_use]
    pub fn capture_command_context(
        &self,
        selector: DispatchTarget,
        source: CommandSource,
        count: u32,
        operation_id: Option<String>,
    ) -> CommandContext {
        CommandContext::capture(&self.state, selector, source, count, operation_id)
    }

    #[must_use]
    pub fn snapshot(&self) -> RuntimeSnapshot {
        Self::snapshot_for_state(&self.state)
    }

    fn snapshot_for_state(state: &ApplicationState) -> RuntimeSnapshot {
        RuntimeSnapshot {
            revision: state.revision(),
            shutdown: state.shutdown(),
            active_window: state.active_window(),
            windows: state
                .windows()
                .values()
                .map(|window| WindowSnapshot {
                    id: window.id,
                    profile: window.profile,
                    active_tab: window.active_tab,
                    tabs: window.tabs.clone(),
                    mode: window.modes.last().copied().unwrap_or(Mode::Normal),
                })
                .collect(),
        }
    }

    fn invariant(diagnostic_context: impl Into<String>) -> RuntimeError {
        RuntimeError {
            code: ErrorCode::Engine,
            user_message: "The browser could not initialize its runtime.".into(),
            diagnostic_context: diagnostic_context.into(),
            source: None,
        }
    }

    /// Applies one typed input and returns every effect the adapter must run.
    ///
    /// # Errors
    ///
    /// Returns a structured [`RuntimeError`] when the input violates a core
    /// ownership, lifecycle, or target invariant.
    pub fn handle(&mut self, input: RuntimeInput) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        if let RuntimeInput::Command(RuntimeCommand::Dispatch(dispatch)) = input {
            let events = self.dispatch_events(*dispatch)?;
            let mut effects = Vec::new();
            for event in events {
                effects.extend(Self::apply_event(&mut self.state, &event)?);
                effects.push(RuntimeEffect::AppliedEvent(event));
            }
            effects.push(RuntimeEffect::UiModelUpdated(self.snapshot()));
            return Ok(effects);
        }
        Self::apply_input(&mut self.state, input)
    }

    fn dispatch_events(&self, dispatch: RuntimeDispatch) -> Result<Vec<Event>, RuntimeError> {
        Ok(dispatch_command_for(
            &self.state,
            &self.registry,
            dispatch.invocation,
            &dispatch.navigation,
            dispatch.target,
        )?)
    }

    fn apply_input(
        state: &mut ApplicationState,
        input: RuntimeInput,
    ) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        if let RuntimeInput::WorkerCompletion { job_id, outcome } = input {
            return Ok(vec![RuntimeEffect::WorkerObserved { job_id, outcome }]);
        }
        let event = match input {
            RuntimeInput::Command(command) => command.into_event(),
            RuntimeInput::EngineFact(event)
            | RuntimeInput::Lifecycle(event)
            | RuntimeInput::UserDecision(event) => event,
            RuntimeInput::WorkerCompletion { .. } => unreachable!("handled above"),
        };
        let mut runtime_effects = Self::apply_event(state, &event)?;
        runtime_effects.push(RuntimeEffect::AppliedEvent(event));
        // Reducer state includes transient presentation state (loading, mode,
        // focus) that intentionally does not advance the durable revision.
        // Publishing a bounded snapshot after every accepted input keeps the
        // adapter synchronized without exposing mutable state.
        runtime_effects.push(RuntimeEffect::UiModelUpdated(Self::snapshot_for_state(
            state,
        )));
        Ok(runtime_effects)
    }

    fn apply_event(
        state: &mut ApplicationState,
        event: &Event,
    ) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        let effects = reduce(state, event.clone())?;
        let ignored_stale_target = effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { .. })
            )
        });
        let mut runtime_effects = effects
            .into_iter()
            .map(|effect| match effect {
                Effect::Engine(effect) => RuntimeEffect::Engine(effect),
                Effect::Persist(effect) => RuntimeEffect::Storage(effect),
                Effect::Diagnostic(diagnostic) => RuntimeEffect::Diagnostic(diagnostic),
            })
            .collect::<Vec<_>>();
        if !ignored_stale_target {
            runtime_effects.push(RuntimeEffect::IpcEvent {
                event_type: ipc_event_type(event),
                payload: ipc_event_payload(event, state),
            });
        }
        Ok(runtime_effects)
    }
}

impl RuntimeCommand {
    fn into_event(self) -> Event {
        match self {
            Self::Dispatch(_) => {
                unreachable!("registry commands are handled by BrowserRuntime")
            }
            Self::Navigate { target, url } => Event::StartNavigation { target, url },
            Self::OpenTab {
                window,
                url: Some(url),
                background,
            } => Event::OpenTabWithNavigation {
                window,
                url,
                background,
            },
            Self::OpenTab {
                window, url: None, ..
            } => Event::OpenTab { window },
            Self::CloseTab { tab } => Event::CloseTab { tab },
            Self::EnterMode { window, mode } => Event::PushMode { window, mode },
            Self::LeaveMode { window } => Event::PopMode { window },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::PrivacyKind;

    fn runtime_with_tab() -> (BrowserRuntime, WindowId, TabId) {
        let mut runtime = BrowserRuntime::new();
        runtime
            .handle(RuntimeInput::Lifecycle(Event::CreateProfile {
                label: "default".into(),
                privacy: PrivacyKind::Normal,
            }))
            .expect("profile");
        let profile = runtime
            .state
            .profiles()
            .keys()
            .next()
            .copied()
            .expect("profile");
        runtime
            .handle(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))
            .expect("window");
        let window = runtime.snapshot().active_window().expect("window");
        runtime
            .handle(RuntimeInput::Command(RuntimeCommand::OpenTab {
                window,
                url: None,
                background: false,
            }))
            .expect("tab");
        let tab = runtime.state.active_tab().expect("tab").id;
        (runtime, window, tab)
    }

    #[test]
    fn bootstrap_uses_runtime_inputs_and_exposes_only_a_read_only_state_view() {
        let (runtime, window, tab) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").expect("bootstrap");
        assert_eq!(runtime.view().active_window(), Some(window));
        assert_eq!(runtime.view().active_tab().map(|tab| tab.id), Some(tab));
        assert_eq!(runtime.snapshot().windows()[0].tabs, vec![tab]);
    }

    #[test]
    fn navigation_open_close_and_modes_share_the_runtime_boundary() {
        let (mut runtime, window, tab) = runtime_with_tab();
        let target = runtime.state.capture_target(tab).expect("target");
        let effects = runtime
            .handle(RuntimeInput::Command(RuntimeCommand::Navigate {
                target,
                url: ValidatedUrl::parse("https://example.test").expect("URL"),
            }))
            .expect("navigate");
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, RuntimeEffect::Engine(_)))
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, RuntimeEffect::UiModelUpdated(_)))
        );

        runtime
            .handle(RuntimeInput::Command(RuntimeCommand::EnterMode {
                window,
                mode: Mode::Command,
            }))
            .expect("mode");
        assert_eq!(runtime.snapshot().windows()[0].mode, Mode::Command);

        runtime
            .handle(RuntimeInput::Command(RuntimeCommand::CloseTab { tab }))
            .expect("close");
    }

    #[test]
    fn registry_commands_are_dispatched_and_reduced_inside_the_runtime() {
        let (mut runtime, window, _) = runtime_with_tab();
        let effects = runtime
            .handle(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                RuntimeDispatch {
                    invocation: CommandInvocation::new(ferric_browser_core::ParsedCommand {
                        name: "tab-open".into(),
                        arguments: vec!["about:blank".into()],
                    }),
                    navigation: NavigationContext::default(),
                    target: DispatchTarget::Window(window),
                },
            ))))
            .expect("registry command");

        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::UiModelUpdated(snapshot)
                if snapshot.windows()[0].tabs.len() == 2
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::OpenTabWithNavigation { window: observed, .. })
                if *observed == window
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::IpcEvent { event_type: "tab.changed", payload }
                if payload["window_id"] == window.to_string()
        )));
    }

    #[test]
    fn reducer_failures_keep_a_stable_error_code() {
        let mut runtime = BrowserRuntime::new();
        let error = runtime
            .handle(RuntimeInput::Command(RuntimeCommand::OpenTab {
                window: WindowId::from_display("windowid-99").expect("synthetic ID"),
                url: None,
                background: false,
            }))
            .expect_err("unknown window");
        assert_eq!(error.code(), ErrorCode::NotFound);
        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
        assert!(!error.diagnostic_context().is_empty());
    }
}
