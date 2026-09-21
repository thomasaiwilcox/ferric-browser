//! Qt-independent application orchestration for Ferric Browser.
//!
//! Adapters translate platform callbacks into [`RuntimeInput`] and execute the
//! returned [`RuntimeEffect`] values. They never receive mutable core state.

use std::{error::Error, fmt};

use ferric_browser_core::{
    ApplicationState, Diagnostic, Effect, EngineEffect, Event, Mode, PersistEffect, ProfileId,
    ReduceError, ShutdownState, TabId, Target, ValidatedUrl, WindowId, reduce,
};
pub use ferric_browser_ipc::ErrorCode;

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

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeCommand {
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
    WorkerObserved { job_id: u64, outcome: WorkerOutcome },
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

pub struct BrowserRuntime {
    state: ApplicationState,
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
        }
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

    /// Applies one typed input and returns every effect the adapter must run.
    ///
    /// # Errors
    ///
    /// Returns a structured [`RuntimeError`] when the input violates a core
    /// ownership, lifecycle, or target invariant.
    pub fn handle(&mut self, input: RuntimeInput) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        Self::handle_with_state(&mut self.state, input)
    }

    /// Applies an input to state still owned by a legacy adapter during the
    /// incremental runtime extraction.
    ///
    /// This compatibility seam keeps mutation inside the runtime/reducer path
    /// while Qt consumers move from read-only core views to bounded snapshots.
    /// New adapters should own a [`BrowserRuntime`] and call [`Self::handle`].
    ///
    /// # Errors
    ///
    /// Returns a structured [`RuntimeError`] under the same conditions as
    /// [`Self::handle`].
    pub fn handle_with_state(
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
        let effects = reduce(state, event)?;
        let mut runtime_effects = effects
            .into_iter()
            .map(|effect| match effect {
                Effect::Engine(effect) => RuntimeEffect::Engine(effect),
                Effect::Persist(effect) => RuntimeEffect::Storage(effect),
                Effect::Diagnostic(diagnostic) => RuntimeEffect::Diagnostic(diagnostic),
            })
            .collect::<Vec<_>>();
        // Reducer state includes transient presentation state (loading, mode,
        // focus) that intentionally does not advance the durable revision.
        // Publishing a bounded snapshot after every accepted input keeps the
        // adapter synchronized without exposing mutable state.
        runtime_effects.push(RuntimeEffect::UiModelUpdated(Self::snapshot_for_state(
            state,
        )));
        Ok(runtime_effects)
    }
}

impl RuntimeCommand {
    fn into_event(self) -> Event {
        match self {
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
