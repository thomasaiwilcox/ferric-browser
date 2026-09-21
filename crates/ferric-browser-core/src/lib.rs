//! The Qt-independent application model.
//!
//! This crate deliberately contains no GUI, display-server, filesystem, or
//! network dependencies. Qt and desktop adapters consume its typed events and
//! effects in later milestones.

mod action;
mod command;
mod completion;
mod dispatcher;
mod hints;
mod id;
mod input;
mod journey;
mod link_cleaning;
mod model;
mod navigation;
mod reducer;
mod switcher;
mod url;

pub use action::{
    ActionConfirmation, ActionDefinition, ActionRegistry, ActionSource, ActionSubject,
};
pub use command::{
    ArgumentDefinition, ArgumentKind, CommandAlias, CommandDefinition, CommandError,
    CommandRegistry, CommandScope, CompletionProvider, CountPolicy, EffectClass, ParseInput,
    ParsedCommand, parse_chain, validate_open_target,
};
pub use completion::{
    CompletionCandidate, CompletionCategory, CompletionResult, DEFAULT_COMPLETION_LIMIT, complete,
};
pub use dispatcher::{
    CommandContext, CommandInvocation, CommandSource, DispatchError, DispatchTarget,
    dispatch_command, dispatch_command_for,
};
pub use hints::{
    HintCandidate, HintError, HintGeometry, HintKind, HintSession, HintTarget, LabeledHint,
    MAX_HINT_CANDIDATES, assign_labels,
};
pub use id::{
    ActionId, ContextId, DocumentId, DownloadId, HintSessionId, IdSource, JourneyNodeId, ProfileId,
    RequestId, SessionId, TabId, WindowId,
};
pub use input::{
    BindingDefinition, BindingError, BindingOutcome, BindingResolver, BindingTrie,
    DEFAULT_CHORD_TIMEOUT_MS,
};
pub use journey::{
    JourneyEdge, JourneyEdgeKind, JourneyGraph, JourneyNode, MAX_JOURNEY_EDGES, MAX_JOURNEY_NODES,
};
pub use link_cleaning::{
    CleanLinkResult, CleaningRule, CleaningRules, HostPattern, LinkCleanError, builtin_rules,
    clean_link,
};
pub use model::{
    ApplicationState, ExistenceState, LoadingState, Mode, PrivacyKind, ProfileState, RendererState,
    ResourceLifecycle, SearchCase, SearchState, ShutdownState, TabState, WindowState,
};
pub use navigation::{
    NavigationContext, NavigationError, NavigationResult, NavigationSource, resolve_input,
    resolve_search_query,
};
pub use reducer::{
    Diagnostic, Effect, EngineEffect, Event, PersistEffect, ReduceError, TabTransfer, Target,
    reduce,
};
pub use switcher::{
    MAX_SWITCHER_QUERY_TERMS, MAX_SWITCHER_TERM_BYTES, rank as switcher_rank,
    tokenize_query as tokenize_switcher_query,
};
pub use url::{UrlError, ValidatedUrl, canonical_origin, canonicalize};
