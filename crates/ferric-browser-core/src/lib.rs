//! The Qt-independent application model.
//!
//! This crate deliberately contains no GUI, display-server, filesystem, or
//! network dependencies. Qt and desktop adapters consume its typed events and
//! effects in later milestones.

mod failure;
pub use failure::FailureKind;

mod action;
mod command;
mod command_executors;
pub use command_executors::CommonCommandExecutor;
mod completion;
mod dispatcher;
mod guide;
mod hints;
pub use guide::{GuideArticle, guide_article};
mod id;
mod input;
mod internal_page;
pub use internal_page::{ContentEntry, ContentHistory, InternalPage};
mod journey;
mod link_cleaning;
mod mixed_policy;
mod model;
mod navigation;
mod spatial_navigation;
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
    HintAutoFollow, HintCandidate, HintCapability, HintError, HintGeometry, HintInputMode,
    HintIntent, HintInteraction, HintInteractionInput, HintInteractionOutcome,
    HintInteractionSnapshot, HintKind, HintPoint, HintPresentationGeometry, HintSession,
    HintTarget, HintTransport, HintUnmatchedPolicy, LabeledHint, MAX_HINT_CANDIDATES,
    MAX_HINT_REGIONS, MAX_HINT_SURFACES, assign_labels, assign_labels_with_options,
    hint_capabilities, hint_intent_for_candidate, hint_kind_is_region_or_surface, refresh_labels,
};
pub use id::{
    ActionId, ContextId, DocumentId, DownloadId, HintSessionId, IdSource, JourneyNodeId, ProfileId,
    RequestId, SessionId, SharedIdAllocator, SpatialRequestId, SpatialSessionId, TabId, WindowId,
};
pub use input::{
    BindingDefinition, BindingError, BindingOutcome, BindingRegistry, BindingResolver, BindingTrie,
    DEFAULT_CHORD_TIMEOUT_MS, binding_key_tokens,
};
pub use journey::{
    JourneyEdge, JourneyEdgeKind, JourneyGraph, JourneyNode, MAX_JOURNEY_EDGES, MAX_JOURNEY_NODES,
};
pub use link_cleaning::{
    CleanLinkResult, CleaningRule, CleaningRules, HostPattern, LinkCleanError, builtin_rules,
    clean_link,
};
pub use mixed_policy::{
    BrowserKeySequenceState, EffectiveMixedKeyPolicy, InputOwner, LogicalKeyToken,
    LogicalKeyTokenError, MAX_MIXED_PAGE_KEYS, MixedKeyDecision, MixedKeyDecisionReason,
    MixedPolicyError, MixedPolicyLayer, MixedPolicySource, ShortcutPolicy,
    resolve_mixed_key_policy,
};
pub use model::reducer::{
    ClipboardEdit, Diagnostic, Effect, EngineEffect, Event, PersistEffect, ReduceError,
    TabTransfer, Target, reduce,
};
pub use model::{
    ApplicationState, ExistenceState, LoadingState, Mode, PrivacyKind, ProfileState, RendererState,
    ResourceLifecycle, SearchCase, SearchState, ShutdownState, TabState, WindowState,
};
pub use navigation::{
    NavigationContext, NavigationError, NavigationResult, NavigationSource, resolve_input,
    resolve_search_query,
};
pub use spatial_navigation::{
    GridCell, LogicalPoint, LogicalRect, MAX_SPATIAL_DEPTH, PointerButton, SPATIAL_PRECISION_LIMIT,
    SPATIAL_SCOPE_GEOMETRY_TOLERANCE, SpatialAck, SpatialAction, SpatialCancelReason,
    SpatialDispatchOutcome, SpatialDispatchRequest, SpatialError, SpatialLifecycle, SpatialOwner,
    SpatialProjection, SpatialScope, SpatialSession, SpatialTarget, SurfaceStamp,
    map_css_rect_to_view_local,
};
pub use switcher::{
    MAX_SWITCHER_QUERY_TERMS, MAX_SWITCHER_TERM_BYTES, rank as switcher_rank,
    tokenize_query as tokenize_switcher_query,
};
pub use url::{UrlError, ValidatedUrl, canonical_origin, canonicalize};
