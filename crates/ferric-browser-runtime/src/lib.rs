//! Qt-independent application orchestration for Ferric Browser.
//!
//! Adapters translate platform callbacks into [`RuntimeInput`] and execute the
//! returned [`RuntimeEffect`] values. They never receive mutable core state.

use std::{error::Error, fmt};

use ferric_browser_core::versioning::SensitiveCategory;
use ferric_browser_core::{
    ApplicationOwnerId, ApplicationState, BranchId, BranchRecord, CapturedScope, CommandContext,
    CommandInvocation, CommandRegistry, CommandSource, Diagnostic, DispatchError, DispatchTarget,
    Effect, EngineEffect, Event, JourneyGraph, LogicalPageId, LogicalWindowId, Mode,
    NativeProfileOwner, NavigationContext, PageCaptureInput, PersistEffect, ProfileId,
    ProfileState, ReduceError, RetainedViewId, RetentionRecord, ScopeError, ScopeSelector,
    ShutdownState, StructuralCaptureError, StructuralSnapshot, TabId, TabState, Target,
    ValidatedUrl, VersioningEvent, VersioningFailure, WindowId, WindowState, dispatch_command_for,
    project_structural_scope, reduce,
};
pub use ferric_browser_ipc::ErrorCode;
use ferric_browser_ipc::PublicError;

mod ipc_projection;
pub use ipc_projection::{ipc_event_payload, ipc_event_type};

pub struct RuntimeError {
    code: ErrorCode,
    user_message: String,
    diagnostic_context: String,
}

impl fmt::Debug for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeError")
            .field("code", &self.code)
            .field("user_message", &"[redacted]")
            .field("diagnostic_context", &"[redacted]")
            .finish()
    }
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

// INVARIANT: converted runtime errors retain only their closed public code,
// fixed user text, and redacted diagnostic category. The consumed source may
// contain caller-controlled URLs, command text, or runtime identity and must
// not remain available through Error::source/downcasting.
impl Error for RuntimeError {}

impl From<VersioningFailure> for RuntimeError {
    fn from(error: VersioningFailure) -> Self {
        let public = PublicError::from(error);
        Self {
            code: public.code(),
            user_message: public.user_message().into(),
            diagnostic_context: public.diagnostic_context().into(),
        }
    }
}

impl From<ReduceError> for RuntimeError {
    fn from(error: ReduceError) -> Self {
        let code = match &error {
            ReduceError::Versioning(inner) => PublicError::from(VersioningFailure::new(
                inner.reason(),
                SensitiveCategory::None,
            ))
            .code(),
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
            | DispatchError::Versioning(_)
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
        }
    }
}

#[derive(Clone, PartialEq)]
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

impl fmt::Debug for RuntimeCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch(_) => formatter.write_str("RuntimeCommand::Dispatch([redacted])"),
            Self::Navigate { .. } => formatter.write_str("RuntimeCommand::Navigate([redacted])"),
            Self::OpenTab {
                url, background, ..
            } => formatter
                .debug_struct("RuntimeCommand::OpenTab")
                .field("has_url", &url.is_some())
                .field("background", background)
                .finish(),
            Self::CloseTab { .. } => formatter.write_str("RuntimeCommand::CloseTab([redacted])"),
            Self::EnterMode { mode, .. } => formatter
                .debug_struct("RuntimeCommand::EnterMode")
                .field("mode", mode)
                .finish(),
            Self::LeaveMode { .. } => formatter.write_str("RuntimeCommand::LeaveMode"),
        }
    }
}

/// Typed dispatch request accepted by [`BrowserRuntime`].
#[derive(Clone, PartialEq)]
pub struct RuntimeDispatch {
    pub invocation: CommandInvocation,
    pub navigation: NavigationContext,
    pub target: DispatchTarget,
}

impl fmt::Debug for RuntimeDispatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeDispatch")
            .field("invocation", &"[redacted]")
            .field("navigation", &"[redacted]")
            .field("target", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub enum RuntimeInput {
    Command(RuntimeCommand),
    EngineFact(Event),
    WorkerCompletion { job_id: u64, outcome: WorkerOutcome },
    Lifecycle(Event),
    UserDecision(Event),
}

impl fmt::Debug for RuntimeInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(command) => formatter
                .debug_tuple("RuntimeInput::Command")
                .field(command)
                .finish(),
            Self::EngineFact(_) => formatter.write_str("RuntimeInput::EngineFact([redacted])"),
            Self::WorkerCompletion { outcome, .. } => formatter
                .debug_struct("RuntimeInput::WorkerCompletion")
                .field("outcome", outcome)
                .finish(),
            Self::Lifecycle(_) => formatter.write_str("RuntimeInput::Lifecycle([redacted])"),
            Self::UserDecision(_) => formatter.write_str("RuntimeInput::UserDecision([redacted])"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerOutcome {
    Completed,
    Failed { code: ErrorCode },
}

#[derive(Clone, Eq, PartialEq)]
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

impl fmt::Debug for RuntimeEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(_) => formatter.write_str("RuntimeEffect::Engine([redacted])"),
            Self::Storage(_) => formatter.write_str("RuntimeEffect::Storage([redacted])"),
            Self::UiModelUpdated(snapshot) => formatter
                .debug_struct("RuntimeEffect::UiModelUpdated")
                .field("revision", &snapshot.revision)
                .field("window_count", &snapshot.windows.len())
                .finish(),
            Self::Diagnostic(_) => formatter.write_str("RuntimeEffect::Diagnostic([redacted])"),
            Self::AppliedEvent(_) => formatter.write_str("RuntimeEffect::AppliedEvent([redacted])"),
            Self::IpcEvent { event_type, .. } => formatter
                .debug_struct("RuntimeEffect::IpcEvent")
                .field("event_type", event_type)
                .field("payload", &"[redacted]")
                .finish(),
            Self::WorkerObserved { outcome, .. } => formatter
                .debug_struct("RuntimeEffect::WorkerObserved")
                .field("outcome", outcome)
                .finish(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    revision: u64,
    shutdown: ShutdownState,
    active_window: Option<WindowId>,
    windows: Vec<WindowSnapshot>,
}

impl fmt::Debug for RuntimeSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeSnapshot")
            .field("revision", &self.revision)
            .field("shutdown", &self.shutdown)
            .field("has_active_window", &self.active_window.is_some())
            .field("window_count", &self.windows.len())
            .finish()
    }
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

#[derive(Clone, Eq, PartialEq)]
pub struct WindowSnapshot {
    pub id: WindowId,
    pub profile: ProfileId,
    pub active_tab: Option<TabId>,
    pub tabs: Vec<TabId>,
    pub mode: Mode,
}

impl fmt::Debug for WindowSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowSnapshot")
            .field("id", &"[redacted]")
            .field("profile", &"[redacted]")
            .field("has_active_tab", &self.active_tab.is_some())
            .field("tab_count", &self.tabs.len())
            .field("mode", &self.mode)
            .finish()
    }
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
    /// Predicts fresh reducer-owned IDs for a new window and its tabs on an
    /// isolated state clone. The caller must retain `revision()` and refuse
    /// execution if the live revision changes before applying the same events.
    /// No live state is changed and no persistence effect is delivered.
    ///
    /// # Errors
    /// Refuses an unknown profile or a tab count outside the bounded window.
    pub fn preview_new_window_ids(
        &self,
        profile: ProfileId,
        tab_count: usize,
    ) -> Result<(WindowId, Vec<TabId>), ferric_browser_core::ReduceError> {
        if !(1..=128).contains(&tab_count) {
            return Err(ReduceError::InvalidActivation);
        }
        let mut preview = self.state.clone_for_identity_preview();
        reduce(&mut preview, Event::CreateWindow { profile })?;
        let window = preview
            .active_window()
            .ok_or(ReduceError::InvalidActivation)?;
        let mut tabs = Vec::with_capacity(tab_count);
        for _ in 0..tab_count {
            reduce(&mut preview, Event::OpenTab { window })?;
            tabs.push(
                preview
                    .active_tab()
                    .ok_or(ReduceError::InvalidActivation)?
                    .id,
            );
        }
        Ok((window, tabs))
    }

    /// Resolves an immutable versioning subject set without exposing mutable
    /// reducer state to the application coordinator.
    ///
    /// # Errors
    /// Rejects stale, mixed-owner or unsupported context scopes.
    pub fn resolve_versioning_scope(
        &self,
        selector: ScopeSelector,
        native: NativeProfileOwner,
    ) -> Result<CapturedScope, ScopeError> {
        ferric_browser_core::resolve_scope(self.state, selector, native)
    }

    #[must_use]
    pub fn logical_window(&self, window: WindowId) -> Option<LogicalWindowId> {
        self.state.versioning().logical_window(window)
    }

    #[must_use]
    pub fn logical_page(&self, tab: TabId) -> Option<LogicalPageId> {
        self.state.versioning().logical_page(tab)
    }

    #[must_use]
    pub fn lineage_parent(&self, tab: TabId) -> Option<LogicalPageId> {
        self.state.versioning().lineage_parent(tab)
    }

    /// Projects only the previously admitted scope at its exact revision.
    ///
    /// # Errors
    /// Rejects stale or mismatched structural input before any storage write.
    pub fn project_structural_scope(
        &self,
        scope: &CapturedScope,
        logical_window: LogicalWindowId,
        inputs: &std::collections::BTreeMap<TabId, PageCaptureInput>,
    ) -> Result<StructuralSnapshot, StructuralCaptureError> {
        project_structural_scope(self.state, scope, logical_window, inputs)
    }

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

    /// The tab strip excludes reducer-owned retained/transferring views while
    /// the logical window continues to own their live tab records.
    #[must_use]
    pub fn visible_versioning_tabs_for_window(&self, window: WindowId) -> Option<Vec<TabId>> {
        self.state
            .versioning()
            .visible_tabs_for_window(self.state, window)
    }

    /// Reports whether a profile has any child-branch record not explicitly
    /// closed. Callers lacking exact window ownership must treat this as a
    /// profile-wide gate.
    #[must_use]
    pub fn has_unresolved_child_branch_for_profile(&self, profile: ProfileId) -> bool {
        self.state
            .versioning()
            .has_unresolved_child_branch_for_profile(profile)
    }

    #[must_use]
    pub fn versioning_root_branch(
        &self,
        application: ApplicationOwnerId,
        profile: ProfileId,
    ) -> Option<&'a BranchRecord> {
        self.state.versioning().branches().values().find(|branch| {
            branch.parent().is_none()
                && branch.application() == application
                && branch.profile() == profile
        })
    }

    #[must_use]
    pub fn versioning_branch(&self, id: BranchId) -> Option<&'a BranchRecord> {
        self.state.versioning().branch(id)
    }

    /// Verify a finite sequence of lineage events against a disposable state
    /// clone before a multi-owner coordinator mutates either live runtime.
    pub fn preview_versioning_events(&self, events: &[VersioningEvent]) -> Result<(), ReduceError> {
        let mut preview = self.state.clone_for_identity_preview();
        for event in events {
            reduce(&mut preview, Event::Versioning(event.clone()))?;
        }
        Ok(())
    }

    #[must_use]
    pub fn versioning_retention(&self, view: RetainedViewId) -> Option<&'a RetentionRecord> {
        self.state.versioning().retained().get(&view)
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
        Self::new().finish_bootstrap(privacy, profile_label)
    }

    /// Bootstraps a UI-owned runtime with process-wide opaque identities.
    /// Separate browser windows must use this path to avoid ID collisions.
    ///
    /// # Errors
    ///
    /// Returns a structured error if the reducer cannot establish the initial
    /// browser state.
    pub fn bootstrap_process_unique(
        privacy: ferric_browser_core::PrivacyKind,
        profile_label: impl Into<String>,
    ) -> Result<(Self, WindowId, TabId), RuntimeError> {
        Self {
            state: ApplicationState::new_process_unique(),
            registry: CommandRegistry::default_v1(),
        }
        .finish_bootstrap(privacy, profile_label)
    }

    fn finish_bootstrap(
        mut self,
        privacy: ferric_browser_core::PrivacyKind,
        profile_label: impl Into<String>,
    ) -> Result<(Self, WindowId, TabId), RuntimeError> {
        self.handle(RuntimeInput::Lifecycle(Event::CreateProfile {
            label: profile_label.into(),
            privacy,
        }))?;
        let profile = self
            .state
            .profiles()
            .keys()
            .next()
            .copied()
            .ok_or_else(|| Self::invariant("initial profile was not created"))?;
        self.handle(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))?;
        let window = self
            .state
            .active_window()
            .ok_or_else(|| Self::invariant("initial window was not created"))?;
        self.handle(RuntimeInput::Command(RuntimeCommand::OpenTab {
            window,
            url: None,
            background: false,
        }))?;
        let tab = self
            .state
            .active_tab()
            .map(|tab| tab.id)
            .ok_or_else(|| Self::invariant("initial tab was not created"))?;
        Ok((self, window, tab))
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
        if !ignored_stale_target && !matches!(event, Event::VersioningRequest(_)) {
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
    use ferric_browser_core::{
        PrivacyKind,
        versioning::{SensitiveCategory, VersioningReason},
    };

    #[test]
    fn versioning_failure_maps_through_public_error_without_sensitive_payload() {
        let error = RuntimeError::from(VersioningFailure::new(
            VersioningReason::MergeOutcomeUnknown,
            SensitiveCategory::Cookie,
        ));
        assert_eq!(error.code(), ErrorCode::OutcomeUnknown);
        assert!(error.diagnostic_context().contains("merge-outcome-unknown"));
        assert!(!error.user_message().contains("cookie"));
        assert!(!error.to_string().contains("cookie"));
    }

    #[test]
    fn runtime_error_debug_redacts_navigation_url_and_source_chain() {
        let sentinel = "private-navigation-url-sentinel";
        let error = RuntimeError::from(DispatchError::Navigation(
            ferric_browser_core::NavigationError::ExternalSchemeRequiresConfirmation {
                scheme: "mailto".into(),
                url: format!("mailto:{sentinel}@example.test?subject=private"),
            },
        ));

        let debug = format!("{error:?}");
        assert!(std::error::Error::source(&error).is_none());
        assert!(!debug.contains(sentinel));
        assert!(!debug.contains("subject=private"));
        assert!(!debug.contains("has_source"));
        assert_eq!(error.diagnostic_context(), "navigation request failed");
        assert!(!error.diagnostic_context().contains(sentinel));
        assert!(!error.diagnostic_context().contains("subject=private"));
        assert!(!error.to_string().contains(sentinel));
    }

    #[test]
    fn runtime_command_input_dispatch_and_effect_debug_redact_page_data() {
        let sentinel = "private-runtime-page-data-sentinel";
        let url = ValidatedUrl::parse(format!("https://example.test/{sentinel}?query=secret"))
            .expect("valid sentinel URL");
        let command = RuntimeCommand::OpenTab {
            window: WindowId::from_display("windowid-1").expect("window id"),
            url: Some(url.clone()),
            background: false,
        };
        let invocation = CommandInvocation {
            command: ferric_browser_core::ParsedCommand {
                name: "open".into(),
                arguments: vec![sentinel.into()],
            },
            count: 1,
            context: CommandContext {
                context: Some(sentinel.into()),
                operation_id: Some(sentinel.into()),
                ..CommandContext::default()
            },
        };
        let dispatch = RuntimeDispatch {
            invocation,
            navigation: NavigationContext {
                quickmarks: std::collections::BTreeMap::from([(sentinel.into(), url.to_string())]),
                default_search_template: Some(url.to_string()),
                ..NavigationContext::default()
            },
            target: DispatchTarget::Active,
        };
        let target = Target {
            tab: TabId::from_display("tabid-1").expect("tab id"),
            generation: 1,
            document: ferric_browser_core::DocumentId::from_display("documentid-1")
                .expect("document id"),
        };
        let event = Event::CommitNavigation {
            target,
            url,
            title: sentinel.into(),
        };

        for debug in [
            format!("{command:?}"),
            format!("{:?}", RuntimeCommand::Dispatch(Box::new(dispatch.clone()))),
            format!("{dispatch:?}"),
            format!("{:?}", RuntimeInput::Command(command)),
            format!("{:?}", RuntimeInput::EngineFact(event.clone())),
            format!("{:?}", RuntimeEffect::AppliedEvent(event)),
        ] {
            assert!(!debug.contains(sentinel));
            assert!(!debug.contains("query=secret"));
        }
    }

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
    fn independently_bootstrapped_ui_owners_have_distinct_runtime_ids() {
        let (_, first_window, first_tab) =
            BrowserRuntime::bootstrap_process_unique(PrivacyKind::Normal, "Default")
                .expect("first UI owner");
        let (_, second_window, second_tab) =
            BrowserRuntime::bootstrap_process_unique(PrivacyKind::Normal, "Default")
                .expect("second UI owner");
        assert_ne!(first_window, second_window);
        assert_ne!(first_tab, second_tab);
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
