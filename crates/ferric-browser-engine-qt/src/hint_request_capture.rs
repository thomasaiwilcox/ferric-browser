//! Original browser request, captured before native/document/collector work.
//! The native adapter supplies actual view identity; no QML/renderer constructor
//! or completed result can replace the frozen policy, selectors or scope.
use std::{pin::Pin, sync::Arc};

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use ferric_browser_application::{
    CompiledSitePolicySnapshot, InteractionCacheUnavailable, InteractionCompilationState,
};
use ferric_browser_config::{EffectiveInteractionPolicy, InteractionFamily};
use ferric_browser_core::{HintTarget, Mode, ShutdownState, TabId, canonicalize};

use crate::{
    BrowserUiRust,
    hint_collection_work::{
        CollectionLedger, CollectionWork, MAX_METADATA_BYTES, NativeBinding, NativeBudget,
        NativeOperation, NativeOutcome, NativePhase, NativeToken, PendingPolicy,
        VerificationSchedule,
    },
    hint_policy::{HintOptions, HintScopeBinding},
    hint_policy_cache::CachedHintSelectorPolicy,
    interaction_proof::{
        AnnotationDocumentRoles, AnnotationFocusedReceiver, AnnotationGenerations,
        AnnotationRequest, CheckedAnnotationFacts, CheckedAnnotations,
        MAX_INTERACTION_ANNOTATION_BYTES, WorldProjections, decode_annotation_facts_with_budget,
    },
    policy_refresh_capture::PolicyRefreshCapture,
    qobject,
};

mod preview;
pub(crate) use preview::begin_hint_preview_request;
pub(crate) use preview::hint_builder_candidate;

#[derive(PartialEq)]
struct Invocation {
    options: HintOptions,
    pointer: bool,
    proxy: bool,
    search: bool,
    search_scope: QString,
}

impl Invocation {
    fn capture(rust: &BrowserUiRust) -> Self {
        Self {
            options: HintOptions {
                family: rust.hint_family.to_string(),
                target: rust.hint_target.to_string(),
                rapid: rust.hint_rapid,
                script: rust.hint_script.clone(),
                first: rust.hint_first,
                index: rust.hint_index,
            },
            pointer: rust.hint_pointer_mode,
            proxy: rust.hint_proxy_targets_enabled,
            search: rust.target_search_active,
            search_scope: rust.target_search_scope.clone(),
        }
    }
}

pub(super) struct RetainedHintRequest {
    owner: Option<PolicyRefreshCapture>,
    preview: Option<Arc<ferric_browser_application::PreparedSitePolicyPreview>>,
    capture_only: bool,
    snapshot: Arc<CompiledSitePolicySnapshot>,
    policy: Option<EffectiveInteractionPolicy>,
    selectors: CachedHintSelectorPolicy,
    invocation: Invocation,
    family: InteractionFamily,
    scope: Option<HintScopeBinding>,
    target: HintTarget,
    tab: TabId,
    index: i32,
    view: QString,
    generations: AnnotationGenerations,
}

/// Opaque native box. The retained Arc is also the current collection token;
/// clearing/replacing it fences stop→restart even when every value is equal.
#[derive(Default)]
pub(crate) struct HintRequestCapture {
    request: Option<Arc<RetainedHintRequest>>,
    pending: bool,
    annotation: Option<AnnotationRequest>,
    observation: ObservationDocuments,
    work: Option<CollectionWork>,
    ledger: Option<Box<CollectionLedger>>,
    native_binding: Option<NativeDocumentBinding>,
    validation: NativeValidation,
    collection: Option<(Vec<ferric_browser_core::HintCandidate>, CheckedAnnotations)>,
    selection: Option<FrozenAnchor>,
}

/// A selected browser code and its original checked witness. Confirmation is
/// one-shot freshness evidence; existing action permissions remain separate.
pub(super) struct FrozenAnchor {
    request: Arc<RetainedHintRequest>,
    session: ferric_browser_core::HintSessionId,
    label: String,
    slot: usize,
    proof: Arc<CheckedAnnotations>,
}

impl FrozenAnchor {
    fn is_current(&self, rust: &BrowserUiRust) -> bool {
        self.request.is_current(rust)
            && rust
                .interaction
                .hint_session
                .as_ref()
                .is_some_and(|session| session.id == self.session)
            && rust
                .interaction
                .hint_interaction
                .as_ref()
                .is_some_and(|interaction| {
                    interaction.hint_selection_is_validated(&self.label)
                        && interaction
                            .reservations()
                            .get(self.slot)
                            .is_some_and(|reservation| {
                                reservation.declaration().code == self.label
                                    && reservation.state()
                                        == ferric_browser_core::HintReservationState::Live
                            })
                })
    }
}

pub(crate) fn hint_anchor_state(ui: &qobject::BrowserUi, label: &QString) -> QString {
    let label = label.to_string();
    let Some(interaction) = ui.rust().interaction.hint_interaction.as_ref() else {
        return QString::from("closed");
    };
    if !interaction.hint_selection_is_validated(&label) {
        return QString::from("pending");
    }
    let Some(reservation) = interaction
        .reservations()
        .iter()
        .find(|reservation| reservation.declaration().code == label)
    else {
        return QString::from("ordinary");
    };
    QString::from(
        if reservation.state() == ferric_browser_core::HintReservationState::Live {
            "live"
        } else {
            "closed"
        },
    )
}

pub(crate) fn freeze_hint_anchor(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    label: &QString,
) -> bool {
    if captured.selection.is_some() || hint_anchor_state(ui, label).to_string() != "live" {
        return false;
    }
    let rust = ui.rust();
    let Some(request) = captured
        .request
        .as_ref()
        .filter(|request| request.is_current(rust))
    else {
        return false;
    };
    if request.preview.is_some() {
        return false;
    }
    let (Some(session), Some(interaction), Some(proof)) = (
        &rust.interaction.hint_session,
        &rust.interaction.hint_interaction,
        &rust.interaction.hint_policy_proof,
    ) else {
        return false;
    };
    if session.target != request.target
        || session.configuration_revision != rust.interaction.hint_configuration_revision
        || request
            .policy
            .as_ref()
            .is_none_or(|policy| policy.revision() != proof.policy_snapshot().revision())
    {
        return false;
    }
    let label = label.to_string();
    let Some(slot) = interaction
        .reservations()
        .iter()
        .position(|reservation| reservation.declaration().code == label)
    else {
        return false;
    };
    captured.selection = Some(FrozenAnchor {
        request: Arc::clone(request),
        session: session.id,
        label,
        slot,
        proof: Arc::clone(proof),
    });
    true
}

pub(crate) fn hint_anchor_manifest(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    let rust = ui.rust();
    let Some(selection) = captured
        .selection
        .as_ref()
        .filter(|selection| selection.is_current(rust))
    else {
        return QString::default();
    };
    let Some(hint) = rust.interaction.hint_session.as_ref().and_then(|session| {
        session
            .hints
            .iter()
            .find(|hint| hint.label == selection.label)
    }) else {
        return QString::default();
    };
    let Some(declaration) = rust
        .interaction
        .hint_interaction
        .as_ref()
        .and_then(|interaction| interaction.reservations().get(selection.slot))
        .map(ferric_browser_core::HintReservation::declaration)
    else {
        return QString::default();
    };
    let policy = selection.proof.policy_snapshot();
    let root = match declaration.category {
        ferric_browser_core::HintReservationCategory::Anchor => policy
            .anchors()
            .iter()
            .find(|anchor| anchor.id == declaration.rule_id)
            .and_then(|anchor| anchor.root.as_ref()),
        ferric_browser_core::HintReservationCategory::Collection => policy
            .collections()
            .iter()
            .find(|collection| collection.id == declaration.rule_id)
            .and_then(|collection| collection.root.as_ref()),
    };
    let focused = root.is_some_and(|root| {
        root.start == ferric_browser_config::InteractionRootStart::FocusedFrame
    });
    QString::from(
        serde_json::json!({"owner":hint.candidate.frame_owner,"focused":focused}).to_string(),
    )
}

pub(super) fn consume_hint_anchor_confirmation(rust: &mut BrowserUiRust, label: &str) -> bool {
    rust.interaction
        .confirmed_hint_anchor
        .take()
        .is_some_and(|confirmation| confirmation.label == label && confirmation.is_current(rust))
}

pub(crate) fn finish_hint_anchor_check(
    mut ui: Pin<&mut qobject::BrowserUi>,
    captured: &mut HintRequestCapture,
    native_valid: bool,
) -> QString {
    let Some(selection) = captured.selection.take() else {
        return QString::from(r#"{"error":"no-anchor-selection"}"#);
    };
    if !selection.is_current(ui.as_ref().rust()) {
        return QString::from(r#"{"error":"anchor-session-changed"}"#);
    }
    let checked = captured.collection.take();
    let valid = native_valid
        && captured
            .request
            .as_ref()
            .is_some_and(|request| request.is_current(ui.as_ref().rust()))
        && checked.as_ref().is_some_and(|(candidates, proof)| {
            let declarations: Vec<_> = ui
                .as_ref()
                .rust()
                .interaction
                .hint_interaction
                .as_ref()
                .into_iter()
                .flat_map(ferric_browser_core::HintInteraction::reservations)
                .map(|reservation| reservation.declaration().clone())
                .collect();
            let live_slots = ui
                .as_ref()
                .rust()
                .interaction
                .hint_interaction
                .as_ref()
                .map(|interaction| {
                    selection
                        .proof
                        .surviving_special_slots(proof, interaction.reservations())
                })
                .unwrap_or_default();
            proof
                .special_assignments(candidates, &declarations, Some(&live_slots))
                .get(selection.slot)
                .is_some_and(|assignment| {
                    matches!(
                        assignment.binding,
                        ferric_browser_core::HintSpecialBinding::Live(_)
                    )
                })
        });
    if valid {
        ui.as_mut()
            .rust_mut()
            .get_mut()
            .interaction
            .confirmed_hint_anchor = Some(selection);
        return QString::from(r#"{"ok":true}"#);
    }
    {
        let mut rust = ui.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.interaction.confirmed_hint_anchor = None;
        if let Some(interaction) = this.interaction.hint_interaction.as_mut() {
            let declaration = interaction.reservations()[selection.slot]
                .declaration()
                .clone();
            let slots: Vec<_> = interaction
                .reservations()
                .iter()
                .enumerate()
                .filter(|(slot, reservation)| {
                    *slot == selection.slot
                        || declaration.category
                            == ferric_browser_core::HintReservationCategory::Collection
                            && reservation.declaration().category == declaration.category
                            && reservation.declaration().rule_id == declaration.rule_id
                })
                .map(|(slot, _)| slot)
                .collect();
            for slot in slots {
                interaction
                    .retire_reservation(slot, ferric_browser_core::HintReservationReason::Stale);
            }
            let hints = interaction.hints().to_vec();
            if let Some(session) = this.interaction.hint_session.as_mut() {
                session.hints.clone_from(&hints);
            }
            this.hint_values = QString::from(
                crate::hint_payload::hint_json(
                    &hints,
                    this.hint_rapid,
                    if this.hint_pointer_mode {
                        "pointer"
                    } else {
                        &this.hint_rapid_target
                    },
                )
                .to_string(),
            );
        }
    }
    let response = ui
        .as_mut()
        .update_hint_interaction(&QString::from("clear"), &QString::default());
    ui.as_mut().set_status_text(QString::from(
        "Policy Hint changed. Start hints again to use its code.",
    ));
    let response: serde_json::Value =
        serde_json::from_str(&response.to_string()).unwrap_or_default();
    QString::from(
        serde_json::json!({"error":"stale-policy-hint","state":response.get("state")}).to_string(),
    )
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeDocumentBinding {
    owner: String,
    depth: u8,
}

#[derive(Default, PartialEq)]
enum NativeValidation {
    #[default]
    NotStarted,
    Active,
    Finished,
}

#[derive(Default)]
enum ObservationDocuments {
    #[default]
    NotStarted,
    PendingFacts(Box<CheckedAnnotationFacts>),
    Pending(CheckedAnnotations),
    Checked(CheckedAnnotations),
    ObjectsChecked(CheckedAnnotations),
    Rejected,
}

fn selected(rust: &BrowserUiRust, tab: TabId, index: i32, view: &QString) -> bool {
    rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.ipc_shutdown_gate
        && !rust.page_window_preparing
        && rust.core_mode == Mode::Hint
        && !rust.interaction.native_prompt_transition
        && !rust.interaction.native_prompt_leases.is_active()
        && rust.active_tab_index == index
        && rust.tab == Some(tab)
        && usize::try_from(index)
            .ok()
            .and_then(|index| rust.tab_ids.get(index))
            == Some(&tab)
        && !view.is_empty()
        && view.len() <= 128
        && rust
            .interaction
            .prompt_view_identities
            .get(&tab)
            .is_some_and(|known| *known == view.to_string())
        && rust.state.as_ref().is_some_and(|state| {
            state.shutdown() == ShutdownState::Running
                && state.active_window() == rust.window
                && rust
                    .window
                    .and_then(|window| state.windows().get(&window))
                    .is_some_and(|window| {
                        window.active_tab == Some(tab)
                            && window.modes.last() == Some(&Mode::Hint)
                            && state.tabs().get(&tab).is_some_and(|page| {
                                page.window == window.id && page.profile == window.profile
                            })
                    })
        })
}

impl RetainedHintRequest {
    pub(super) fn requires_proof(&self) -> bool {
        self.capture_only
            || !self.invocation.options.first
                && self
                    .policy
                    .as_ref()
                    .is_some_and(|policy| policy.enhanced_requested_for(self.family))
    }
    fn is_current(&self, rust: &BrowserUiRust) -> bool {
        if self.preview.is_some() {
            return preview::is_current(self, rust);
        }
        selected(rust, self.tab, self.index, &self.view)
            && self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.is_current_rust(rust))
            && rust
                .interaction
                .hint_request
                .as_ref()
                .is_some_and(|current| std::ptr::eq(self, current.as_ref()))
            && self.invocation == Invocation::capture(rust)
            && self.scope == rust.interaction.active_hint_scope
            && rust
                .interaction
                .cached_hint_policies
                .as_ref()
                .is_some_and(|cache| {
                    let Some(state) = rust.state.as_ref() else {
                        return false;
                    };
                    let Some(window) = rust.window else {
                        return false;
                    };
                    cache
                        .capture(
                            state,
                            window,
                            &rust.current_url,
                            rust.interaction.hint_configuration_revision,
                            &self.invocation.options.family,
                        )
                        .is_ok_and(|(snapshot, selectors)| {
                            Arc::ptr_eq(&snapshot, &self.snapshot)
                                && selectors.revision == self.selectors.revision
                        })
                })
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CaptureFailure {
    Pending,
    Closed,
}

fn capture_failure(rust: &BrowserUiRust) -> CaptureFailure {
    if policy_pending(rust) {
        CaptureFailure::Pending
    } else {
        CaptureFailure::Closed
    }
}

fn capture(
    rust: &mut BrowserUiRust,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
) -> Result<Arc<RetainedHintRequest>, CaptureFailure> {
    // Retire the previous collection even if the replacement cannot be minted.
    rust.interaction.hint_request = None;
    let tab = TabId::from_display(&tab.to_string()).ok_or(CaptureFailure::Closed)?;
    if !selected(rust, tab, index, view) || view_generation == 0 {
        return Err(CaptureFailure::Closed);
    }
    let owner = PolicyRefreshCapture::capture_rust(rust).ok_or(CaptureFailure::Closed)?;
    let app = rust.state.as_ref().ok_or(CaptureFailure::Closed)?;
    let window = rust.window.ok_or(CaptureFailure::Closed)?;
    let target = app.capture_target(tab).ok_or(CaptureFailure::Closed)?;
    let target = HintTarget {
        tab: target.tab,
        generation: target.generation,
        document: target.document,
    };
    let family: InteractionFamily =
        serde_json::from_value(serde_json::Value::String(rust.hint_family.to_string()))
            .map_err(|_| CaptureFailure::Closed)?;
    let (snapshot, selectors) = rust
        .interaction
        .cached_hint_policies
        .as_ref()
        .ok_or_else(|| capture_failure(rust))?
        .capture(
            app,
            window,
            &rust.current_url,
            rust.interaction.hint_configuration_revision,
            &rust.hint_family.to_string(),
        )
        .map_err(|_| capture_failure(rust))?;
    // Non-HTTP browser contexts retain their global discovery snapshot, but
    // have no Interaction consumer. A failed HTTP compilation is different:
    // it must never acquire the ordinary/no-enhancement classification.
    let policy = match snapshot.interaction() {
        Some(interaction) => Some(interaction.policy().ok_or(CaptureFailure::Closed)?.clone()),
        None => None,
    };
    let scope = rust.interaction.active_hint_scope.clone();
    if scope.as_ref().is_some_and(|scope| {
        scope.owner.window != window
            || scope.owner.target != target
            || scope.owner.configuration_revision != rust.interaction.hint_configuration_revision
            || !matches!(family, InteractionFamily::All | InteractionFamily::Actions)
    }) {
        return Err(CaptureFailure::Closed);
    }
    let request_id = rust
        .interaction
        .next_hint_request_id
        .checked_add(1)
        .ok_or(CaptureFailure::Closed)?;
    rust.interaction.next_hint_request_id = request_id;
    let request = Arc::new(RetainedHintRequest {
        owner: Some(owner),
        preview: None,
        capture_only: false,
        policy,
        snapshot,
        selectors: selectors.clone(),
        invocation: Invocation::capture(rust),
        family,
        scope,
        target,
        tab,
        index,
        view: view.clone(),
        // Native/provider viewport and shared collection counters still need
        // integration. Zero here grants no geometry/receiver authority.
        generations: AnnotationGenerations {
            selector_revision: selectors.revision,
            view_generation,
            request_id,
            ..AnnotationGenerations::default()
        },
    });
    rust.interaction.hint_request = Some(Arc::clone(&request));
    Ok(request)
}

pub(crate) fn begin_hint_request(
    mut ui: Pin<&mut qobject::BrowserUi>,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
) -> Box<HintRequestCapture> {
    begin_capture(
        ui.as_mut().rust_mut().get_mut(),
        tab,
        index,
        view,
        view_generation,
    )
}

fn policy_pending(rust: &BrowserUiRust) -> bool {
    rust.state
        .as_ref()
        .is_some_and(|state| match state.interaction_cache_unavailable() {
            Some(InteractionCacheUnavailable::PendingRefresh) => true,
            Some(_) => false,
            None => {
                rust.interaction.interaction_policy_refresh_pending
                    && rust.interaction.cached_hint_policies.is_some()
            }
        })
}

fn begin_capture(
    rust: &mut BrowserUiRust,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
) -> Box<HintRequestCapture> {
    begin_capture_with_schedule(
        rust,
        tab,
        index,
        view,
        view_generation,
        VerificationSchedule::graph_documents(),
    )
}

fn begin_capture_with_schedule(
    rust: &mut BrowserUiRust,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
    schedule: VerificationSchedule,
) -> Box<HintRequestCapture> {
    let result = capture(rust, tab, index, view, view_generation);
    let pending = matches!(result, Err(CaptureFailure::Pending));
    let request = result.ok();
    let ledger = request
        .as_ref()
        .and_then(|request| CollectionLedger::new(request.generations.request_id, schedule));
    Box::new(HintRequestCapture {
        request,
        ledger,
        pending,
        annotation: None,
        ..Default::default()
    })
}

/// Synchronous capture classification only. A pending/closed result supplies no
/// collection token. Ordinary and proof-ready results still require the exact
/// retained request to be current at every later consumer boundary.
pub(crate) fn hint_request_classification(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    QString::from(classification(ui.rust(), captured))
}

fn classification(rust: &BrowserUiRust, captured: &HintRequestCapture) -> &'static str {
    if let Some(request) = &captured.request {
        if !request.is_current(rust) {
            return "closed";
        }
        if request.capture_only {
            return "proof-ready";
        }
        match request.snapshot.interaction().map(|value| value.state()) {
            None
            | Some(InteractionCompilationState::Disabled | InteractionCompilationState::NoOp) => {
                "ordinary"
            }
            Some(InteractionCompilationState::Ready) if request.invocation.options.first => {
                "ordinary"
            }
            Some(InteractionCompilationState::Ready) => "proof-ready",
            Some(InteractionCompilationState::Conflicted) => "closed",
        }
    } else if captured.pending && policy_pending(rust) {
        "pending"
    } else {
        "closed"
    }
}

pub(crate) fn hint_request_is_current(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    tab: &QString,
    index: i32,
    view: &QString,
    url: &QString,
) -> bool {
    let rust = ui.rust();
    captured.request.as_ref().is_some_and(|request| {
        request.tab.to_string() == tab.to_string()
            && request.index == index
            && request.view == *view
            && usize::try_from(url.len()).is_ok_and(|len| len <= ferric_browser_core::MAX_URL_BYTES)
            && canonicalize(&url.to_string())
                .ok()
                .zip(
                    canonicalize(
                        &request
                            .preview
                            .as_ref()
                            .and_then(|preview| preview.receipt().url.clone())
                            .unwrap_or_else(|| rust.current_url.to_string()),
                    )
                    .ok(),
                )
                .is_some_and(|(actual, expected)| actual == expected)
            && request.is_current(rust)
    })
}

/// Observation freshness only; this neither reseals a DOM proof nor grants an
/// action permit. Selection still requires the existing native freshness walk.
pub(super) fn retained_request_is_current(rust: &BrowserUiRust) -> bool {
    rust.interaction
        .hint_request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
}

pub(crate) fn finalize_hint_request(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    roles_json: &QString,
) -> bool {
    finalize(ui.rust(), captured, roles_json)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeDocumentRoles {
    structural_owners: Vec<String>,
    #[serde(deserialize_with = "nullable_receiver")]
    focused_receiver: Option<NativeFocusedReceiver>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeFocusedReceiver {
    owner: String,
    origin: String,
    depth: u8,
    composed_depth: u8,
}

fn nullable_receiver<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NativeFocusedReceiver>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}

fn finalize(rust: &BrowserUiRust, captured: &mut HintRequestCapture, roles_json: &QString) -> bool {
    if captured.annotation.is_some()
        || roles_json.len() > 256 * 1024
        || captured
            .ledger
            .as_deref()
            .is_some_and(CollectionLedger::has_pending)
    {
        return false;
    }
    let Some(request) = captured
        .request
        .as_ref()
        .filter(|request| request.is_current(rust))
    else {
        return false;
    };
    let Some(policy) = &request.policy else {
        return false;
    };
    let encoded = roles_json.to_string();
    if encoded.len() > 256 * 1024 {
        return false;
    }
    let Ok(roles) = serde_json::from_str::<NativeDocumentRoles>(&encoded) else {
        return false;
    };
    let Ok(roles) = AnnotationDocumentRoles::capture(
        roles.structural_owners,
        roles
            .focused_receiver
            .map(|receiver| AnnotationFocusedReceiver {
                owner: receiver.owner,
                origin: receiver.origin,
                depth: receiver.depth,
                composed_depth: receiver.composed_depth,
            }),
    ) else {
        return false;
    };
    let Some(window) = rust.window else {
        return false;
    };
    captured.annotation = AnnotationRequest::capture(
        policy.clone(),
        window,
        request.target,
        request.family,
        request.generations,
        request.scope.as_ref().map(|scope| scope.lease_id.as_str()),
        roles,
    )
    .ok();
    captured.annotation.is_some()
}

/// Stage structurally checked facts against the original request. No native
/// association or receiver/action authority is implied until the native caller
/// performs its continuation; even then this state covers Documents only.
pub(crate) fn prepare_hint_observation_documents(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    candidates: &QString,
    annotation: &QString,
) -> QString {
    prepare_documents(ui.rust(), captured, candidates, annotation)
}

fn prepare_documents(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    candidates: &QString,
    annotation: &QString,
) -> QString {
    if captured.work.is_some() || !matches!(captured.observation, ObservationDocuments::NotStarted)
    {
        return QString::default();
    }
    captured.observation = ObservationDocuments::Rejected;
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
        || !usize::try_from(annotation.len())
            .is_ok_and(|len| len <= MAX_INTERACTION_ANNOTATION_BYTES)
    {
        return QString::default();
    }
    let Some(request) = captured.annotation.as_ref() else {
        return QString::default();
    };
    let Some(ledger) = captured
        .ledger
        .as_mut()
        .filter(|ledger| !ledger.has_pending())
    else {
        return QString::default();
    };
    let verification_limit = ledger.remaining_native().verification_steps;
    if verification_limit == 0 {
        return QString::default();
    }
    // Ordinary rows retain their existing independent decoder and limits.
    // In particular this path never applies the annotation's 2MiB cap to them.
    let Ok(candidates) = crate::parse_hint_payload(&candidates.to_string()) else {
        return QString::default();
    };
    let Ok(facts) = decode_annotation_facts_with_budget(
        &annotation.to_string(),
        request,
        candidates,
        verification_limit,
    ) else {
        ledger.burn_remaining(false);
        return QString::default();
    };
    if !ledger.charge_verification(facts.verification_steps()) {
        ledger.burn_remaining(false);
        return QString::default();
    }
    let Ok(documents) = facts.association_documents_json() else {
        return QString::default();
    };
    captured.observation = ObservationDocuments::PendingFacts(Box::new(facts));
    QString::from(documents)
}

pub(crate) fn begin_hint_collection_work(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    required_owner: &QString,
) -> bool {
    if required_owner.len() > 42 {
        return false;
    }
    begin_work(ui.rust(), captured, &required_owner.to_string())
}

fn begin_work(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    required_owner: &str,
) -> bool {
    if captured.work.is_some() || !matches!(captured.observation, ObservationDocuments::NotStarted)
    {
        return false;
    }
    let Some(request) = captured
        .request
        .as_ref()
        .filter(|request| request.is_current(rust))
    else {
        return false;
    };
    let Some(annotation) = captured.annotation.as_ref() else {
        return false;
    };
    let Some(ledger) = captured.ledger.take() else {
        return false;
    };
    match CollectionWork::capture(
        annotation.selected_collector().cloned(),
        required_owner,
        request.generations.selector_revision,
        ledger,
    ) {
        Ok(work) => captured.work = Some(work),
        Err(ledger) => captured.ledger = Some(ledger),
    }
    captured.work.is_some()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeReservation {
    operation: NativeOperation,
    binding: NativeBinding,
    limit: NativeBudget,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum NativeReportKind {
    Known,
    NoScript,
    Unknown,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeReport {
    outcome: NativeReportKind,
    spent: NativeBudget,
}

fn native_ledger(captured: &mut HintRequestCapture) -> Option<&mut CollectionLedger> {
    match captured.work.as_mut() {
        Some(work) => Some(work.ledger.as_mut()),
        None => captured.ledger.as_deref_mut(),
    }
}

/// Native receipt catalogue commitment, not renderer evidence or an action
/// permit. A later validation cannot replace the original owner/depth binding.
pub(crate) fn commit_hint_document_binding(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    binding: &QString,
) -> bool {
    commit_document_binding(ui.rust(), captured, binding)
}

fn commit_document_binding(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    binding: &QString,
) -> bool {
    if captured.native_binding.is_some()
        || captured.work.is_some()
        || !captured
            .request
            .as_ref()
            .is_some_and(|request| request.is_current(rust))
        || captured
            .ledger
            .as_ref()
            .is_none_or(|ledger| ledger.has_pending())
    {
        return false;
    }
    let Some(binding): Option<NativeDocumentBinding> = bounded_native_json(binding) else {
        return false;
    };
    let valid = binding.depth <= 8
        && (binding.depth != 0 || binding.owner.is_empty())
        && NativeBinding {
            phase: NativePhase::Validation,
            stage: 0,
            owner: binding.owner.clone(),
        }
        .valid();
    if !valid {
        return false;
    }
    if let Some(annotation) = &captured.annotation {
        let matches = annotation
            .selected_collector()
            .map_or(binding.depth == 0 && binding.owner.is_empty(), |receiver| {
                receiver.owner == binding.owner && receiver.depth == binding.depth
            });
        if !matches {
            return false;
        }
    } else if classification(rust, captured) != "ordinary" {
        return false;
    }
    captured.native_binding = Some(binding);
    true
}

pub(crate) fn begin_hint_document_validation(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
) -> bool {
    begin_document_validation(ui.rust(), captured)
}

fn begin_document_validation(rust: &BrowserUiRust, captured: &mut HintRequestCapture) -> bool {
    if captured.validation != NativeValidation::NotStarted
        || captured.native_binding.is_none()
        || !captured
            .request
            .as_ref()
            .is_some_and(|request| request.is_current(rust))
        || native_ledger(captured).is_none_or(|ledger| ledger.has_pending())
    {
        return false;
    }
    captured.validation = NativeValidation::Active;
    true
}

pub(crate) fn finish_hint_document_validation(captured: &mut HintRequestCapture) {
    if captured.validation == NativeValidation::Active {
        captured.validation = NativeValidation::Finished;
    }
}

fn bounded_native_json<T: serde::de::DeserializeOwned>(value: &QString) -> Option<T> {
    if value.len() > 2048 {
        return None;
    }
    let text = value.to_string();
    if text.len() > 2048 {
        return None;
    }
    serde_json::from_str(&text).ok()
}

/// Native-only resource reservation, not a renderer command or proof receipt.
/// Preflight remains closed under the graph-only schedule until its bounded
/// native protocol is integrated; role finalization cannot reset this ledger.
pub(crate) fn reserve_hint_native_work(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    specification: &QString,
) -> QString {
    reserve_native_work(ui.rust(), captured, specification)
}

fn reserve_native_work(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    specification: &QString,
) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return QString::default();
    }
    let Some(specification): Option<NativeReservation> = bounded_native_json(specification) else {
        return QString::default();
    };
    let binding = &specification.binding;
    let eligible = binding.valid()
        && match binding.phase {
            NativePhase::Preflight => captured.work.is_none() && captured.annotation.is_none() && captured.native_binding.is_none(),
            NativePhase::Policy => captured.work.as_ref().is_some_and(|work| {
                work.pending_policy(&binding.stage.to_string(), &binding.owner)
                    .is_some()
            }),
            NativePhase::Projection => captured
                .work
                .as_ref()
                .is_some_and(|work| work.projection_ready(&binding.owner)),
            NativePhase::Validation => captured.validation == NativeValidation::Active
                && captured.native_binding.as_ref().is_some_and(|native| {
                    native.owner == binding.owner && matches!(specification.operation,
                        NativeOperation::DocumentValidation { document, .. } if document <= native.depth)
                }),
        };
    if !eligible {
        return QString::default();
    }
    native_ledger(captured)
        .and_then(|ledger| {
            ledger.reserve_operation(
                specification.operation,
                specification.binding,
                specification.limit,
            )
        })
        .and_then(|grant| serde_json::to_string(&grant).ok())
        .map(QString::from)
        .unwrap_or_default()
}

/// Completion arguments are private native continuations. A malformed report
/// with the original ticket burns its reservation; no QML no-script flag exists.
pub(crate) fn complete_hint_native_work(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    token: &QString,
    binding: &QString,
    report: &QString,
) -> bool {
    complete_native_work(ui.rust(), captured, token, binding, report)
}

fn complete_native_work(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    token: &QString,
    binding: &QString,
    report: &QString,
) -> bool {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return false;
    }
    let (Some(token), Some(binding)): (Option<NativeToken>, Option<NativeBinding>) =
        (bounded_native_json(token), bounded_native_json(binding))
    else {
        return false;
    };
    let outcome = match bounded_native_json::<NativeReport>(report) {
        Some(NativeReport {
            outcome: NativeReportKind::Known,
            spent,
        }) => NativeOutcome::Known(spent),
        Some(NativeReport {
            outcome: NativeReportKind::NoScript,
            spent,
        }) if spent == NativeBudget::default() => NativeOutcome::NoScript,
        Some(_) | None => NativeOutcome::Unknown,
    };
    native_ledger(captured).is_some_and(|ledger| ledger.complete(&token, &binding, outcome))
}

pub(crate) fn hint_collection_work_plan(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    work_plan(ui.rust(), captured)
}

fn work_plan(rust: &BrowserUiRust, captured: &HintRequestCapture) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return QString::default();
    }
    captured
        .work
        .as_ref()
        .and_then(CollectionWork::plan_json)
        .map(QString::from)
        .unwrap_or_default()
}

pub(crate) fn accept_hint_collection_ordinary(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    stage: &QString,
    owner: &QString,
    candidates: &QString,
    metadata: &QString,
) -> bool {
    if stage.len() != 1 || owner.len() > 42 {
        return false;
    }
    let stage = stage.to_string();
    let owner = owner.to_string();
    if !usize::try_from(metadata.len()).is_ok_and(|bytes| bytes <= MAX_METADATA_BYTES) {
        reject_work(ui.rust(), captured, &stage, &owner);
        return false;
    }
    accept_ordinary(
        ui.rust(),
        captured,
        &stage,
        &owner,
        &candidates.to_string(),
        &metadata.to_string(),
    )
}

fn accept_ordinary(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    stage: &str,
    owner: &str,
    candidates: &str,
    metadata: &str,
) -> bool {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return false;
    }
    captured
        .work
        .as_mut()
        .is_some_and(|work| work.accept_ordinary(stage, owner, candidates, metadata))
}

/// Native-only raw-world staging. Duplicate callbacks cannot cancel pending
/// work; malformed first submissions reserve their matching grant.
pub(crate) fn prepare_hint_collection_policy(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    stage: &QString,
    owner: &QString,
    annotation: &QString,
    metadata: &QString,
) -> bool {
    if stage.len() != 1 || owner.len() > 42 {
        return false;
    }
    let stage = stage.to_string();
    let owner = owner.to_string();
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(ui.rust()))
        || !captured
            .work
            .as_ref()
            .is_some_and(|work| work.can_prepare_policy(&stage, &owner))
    {
        return false;
    }
    if !usize::try_from(metadata.len()).is_ok_and(|bytes| bytes <= MAX_METADATA_BYTES)
        || !usize::try_from(annotation.len())
            .is_ok_and(|bytes| bytes <= MAX_INTERACTION_ANNOTATION_BYTES)
    {
        reject_work(ui.rust(), captured, &stage, &owner);
        return false;
    }
    let Some(request) = captured.annotation.as_ref() else {
        return false;
    };
    captured.work.as_mut().is_some_and(|work| {
        work.prepare_policy(
            &stage,
            &owner,
            &annotation.to_string(),
            &metadata.to_string(),
            request,
        )
    })
}

fn pending_policy<'a>(
    rust: &BrowserUiRust,
    captured: &'a HintRequestCapture,
    stage: &str,
    owner: &str,
) -> Option<PendingPolicy<'a>> {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return None;
    }
    captured.work.as_ref()?.pending_policy(stage, owner)
}

pub(crate) fn hint_collection_policy_manifest(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    stage: &QString,
    owner: &QString,
) -> QString {
    if stage.len() != 1 || owner.len() > 42 {
        return QString::default();
    }
    let stage = stage.to_string();
    let owner = owner.to_string();
    let value = (|| {
        let pending = pending_policy(ui.rust(), captured, &stage, &owner)?;
        Some(
            serde_json::json!({
                "owner":owner, "nonce":pending.nonce, "revision":pending.revision.to_string(),
                "stamp":captured.annotation.as_ref()?.stamp_json().ok()?,
                "focused_documents":pending.facts.association_focused_documents_json().ok()?,
                "scope":original_scope_json(captured.request.as_ref()?)?,
                "annotation_bytes":pending.annotation_bytes,
                "annotation_limit":pending.annotation_limit,
                "viewport":{"width":pending.viewport[0],"height":pending.viewport[1]},
            })
            .to_string(),
        )
    })();
    value.map(QString::from).unwrap_or_default()
}

pub(crate) fn hint_collection_policy_annotation(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    stage: &QString,
    owner: &QString,
) -> QString {
    if stage.len() != 1 || owner.len() > 42 {
        return QString::default();
    }
    pending_policy(ui.rust(), captured, &stage.to_string(), &owner.to_string())
        .map(|pending| QString::from(pending.facts.original_annotation()))
        .unwrap_or_default()
}

pub(crate) fn hint_collection_policy_geometry(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    stage: &QString,
    owner: &QString,
) -> QString {
    if stage.len() != 1 || owner.len() > 42 {
        return QString::default();
    }
    pending_policy(ui.rust(), captured, &stage.to_string(), &owner.to_string())
        .and_then(|pending| pending.facts.ordinary_geometry_json().ok())
        .map(QString::from)
        .unwrap_or_default()
}

/// Original browser input plus the grant frozen before ordinary execution.
/// This separate native carrier is not limited by the small metadata envelope.
pub(crate) fn hint_collection_policy_input(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    stage: &QString,
    owner: &QString,
) -> QString {
    if stage.len() != 1 || owner.len() > 42 {
        return QString::default();
    }
    policy_ordinary_input(ui.rust(), captured, &stage.to_string(), &owner.to_string())
        .map(QString::from)
        .unwrap_or_default()
}

fn policy_ordinary_input(
    rust: &BrowserUiRust,
    captured: &HintRequestCapture,
    stage: &str,
    owner: &str,
) -> Option<String> {
    let pending = pending_policy(rust, captured, stage, owner)?;
    let request = captured.request.as_ref()?;
    let selectors: serde_json::Value =
        serde_json::from_str(&request.selectors.serialized.to_string()).ok()?;
    Some(
        serde_json::json!({
            "version":1,
            "family":request.invocation.options.family,
            "selectors":selectors,
            "proxy":request.invocation.proxy,
            "scope_lease":request.scope.as_ref().map_or("", |scope|scope.lease_id.as_str()),
            "include_facets":owner.is_empty() && request.invocation.search
                && request.invocation.search_scope.to_string()=="all",
            "deferred":true,
            "budget":pending.ordinary_budget,
        })
        .to_string(),
    )
}

pub(crate) fn complete_hint_collection_policy(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    stage: &QString,
    owner: &QString,
    valid: bool,
) -> bool {
    if stage.len() != 1
        || owner.len() > 42
        || !captured
            .request
            .as_ref()
            .is_some_and(|request| request.is_current(ui.rust()))
    {
        return false;
    }
    captured
        .work
        .as_mut()
        .is_some_and(|work| work.complete_policy(&stage.to_string(), &owner.to_string(), valid))
}

/// Native projections of the original sealed worlds. This one-shot boundary
/// cannot accept a replacement inventory, policy, root graph or candidate ID.
pub(crate) fn finish_hint_collection(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    projections: &QString,
) -> QString {
    if captured.collection.is_some()
        || captured.validation != NativeValidation::Finished
        || !captured
            .request
            .as_ref()
            .is_some_and(|request| request.is_current(ui.rust()))
        || !usize::try_from(projections.len())
            .is_ok_and(|length| length <= MAX_INTERACTION_ANNOTATION_BYTES)
    {
        return QString::default();
    }
    let projections = projections.to_string();
    if projections.len() > MAX_INTERACTION_ANNOTATION_BYTES {
        return QString::default();
    }
    let Ok(projections): Result<WorldProjections, _> = serde_json::from_str(&projections) else {
        return QString::default();
    };
    let Some(request) = captured.annotation.as_ref() else {
        return QString::default();
    };
    let Some(work) = captured.work.take() else {
        return QString::default();
    };
    let Ok((candidates, checked)) = work.finish(request, &projections) else {
        return QString::default();
    };
    let Ok(annotation) = serde_json::to_string(&checked) else {
        return QString::default();
    };
    // Selection confirmation consumes the retained native inventory. QML only
    // needs the annotation, so avoid materializing an unused candidate copy.
    let presented_candidates = if captured.selection.is_some() {
        Vec::new()
    } else {
        candidates
            .iter()
            .map(crate::hint_payload::candidate_json)
            .collect::<Vec<_>>()
    };
    let result = serde_json::json!({
        "candidates":presented_candidates,
        "annotation_json":annotation,
    })
    .to_string();
    captured.collection = Some((candidates, checked));
    QString::from(result)
}

/// Publish the retained browser inventory, never a QML replacement envelope.
pub(crate) fn publish_hint_collection(
    ui: Pin<&mut qobject::BrowserUi>,
    captured: &mut HintRequestCapture,
) -> QString {
    if captured
        .request
        .as_ref()
        .is_some_and(|request| request.preview.is_some())
    {
        return preview::publish(ui, captured);
    }
    if captured.selection.is_some() {
        return QString::from(r#"{"error":"selection-check-cannot-publish"}"#);
    }
    let Some(request) = captured
        .request
        .as_ref()
        .filter(|request| request.is_current(ui.as_ref().rust()))
    else {
        return QString::from(r#"{"error":"stale-policy-collection"}"#);
    };
    let Some((candidates, checked)) = captured.collection.take() else {
        return QString::from(r#"{"error":"policy-collection-already-published"}"#);
    };
    let payload = serde_json::json!({
        "candidates":candidates.iter().map(crate::hint_payload::candidate_json).collect::<Vec<_>>(),
        "selector_revision":format!("{:016x}",request.generations.selector_revision),
        "scope_lease_id":request.scope.as_ref().map(|scope| scope.lease_id.as_str()),
    });
    ui.begin_hint_session_checked(&QString::from(payload.to_string()), Some(checked))
}

pub(crate) fn reject_hint_collection_work(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    stage: &QString,
    owner: &QString,
) -> bool {
    if stage.len() != 1 || owner.len() > 42 {
        return false;
    }
    reject_work(ui.rust(), captured, &stage.to_string(), &owner.to_string())
}

fn reject_work(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    stage: &str,
    owner: &str,
) -> bool {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(rust))
    {
        return false;
    }
    captured
        .work
        .as_mut()
        .is_some_and(|work| work.reject(stage, owner))
}

/// Receiver requirements come from the original checked policy/root references,
/// never from a renderer-authored role list. Native code must prove these IDs.
pub(crate) fn hint_observation_focused_documents(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(ui.rust()))
    {
        return QString::default();
    }
    let documents = match &captured.observation {
        ObservationDocuments::PendingFacts(facts) => facts.association_focused_documents_json(),
        ObservationDocuments::Pending(checked) => checked.association_focused_documents_json(),
        _ => return QString::default(),
    };
    documents.map(QString::from).unwrap_or_default()
}

pub(crate) fn hint_observation_annotation(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(ui.rust()))
    {
        return QString::default();
    }
    let annotation = match &captured.observation {
        ObservationDocuments::PendingFacts(facts) => facts.original_annotation(),
        ObservationDocuments::Pending(checked) => checked.original_annotation(),
        _ => return QString::default(),
    };
    QString::from(annotation)
}

/// Scope identity comes from the original browser permit. Current Rust
/// ownership/generation checks remain authoritative; renderer lease metadata
/// cannot replace its element, namespace or parent lease.
pub(crate) fn hint_observation_scope(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    let Some(request) = captured
        .request
        .as_ref()
        .filter(|request| request.is_current(ui.rust()))
    else {
        return QString::default();
    };
    if !matches!(
        captured.observation,
        ObservationDocuments::PendingFacts(_) | ObservationDocuments::Pending(_)
    ) {
        return QString::default();
    }
    original_scope_json(request)
        .map(QString::from)
        .unwrap_or_default()
}

fn original_scope_json(request: &RetainedHintRequest) -> Option<String> {
    let scope = request.scope.as_ref().map(|scope| {
        serde_json::json!({
            "lease_id":scope.lease_id,"element_id":scope.element_id,"frame_path":scope.frame_path,
            "kind":crate::hint_payload::hint_kind_name(scope.kind),"selector_revision":format!("{:016x}",scope.selector_policy_revision),
            "parent_scope_lease_id":scope.parent_scope_lease_id,
        })
    });
    serde_json::to_string(&scope).ok()
}

pub(crate) fn hint_observation_stamp(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(ui.rust()))
    {
        return QString::default();
    }
    captured
        .annotation
        .as_ref()
        .and_then(|request| request.stamp_json().ok())
        .map(QString::from)
        .unwrap_or_default()
}

pub(crate) fn complete_hint_observation_documents(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    valid: bool,
) -> bool {
    complete_documents(ui.rust(), captured, valid)
}

/// Native-only continuation after the first raw seal, before the final seal.
/// No checked status or Hint session is published by classification.
pub(crate) fn classify_hint_observation(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
) -> bool {
    classify_observation(ui.rust(), captured)
}

fn classify_observation(rust: &BrowserUiRust, captured: &mut HintRequestCapture) -> bool {
    let facts = match std::mem::replace(&mut captured.observation, ObservationDocuments::Rejected) {
        ObservationDocuments::PendingFacts(facts) => facts,
        historical @ (ObservationDocuments::Checked(_)
        | ObservationDocuments::ObjectsChecked(_)) => {
            captured.observation = historical;
            return false;
        }
        _ => return false,
    };
    if !captured.request.as_ref().is_some_and(|request| {
        request.is_current(rust)
            && request
                .policy
                .as_ref()
                .is_some_and(|policy| policy.revision() == facts.policy_snapshot().revision())
    }) {
        return false;
    }
    let Ok(checked) = (*facts).normalize() else {
        return false;
    };
    captured.observation = ObservationDocuments::Pending(checked);
    true
}

pub(crate) fn complete_hint_observation_objects(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
    valid: bool,
) -> bool {
    complete_objects(ui.rust(), captured, valid)
}

fn complete_objects(rust: &BrowserUiRust, captured: &mut HintRequestCapture, valid: bool) -> bool {
    if !complete_documents(rust, captured, valid) {
        return false;
    }
    let ObservationDocuments::Checked(checked) =
        std::mem::replace(&mut captured.observation, ObservationDocuments::Rejected)
    else {
        return false;
    };
    captured.observation = ObservationDocuments::ObjectsChecked(checked);
    true
}

fn complete_documents(
    rust: &BrowserUiRust,
    captured: &mut HintRequestCapture,
    valid: bool,
) -> bool {
    let checked = match std::mem::replace(&mut captured.observation, ObservationDocuments::Rejected)
    {
        ObservationDocuments::Pending(checked) => checked,
        historical @ (ObservationDocuments::Checked(_)
        | ObservationDocuments::ObjectsChecked(_)) => {
            captured.observation = historical;
            return false;
        }
        _ => return false,
    };
    if !valid
        || !captured.request.as_ref().is_some_and(|request| {
            request.is_current(rust)
                && request
                    .policy
                    .as_ref()
                    .is_some_and(|policy| policy.revision() == checked.policy_snapshot().revision())
        })
    {
        return false;
    }
    captured.observation = ObservationDocuments::Checked(checked);
    true
}

/// Historical document-association status, not a live proof or action permit.
pub(crate) fn hint_observation_documents_checked(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> bool {
    documents_checked(ui.rust(), captured)
}

fn documents_checked(rust: &BrowserUiRust, captured: &HintRequestCapture) -> bool {
    let checked = match &captured.observation {
        ObservationDocuments::Checked(checked) | ObservationDocuments::ObjectsChecked(checked) => {
            checked
        }
        _ => return false,
    };
    captured.request.as_ref().is_some_and(|request| {
        request.is_current(rust)
            && request
                .policy
                .as_ref()
                .is_some_and(|policy| policy.revision() == checked.policy_snapshot().revision())
    })
}

/// Historical graph-association status only. Activation still requires its
/// own live semantic witness and browser-owned local action permit.
pub(crate) fn hint_observation_objects_checked(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> bool {
    objects_checked(ui.rust(), captured)
}

fn objects_checked(rust: &BrowserUiRust, captured: &HintRequestCapture) -> bool {
    matches!(
        captured.observation,
        ObservationDocuments::ObjectsChecked(_)
    ) && documents_checked(rust, captured)
}

pub(crate) fn hint_request_plan(ui: &qobject::BrowserUi, captured: &HintRequestCapture) -> QString {
    if !captured
        .request
        .as_ref()
        .is_some_and(|request| request.is_current(ui.rust()))
    {
        return QString::default();
    }
    QString::from(
        captured
            .annotation
            .as_ref()
            .and_then(|request| request.collector_plan_json().ok())
            .unwrap_or_default(),
    )
}

pub(crate) fn hint_request_selectors(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
) -> QString {
    selectors(ui.rust(), captured)
}

fn selectors(rust: &BrowserUiRust, captured: &HintRequestCapture) -> QString {
    captured
        .request
        .as_ref()
        .filter(|request| request.is_current(rust))
        .map(|request| request.selectors.serialized.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_policy::{HintScopePurpose, LocalInteractionOwner};
    use crate::hint_policy_cache::CachedHintPolicies;
    use ferric_browser_application::SitePolicyMatchContext;
    use ferric_browser_core::{Event, HintKind, HintSessionId, ValidatedUrl};
    use ferric_browser_runtime::RuntimeInput;

    const URL: &str = "https://example.test/page";
    const VIEW: &str = "actual-native-view-identity";

    fn ready() -> BrowserUiRust {
        let (mut app, window) = crate::hint_policy_cache::tests::application();
        let tab = app.windows()[&window].active_tab.unwrap();
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
            target: app.capture_target(tab).unwrap(),
            url: ValidatedUrl::parse(URL).unwrap(),
        }))
        .unwrap();
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::PushMode {
            window,
            mode: Mode::Hint,
        }))
        .unwrap();
        let snapshot = app.refresh_site_policy_snapshot(window, URL).unwrap();
        let mut rust = BrowserUiRust::default();
        rust.state = Some(app);
        rust.window = Some(window);
        rust.tab = Some(tab);
        rust.tab_ids = vec![tab];
        rust.active_tab_index = 0;
        rust.current_url = QString::from(URL);
        rust.core_mode = Mode::Hint;
        rust.hint_family = QString::from("all");
        rust.profile_bootstrap_pending = false;
        rust.interaction
            .prompt_view_identities
            .insert(tab, VIEW.into());
        rust.interaction.cached_hint_policies = Some(
            CachedHintPolicies::prepare(
                snapshot,
                SitePolicyMatchContext::for_url(&ValidatedUrl::parse(URL).unwrap()),
                QString::from(URL),
                0,
            )
            .unwrap(),
        );
        rust
    }

    fn begin(rust: &mut BrowserUiRust) -> Arc<RetainedHintRequest> {
        let tab = QString::from(rust.tab.unwrap().to_string());
        capture(rust, &tab, 0, &QString::from(VIEW), 13).expect("current selected native view")
    }

    fn prepare_context(rust: &mut BrowserUiRust, config: ferric_browser_config::Config, url: &str) {
        let tab = rust.tab.unwrap();
        let window = rust.window.unwrap();
        let state = rust.state.as_mut().unwrap();
        state.replace_configuration(config).unwrap();
        state
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
                target: state.capture_target(tab).unwrap(),
                url: ValidatedUrl::parse(url).unwrap(),
            }))
            .unwrap();
        let snapshot = state.refresh_site_policy_snapshot(window, url).unwrap();
        rust.current_url = QString::from(url);
        rust.interaction.cached_hint_policies = Some(
            CachedHintPolicies::prepare(
                snapshot,
                SitePolicyMatchContext::for_url(&ValidatedUrl::parse(url).unwrap()),
                QString::from(url),
                rust.interaction.hint_configuration_revision,
            )
            .unwrap(),
        );
    }

    fn classified(rust: &mut BrowserUiRust) -> Box<HintRequestCapture> {
        let tab = QString::from(rust.tab.unwrap().to_string());
        begin_capture(rust, &tab, 0, &QString::from(VIEW), 13)
    }

    #[test]
    fn native_document_binding_requires_original_roles_and_cannot_be_replaced() {
        let mut rust = ready();
        let config = serde_json::from_value(serde_json::json!({"schema_version":3,"site_rules":[{
            "id":"binding","pattern":"https://example.test/*",
            "interaction":{"enabled":true,"labels":{"fallback_prefix":"z"}}
        }]}))
        .unwrap();
        prepare_context(&mut rust, config, URL);
        let mut captured = classified(&mut rust);
        assert_eq!(classification(&rust, &captured), "proof-ready");
        let binding = QString::from(r#"{"owner":"","depth":0}"#);
        assert!(!begin_document_validation(&rust, &mut captured));
        assert!(!commit_document_binding(&rust, &mut captured, &binding));
        assert!(finalize(
            &rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        for invalid in [
            r#"{"owner":"","depth":1}"#,
            r#"{"owner":"bad","depth":0}"#,
            r#"{"owner":"","depth":9}"#,
            r#"{"owner":"","depth":0,"extra":true}"#,
        ] {
            assert!(!commit_document_binding(
                &rust,
                &mut captured,
                &QString::from(invalid)
            ));
        }
        assert!(commit_document_binding(&rust, &mut captured, &binding));
        assert!(!commit_document_binding(&rust, &mut captured, &binding));
        rust.current_url = QString::from("https://example.test/changed");
        assert!(!begin_document_validation(&rust, &mut captured));
    }

    #[test]
    fn committed_validation_survives_collection_start_with_bounded_original_positions() {
        let mut rust = ready();
        let mut captured = classified(&mut rust);
        assert!(finalize(
            &rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        assert!(commit_document_binding(
            &rust,
            &mut captured,
            &QString::from(r#"{"owner":"","depth":0}"#)
        ));
        assert!(begin_work(&rust, &mut captured, ""));
        assert!(begin_document_validation(&rust, &mut captured));
        assert!(!begin_document_validation(&rust, &mut captured));
        let specification = |document: u8, pass: u8, owner: &str| {
            QString::from(
                serde_json::json!({
                    "operation":{"kind":"document-validation","document":document,"pass":pass},
                    "binding":{"phase":"validation","stage":0,"owner":owner},
                    "limit":{"inspections":1,"verification_steps":0,"annotation_bytes":0}
                })
                .to_string(),
            )
        };
        for invalid in [
            specification(1, 0, ""),
            specification(0, 2, ""),
            specification(0, 0, "frame-22222222-2222-4222-8222-222222222222"),
        ] {
            assert!(reserve_native_work(&rust, &mut captured, &invalid).is_empty());
        }
        assert!(!native_ledger(&mut captured).unwrap().has_pending());
        let grant = reserve_native_work(&rust, &mut captured, &specification(0, 0, ""));
        let grant: serde_json::Value = serde_json::from_str(&grant.to_string()).unwrap();
        assert!(complete_native_work(
            &rust,
            &mut captured,
            &QString::from(grant["token"].to_string()),
            &QString::from(grant["token"]["binding"].to_string()),
            &QString::from(
                r#"{"outcome":"known","spent":{"inspections":1,"verification_steps":0,"annotation_bytes":0}}"#
            )
        ));
        finish_hint_document_validation(&mut captured);
        assert!(reserve_native_work(&rust, &mut captured, &specification(0, 1, "")).is_empty());
        assert!(!begin_document_validation(&rust, &mut captured));
    }

    #[test]
    fn ordinary_native_binding_also_closes_preflight_and_validation_renewal() {
        let mut rust = ready();
        prepare_context(&mut rust, ferric_browser_config::Config::default(), URL);
        let mut captured = classified(&mut rust);
        assert_eq!(classification(&rust, &captured), "ordinary");
        assert!(commit_document_binding(
            &rust,
            &mut captured,
            &QString::from(r#"{"owner":"","depth":0}"#)
        ));
        assert!(reserve_native_work(&rust, &mut captured, &QString::from(
            r#"{"operation":{"kind":"document-facts","checkpoint":"bind-top","document":0,"pass":0},"binding":{"phase":"preflight","stage":0,"owner":""},"limit":{"inspections":1,"verification_steps":0,"annotation_bytes":0}}"#)).is_empty());
        assert!(begin_document_validation(&rust, &mut captured));
        finish_hint_document_validation(&mut captured);
        assert!(!begin_document_validation(&rust, &mut captured));
    }

    #[test]
    fn original_native_ledger_is_preserved_through_role_finalization_and_collection_start() {
        let mut rust = ready();
        let tab = QString::from(rust.tab.unwrap().to_string());
        let mut captured = begin_capture_with_schedule(
            &mut rust,
            &tab,
            0,
            &QString::from(VIEW),
            13,
            VerificationSchedule::testing(
                100,
                NativeBudget {
                    inspections: 10,
                    verification_steps: 20,
                    annotation_bytes: 30,
                },
            ),
        );
        let specification = QString::from(
            r#"{"operation":{"kind":"document-facts","checkpoint":"bind-top","document":0,"pass":0},"binding":{"phase":"preflight","stage":0,"owner":""},"limit":{"inspections":10,"verification_steps":20,"annotation_bytes":30}}"#,
        );
        let untyped = QString::from(
            r#"{"operation":65535,"binding":{"phase":"preflight","stage":0,"owner":""},"limit":{"inspections":10,"verification_steps":20,"annotation_bytes":30}}"#,
        );
        assert!(reserve_native_work(&rust, &mut captured, &untyped).is_empty());
        assert!(!captured.ledger.as_ref().unwrap().has_pending());
        let uncommitted_validation = QString::from(
            r#"{"operation":{"kind":"document-validation","document":0,"pass":0},"binding":{"phase":"validation","stage":0,"owner":""},"limit":{"inspections":1,"verification_steps":0,"annotation_bytes":0}}"#,
        );
        assert!(reserve_native_work(&rust, &mut captured, &uncommitted_validation).is_empty());
        assert!(!captured.ledger.as_ref().unwrap().has_pending());
        let grant = reserve_native_work(&rust, &mut captured, &specification);
        assert!(!grant.is_empty());
        let grant: serde_json::Value = serde_json::from_str(&grant.to_string()).unwrap();
        let token = QString::from(grant["token"].to_string());
        let binding = QString::from(grant["token"]["binding"].to_string());
        let roles = QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#);
        assert!(!finalize(&rust, &mut captured, &roles));
        assert!(!begin_work(&rust, &mut captured, ""));
        assert!(complete_native_work(
            &rust,
            &mut captured,
            &token,
            &binding,
            &QString::from(
                r#"{"outcome":"known","spent":{"inspections":3,"verification_steps":4,"annotation_bytes":5}}"#
            )
        ));
        assert!(finalize(&rust, &mut captured, &roles));
        assert!(begin_work(&rust, &mut captured, ""));
        assert!(captured.ledger.is_none());
        let work = captured.work.as_ref().unwrap();
        let plan: serde_json::Value = serde_json::from_str(&work.plan_json().unwrap()).unwrap();
        assert_eq!(plan["budget"]["inspections"], 49997);
        assert_eq!(
            plan["budget"]["annotation_bytes"],
            MAX_INTERACTION_ANNOTATION_BYTES - 5
        );
        assert_eq!(plan["budget"]["predicate_steps"], 100_000);
        assert_eq!(work.ledger.remaining_native().verification_steps, 96);
        assert!(!complete_native_work(
            &rust,
            &mut captured,
            &token,
            &binding,
            &QString::from(
                r#"{"outcome":"no-script","spent":{"inspections":0,"verification_steps":0,"annotation_bytes":0}}"#
            )
        ));
    }

    #[test]
    fn matching_malformed_native_report_burns_without_publishing_any_proof() {
        let mut rust = ready();
        let tab = QString::from(rust.tab.unwrap().to_string());
        let mut captured = begin_capture_with_schedule(
            &mut rust,
            &tab,
            0,
            &QString::from(VIEW),
            13,
            VerificationSchedule::testing(
                100,
                NativeBudget {
                    inspections: 10,
                    verification_steps: 20,
                    annotation_bytes: 30,
                },
            ),
        );
        let grant = reserve_native_work(
            &rust,
            &mut captured,
            &QString::from(
                r#"{"operation":{"kind":"document-facts","checkpoint":"bind-top","document":0,"pass":0},"binding":{"phase":"preflight","stage":0,"owner":""},"limit":{"inspections":10,"verification_steps":20,"annotation_bytes":30}}"#,
            ),
        );
        let grant: serde_json::Value = serde_json::from_str(&grant.to_string()).unwrap();
        let token = QString::from(grant["token"].to_string());
        let binding = QString::from(grant["token"]["binding"].to_string());
        assert!(!complete_native_work(
            &rust,
            &mut captured,
            &token,
            &binding,
            &QString::from("{")
        ));
        assert_eq!(
            native_ledger(&mut captured).unwrap().remaining_native(),
            NativeBudget {
                inspections: 49990,
                verification_steps: 80,
                annotation_bytes: MAX_INTERACTION_ANNOTATION_BYTES - 30
            }
        );
        assert!(!complete_native_work(
            &rust,
            &mut captured,
            &token,
            &binding,
            &QString::from(
                r#"{"outcome":"no-script","spent":{"inspections":0,"verification_steps":0,"annotation_bytes":0}}"#
            )
        ));
        assert!(captured.annotation.is_none());
        assert!(captured.work.is_none());
        assert!(!documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
    }

    #[test]
    fn empty_rule_inventories_do_not_classify_enhanced_labels_or_page_keys_as_ordinary() {
        for (patch, expected) in [
            (serde_json::json!({"enabled":false}), "ordinary"),
            (serde_json::json!({"enabled":true}), "ordinary"),
            (
                serde_json::json!({"enabled":true,"labels":{"fallback_prefix":"z"}}),
                "proof-ready",
            ),
            (
                serde_json::json!({"enabled":true,"hint_page_keys":["k"]}),
                "proof-ready",
            ),
        ] {
            let mut rust = ready();
            let config =
                serde_json::from_value(serde_json::json!({"schema_version":3,"site_rules":[{
                    "id":"classification","pattern":"https://example.test/*","interaction":patch
                }]}))
                .unwrap();
            prepare_context(&mut rust, config, URL);
            let compilations = rust.state.as_ref().unwrap().interaction_compilation_count();
            let captured = classified(&mut rust);
            let request = captured.request.as_ref().unwrap();
            let policy = request.policy.as_ref().unwrap();
            assert!(policy.anchors().is_empty() && policy.collections().is_empty());
            assert_eq!(classification(&rust, &captured), expected);
            assert!(!selectors(&rust, &captured).is_empty());
            assert_eq!(
                rust.state.as_ref().unwrap().interaction_compilation_count(),
                compilations
            );
            begin(&mut rust);
            assert_eq!(classification(&rust, &captured), "closed");
        }
    }

    #[test]
    fn non_http_ordinary_capture_retains_global_selectors_without_an_interaction_policy() {
        for url in ["file:///tmp/ferric-classification.html", "about:blank"] {
            let mut rust = ready();
            let mut config = ferric_browser_config::Config::default();
            config.hints.selectors.insert(
                ferric_browser_config::HintSelectorFamily::All,
                toml::from_str("mode='replace'\nselectors=['#global-sentinel']").unwrap(),
            );
            prepare_context(&mut rust, config, url);
            let mut captured = classified(&mut rust);
            assert_eq!(classification(&rust, &captured), "ordinary");
            let request = captured.request.as_ref().unwrap();
            assert!(request.policy.is_none());
            let original = request.selectors.serialized.clone();
            assert_eq!(selectors(&rust, &captured), original);
            let decoded: serde_json::Value = serde_json::from_str(&original.to_string()).unwrap();
            assert_eq!(decoded["policies"]["all"]["policy"]["mode"], "replace");
            assert_eq!(
                decoded["policies"]["all"]["policy"]["selectors"],
                serde_json::json!(["#global-sentinel"])
            );
            assert!(!finalize(
                &rust,
                &mut captured,
                &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
            ));
            rust.state
                .as_mut()
                .unwrap()
                .replace_configuration(ferric_browser_config::Config::default())
                .unwrap();
            assert_eq!(classification(&rust, &captured), "closed");
            assert!(selectors(&rust, &captured).is_empty());
        }
    }

    #[test]
    fn pending_preparation_and_invalid_capture_are_distinct_and_never_supply_a_token() {
        let mut rust = ready();
        let previous = classified(&mut rust);
        rust.state
            .as_mut()
            .unwrap()
            .invalidate_interaction_snapshot();
        let pending = classified(&mut rust);
        assert!(pending.request.is_none());
        assert_eq!(classification(&rust, &pending), "pending");
        assert_eq!(classification(&rust, &previous), "closed");
        assert!(selectors(&rust, &pending).is_empty());
        let tab = QString::from(rust.tab.unwrap().to_string());
        let invalid = begin_capture(&mut rust, &tab, 1, &QString::from(VIEW), 13);
        assert!(invalid.request.is_none());
        assert_eq!(classification(&rust, &invalid), "closed");
        prepare_context(&mut rust, ferric_browser_config::Config::default(), URL);
        assert_eq!(classification(&rust, &pending), "closed");
        let fresh = classified(&mut rust);
        assert_eq!(classification(&rust, &fresh), "ordinary");
    }

    #[test]
    fn composed_policy_conflict_cannot_be_reclassified_as_ordinary_discovery() {
        let mut rust = ready();
        // Install individually valid sources before entering their matching
        // context; publication must still reject a new conflict on an open URL.
        prepare_context(
            &mut rust,
            ferric_browser_config::Config::default(),
            "about:blank",
        );
        // The two array fields survive composition independently. Two anchor
        // lists would replace one another and correctly produce no conflict.
        let config=serde_json::from_value(serde_json::json!({"schema_version":3,"site_rules":[
            {"id":"anchor-source","pattern":"https://example.test/*","interaction":{"enabled":true,"anchors":[{
                "id":"one","code":"m","families":["all"],"target_selector":"button"
            }]}},
            {"id":"collection-source","pattern":"https://example.test/*","interaction":{"collections":[{
                "id":"two","codes":["m"],"families":["all"],"container_self":true,"item_selector":"button",
                "primary_self":true,"order":"dom"
            }]}}
        ]})).unwrap();
        prepare_context(&mut rust, config, URL);
        assert_eq!(
            rust.state
                .as_ref()
                .unwrap()
                .cached_site_policy_snapshot(
                    rust.window.unwrap(),
                    &SitePolicyMatchContext::for_url(&ValidatedUrl::parse(URL).unwrap()),
                )
                .unwrap()
                .interaction()
                .unwrap()
                .state(),
            InteractionCompilationState::Conflicted
        );
        let captured = classified(&mut rust);
        assert!(captured.request.is_none());
        assert_eq!(classification(&rust, &captured), "closed");
        assert!(selectors(&rust, &captured).is_empty());
    }

    fn observation_fixture(rust: &mut BrowserUiRust) -> (HintRequestCapture, QString) {
        let request = begin(rust);
        let mut captured = HintRequestCapture {
            ledger: CollectionLedger::new(
                request.generations.request_id,
                VerificationSchedule::graph_decode(),
            ),
            request: Some(request),
            ..Default::default()
        };
        assert!(finalize(
            rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        let stamp: serde_json::Value =
            serde_json::from_str(&captured.annotation.as_ref().unwrap().stamp_json().unwrap())
                .unwrap();
        let annotation = QString::from(
            serde_json::json!({"version":1,"stamp":stamp,"documents":[],"roots":[],"outcomes":[]})
                .to_string(),
        );
        (captured, annotation)
    }

    #[test]
    fn world_work_retains_original_capture_and_never_publishes_checked_status() {
        let mut rust = ready();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        assert!(begin_work(&rust, &mut captured, ""));
        assert!(!begin_work(&rust, &mut captured, ""));
        let first = work_plan(&rust, &captured);
        let plan: serde_json::Value = serde_json::from_str(&first.to_string()).unwrap();
        assert_eq!(plan["world"], "top");
        assert_eq!(plan["phase"], "ordinary");
        assert!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation).is_empty()
        );
        let revision = captured
            .request
            .as_ref()
            .unwrap()
            .generations
            .selector_revision;
        let metadata = serde_json::json!({"nonce":"11111111-1111-4111-8111-111111111111",
            "revision":1,"selector_revision":format!("{revision:016x}"),"inspections":7,
            "predicate_steps":2,"truncated":false,"viewport":{"width":800,"height":600}})
        .to_string();
        assert!(accept_ordinary(
            &rust,
            &mut captured,
            "1",
            "",
            "[]",
            &metadata
        ));
        let policy: serde_json::Value =
            serde_json::from_str(&work_plan(&rust, &captured).to_string()).unwrap();
        assert_eq!(policy["phase"], "policy");
        assert_eq!(policy["budget"]["inspections"], 49993);
        assert!(!classify_observation(&rust, &mut captured));
        assert!(!complete_objects(&rust, &mut captured, true));
        assert!(!documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
        assert!(rust.interaction.hint_session.is_none());
        begin(&mut rust);
        assert!(work_plan(&rust, &captured).is_empty());
        assert!(!accept_ordinary(
            &rust,
            &mut captured,
            "1",
            "",
            "[]",
            &metadata
        ));
    }

    #[test]
    fn raw_observations_cannot_complete_before_classification() {
        let mut rust = ready();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        assert_eq!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation),
            QString::from("[]")
        );
        assert!(!complete_documents(&rust, &mut captured, true));
        assert!(!classify_observation(&rust, &mut captured));
        assert!(!documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
        assert!(rust.interaction.hint_session.is_none());
    }

    #[test]
    fn native_policy_input_uses_original_flags_selectors_and_issued_ordinary_grant() {
        let mut rust = ready();
        rust.hint_family = QString::from("links");
        rust.hint_proxy_targets_enabled = true;
        rust.target_search_active = true;
        rust.target_search_scope = QString::from("all");
        let (mut captured, annotation) = observation_fixture(&mut rust);
        let selectors: serde_json::Value = serde_json::from_str(
            &captured
                .request
                .as_ref()
                .unwrap()
                .selectors
                .serialized
                .to_string(),
        )
        .unwrap();
        assert!(begin_work(&rust, &mut captured, ""));
        let revision = captured
            .request
            .as_ref()
            .unwrap()
            .generations
            .selector_revision;
        let mut metadata = serde_json::json!({"nonce":"11111111-1111-4111-8111-111111111111",
            "revision":1,"selector_revision":format!("{revision:016x}"),"inspections":7,
            "predicate_steps":2,"truncated":false,"viewport":{"width":800,"height":600}});
        assert!(accept_ordinary(
            &rust,
            &mut captured,
            "1",
            "",
            "[]",
            &metadata.to_string()
        ));
        let mut raw: serde_json::Value = serde_json::from_str(&annotation.to_string()).unwrap();
        raw["discovery_work"] = serde_json::json!({"inspections":7,"predicate_steps":2});
        let raw = raw.to_string();
        metadata["inspections"] = serde_json::json!(0);
        metadata["predicate_steps"] = serde_json::json!(0);
        metadata["annotation_bytes"] = serde_json::json!(raw.len());
        let request = captured.annotation.as_ref().unwrap().clone();
        assert!(captured.work.as_mut().unwrap().prepare_policy(
            "2",
            "",
            &raw,
            &metadata.to_string(),
            &request
        ));
        let original = policy_ordinary_input(&rust, &captured, "2", "").unwrap();
        let input: serde_json::Value = serde_json::from_str(&original).unwrap();
        assert_eq!(input["selectors"], selectors);
        assert_eq!(input["family"], "links");
        assert_eq!(input["proxy"], true);
        assert_eq!(input["scope_lease"], "");
        assert_eq!(input["include_facets"], true);
        assert_eq!(input["deferred"], true);
        assert_eq!(
            input["budget"]["inspections"],
            ferric_browser_core::MAX_HINT_INSPECTIONS
        );
        assert_eq!(
            input["budget"]["candidates"],
            ferric_browser_core::MAX_HINT_CANDIDATES
        );
        assert_eq!(
            input["budget"]["predicate_steps"],
            ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS
        );
        assert!(policy_ordinary_input(&rust, &captured, "1", "").is_none());
        rust.hint_proxy_targets_enabled = false;
        assert!(policy_ordinary_input(&rust, &captured, "2", "").is_none());
        rust.hint_proxy_targets_enabled = true;
        assert_eq!(
            policy_ordinary_input(&rust, &captured, "2", "").unwrap(),
            original
        );
    }

    #[test]
    fn exhausted_original_ledger_blocks_document_preparation_before_decoding() {
        let mut rust = ready();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        let ledger = captured.ledger.as_mut().unwrap();
        let remaining = ledger.remaining_native().verification_steps;
        assert!(ledger.charge_verification(remaining));
        assert!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation).is_empty()
        );
        assert!(matches!(
            captured.observation,
            ObservationDocuments::Rejected
        ));
        assert_eq!(
            captured
                .ledger
                .as_ref()
                .unwrap()
                .remaining_native()
                .verification_steps,
            0
        );
        assert!(!documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
    }

    #[test]
    fn raw_structure_preparation_defers_strict_predicate_rejection() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/site-policy/witness-observed-anchor.json"
        ))
        .unwrap();
        let mut patch = fixture["patch"].clone();
        patch["anchors"][0]["predicates"] =
            serde_json::json!({"hint_name":{"equals":"Different name"}});
        let config = serde_json::from_value(serde_json::json!({"schema_version":3,"site_rules":[{
            "id":"raw-stage","pattern":"https://example.test/*","interaction":patch
        }]}))
        .unwrap();
        let mut rust = ready();
        rust.hint_family = QString::from("links");
        prepare_context(&mut rust, config, URL);
        let (mut captured, _) = observation_fixture(&mut rust);
        let mut annotation = fixture["annotation"].clone();
        annotation["stamp"] =
            serde_json::from_str(&captured.annotation.as_ref().unwrap().stamp_json().unwrap())
                .unwrap();
        let result = annotation["outcomes"][0]["result"].as_object_mut().unwrap();
        let mut complete = result.remove("anchor-observed").unwrap();
        complete["matches"] = complete.as_object_mut().unwrap().remove("targets").unwrap();
        result.insert("anchor-complete".into(), complete);
        let documents = prepare_documents(
            &rust,
            &mut captured,
            &QString::from(fixture["candidates"].to_string()),
            &QString::from(annotation.to_string()),
        );
        assert!(
            !documents.is_empty(),
            "structure must reach the native seal"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&documents.to_string()).unwrap(),
            annotation["documents"]
        );
        let ObservationDocuments::PendingFacts(facts) = &captured.observation else {
            panic!("raw structural facts must be retained")
        };
        let spent = facts.verification_steps();
        assert!(spent > 0);
        assert_eq!(
            captured.ledger.as_ref().unwrap().used.verification_steps,
            spent
        );
        assert!(!classify_observation(&rust, &mut captured));
        assert!(!complete_objects(&rust, &mut captured, true));
        assert!(!documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
        assert!(rust.interaction.hint_session.is_none());
    }

    #[test]
    fn observation_document_staging_is_one_shot_and_cannot_create_a_hint_session() {
        let mut rust = ready();
        let compilations = rust.state.as_ref().unwrap().interaction_compilation_count();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        assert_eq!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation),
            QString::from("[]")
        );
        assert!(!documents_checked(&rust, &captured));
        assert!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation).is_empty()
        );
        assert!(classify_observation(&rust, &mut captured));
        assert!(!documents_checked(&rust, &captured));
        assert!(complete_documents(&rust, &mut captured, true));
        assert!(documents_checked(&rust, &captured));
        assert!(!objects_checked(&rust, &captured));
        assert!(!complete_documents(&rust, &mut captured, true));
        assert!(documents_checked(&rust, &captured));
        assert!(rust.interaction.hint_session.is_none());
        assert_eq!(
            rust.state.as_ref().unwrap().interaction_compilation_count(),
            compilations
        );
    }

    #[test]
    fn object_association_requires_its_own_native_completion_and_original_capture() {
        let mut rust = ready();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        assert_eq!(
            prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation),
            QString::from("[]")
        );
        let ObservationDocuments::PendingFacts(facts) = &captured.observation else {
            panic!("pending original observations")
        };
        assert_eq!(facts.original_annotation(), annotation.to_string());
        assert!(!objects_checked(&rust, &captured));
        assert!(classify_observation(&rust, &mut captured));
        assert!(!objects_checked(&rust, &captured));
        assert!(complete_objects(&rust, &mut captured, true));
        assert!(documents_checked(&rust, &captured));
        assert!(objects_checked(&rust, &captured));
        assert!(!complete_objects(&rust, &mut captured, true));
        assert!(rust.interaction.hint_session.is_none());
        begin(&mut rust);
        assert!(!objects_checked(&rust, &captured));
    }

    #[test]
    fn native_failure_or_replaced_original_request_cannot_finish_staged_observations() {
        for replace in [false, true] {
            let mut rust = ready();
            let (mut captured, annotation) = observation_fixture(&mut rust);
            assert!(
                !prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation)
                    .is_empty()
            );
            if replace {
                begin(&mut rust);
            }
            assert!(!complete_documents(&rust, &mut captured, replace));
            assert!(!documents_checked(&rust, &captured));
            assert!(!complete_documents(&rust, &mut captured, true));
            assert!(rust.interaction.hint_session.is_none());
        }
    }

    #[test]
    fn classification_is_one_shot_and_never_publishes_checked_status() {
        for change in [
            "stable",
            "repeat",
            "replace-before",
            "replace-after",
            "native-failure",
        ] {
            let mut rust = ready();
            let (mut captured, annotation) = observation_fixture(&mut rust);
            assert!(
                !prepare_documents(&rust, &mut captured, &QString::from("[]"), &annotation)
                    .is_empty()
            );
            if change == "replace-before" {
                begin(&mut rust);
                assert!(!classify_observation(&rust, &mut captured));
            } else {
                assert!(classify_observation(&rust, &mut captured));
                assert!(!documents_checked(&rust, &captured));
                assert!(!objects_checked(&rust, &captured));
                if change == "replace-after" {
                    begin(&mut rust);
                } else if change == "repeat" {
                    assert!(!classify_observation(&rust, &mut captured));
                }
            }
            assert_eq!(
                complete_objects(&rust, &mut captured, change != "native-failure"),
                change == "stable"
            );
            assert_eq!(objects_checked(&rust, &captured), change == "stable");
            assert!(rust.interaction.hint_session.is_none());
        }
    }

    #[test]
    fn malformed_or_stale_observations_never_leave_a_pending_document_check() {
        let mut rust = ready();
        for failure in ["stamp", "annotation-bytes", "ordinary-count"] {
            let (mut captured, mut annotation) = observation_fixture(&mut rust);
            let mut candidates = QString::from("[]");
            match failure {
                "stamp" => {
                    let mut changed: serde_json::Value =
                        serde_json::from_str(&annotation.to_string()).unwrap();
                    changed["stamp"]["request_id"] = serde_json::json!("999");
                    annotation = QString::from(changed.to_string());
                }
                "annotation-bytes" => {
                    annotation = QString::from(" ".repeat(MAX_INTERACTION_ANNOTATION_BYTES + 1))
                }
                "ordinary-count" => {
                    candidates = QString::from(
                        serde_json::json!(vec![serde_json::Value::Null; 5001]).to_string(),
                    )
                }
                _ => unreachable!(),
            }
            assert!(prepare_documents(&rust, &mut captured, &candidates, &annotation).is_empty());
            assert!(!complete_documents(&rust, &mut captured, true));
            assert!(!documents_checked(&rust, &captured));
        }
    }

    #[test]
    fn observation_document_staging_keeps_ordinary_bytes_outside_the_annotation_cap() {
        let mut rust = ready();
        let (mut captured, annotation) = observation_fixture(&mut rust);
        let href = format!("https://example.test/{}", "x".repeat(8000));
        let rows=(1..=300).map(|id|serde_json::json!({"element_id":id,"kind":"link","frame_path":"0","text":"ordinary","href":href,"geometry":{"x":0.,"y":0.,"width":10.,"height":10.}})).collect::<Vec<_>>();
        let ordinary = serde_json::to_string(&rows).unwrap();
        assert!(ordinary.len() > MAX_INTERACTION_ANNOTATION_BYTES);
        assert_eq!(
            prepare_documents(&rust, &mut captured, &QString::from(ordinary), &annotation),
            QString::from("[]")
        );
        assert!(classify_observation(&rust, &mut captured));
        assert!(!documents_checked(&rust, &captured));
        assert!(complete_documents(&rust, &mut captured, true));
        assert!(documents_checked(&rust, &captured));
    }

    #[test]
    fn original_capture_and_one_shot_finalization_do_not_compile_or_recapture_policy() {
        let mut rust = ready();
        let compilations = rust.state.as_ref().unwrap().interaction_compilation_count();
        let request = begin(&mut rust);
        assert!(request.is_current(&rust));
        let mut captured = HintRequestCapture {
            request: Some(Arc::clone(&request)),
            annotation: None,
            ..Default::default()
        };
        assert_eq!(selectors(&rust, &captured), request.selectors.serialized);
        assert!(finalize(
            &rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        assert!(
            !finalize(
                &rust,
                &mut captured,
                &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
            ),
            "finalization cannot replace the original catalogue"
        );
        let annotation = captured.annotation.as_ref().unwrap();
        let original_stamp = annotation.stamp_json().unwrap();
        let stamp: serde_json::Value = serde_json::from_str(&original_stamp).unwrap();
        assert_eq!(stamp["request_id"], "1");
        assert_eq!(stamp["view_generation"], "13");
        assert_eq!(
            stamp["selector_revision"],
            request.selectors.revision.to_string()
        );
        assert_eq!(stamp["family"], "all");
        assert_eq!(stamp["tab"], request.tab.to_string());
        assert_eq!(
            rust.state.as_ref().unwrap().interaction_compilation_count(),
            compilations
        );
        rust.state
            .as_mut()
            .unwrap()
            .invalidate_interaction_snapshot();
        assert!(!request.is_current(&rust));
        assert!(selectors(&rust, &captured).is_empty());
        assert_eq!(
            annotation.stamp_json().unwrap(),
            original_stamp,
            "stamp values are immutable"
        );
    }

    #[test]
    fn failed_replacement_and_same_value_restart_never_revive_original_request() {
        let mut rust = ready();
        let first = begin(&mut rust);
        let second = begin(&mut rust);
        assert!(!first.is_current(&rust));
        assert!(second.is_current(&rust));
        assert_eq!(
            second.generations.request_id,
            first.generations.request_id + 1
        );
        let tab = QString::from(rust.tab.unwrap().to_string());
        assert!(capture(&mut rust, &tab, 1, &QString::from(VIEW), 13).is_err());
        assert!(!second.is_current(&rust));
        let third = begin(&mut rust);
        rust.interaction.hint_request = None; // production stop/option/scope seams
        assert!(!third.is_current(&rust));
        let fourth = begin(&mut rust);
        assert!(fourth.is_current(&rust));
        assert!(!third.is_current(&rust));
        rust.interaction.next_hint_request_id = u64::MAX;
        assert!(capture(&mut rust, &tab, 0, &QString::from(VIEW), 13).is_err());
        assert!(!fourth.is_current(&rust));
        assert_eq!(rust.interaction.next_hint_request_id, u64::MAX);
    }

    #[test]
    fn selected_context_options_and_view_identity_are_all_required() {
        let mut rust = ready();
        let request = begin(&mut rust);
        rust.input_epoch += 1;
        assert!(!request.is_current(&rust));
        rust.input_epoch -= 1;
        rust.hint_target = QString::from("yank");
        assert!(!request.is_current(&rust));
        rust.hint_target = QString::from(request.invocation.options.target.clone());
        rust.hint_family = QString::from("links");
        assert!(!request.is_current(&rust));
        rust.hint_family = QString::from("all");
        rust.hint_proxy_targets_enabled = !rust.hint_proxy_targets_enabled;
        assert!(!request.is_current(&rust));
        rust.hint_proxy_targets_enabled = !rust.hint_proxy_targets_enabled;
        rust.core_mode = Mode::Normal;
        assert!(!request.is_current(&rust));
        rust.core_mode = Mode::Hint;
        rust.active_tab_index = 1;
        assert!(!request.is_current(&rust));
        rust.active_tab_index = 0;
        rust.interaction
            .prompt_view_identities
            .insert(request.tab, "other-view-same-url".into());
        assert!(!request.is_current(&rust));
        rust.interaction
            .prompt_view_identities
            .insert(request.tab, VIEW.into());
        rust.ipc_shutdown_gate = true;
        assert!(!request.is_current(&rust));
        rust.ipc_shutdown_gate = false;
        rust.profile_bootstrap_pending = true;
        assert!(!request.is_current(&rust));
        rust.profile_bootstrap_pending = false;
        rust.view_alive = false;
        assert!(!request.is_current(&rust));
        rust.view_alive = true;
        assert!(request.is_current(&rust));
        let window = rust.window.unwrap();
        rust.state
            .as_mut()
            .unwrap()
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::PopMode { window }))
            .unwrap();
        assert_eq!(
            rust.core_mode,
            Mode::Hint,
            "stale UI projection cannot stand in for actual Core mode"
        );
        assert!(!request.is_current(&rust));
    }

    #[test]
    fn completed_scope_handoff_is_captured_without_requiring_the_retired_session() {
        let mut rust = ready();
        let original = begin(&mut rust);
        let scope = HintScopeBinding {
            purpose: HintScopePurpose::Hint,
            source_family: "regions".into(),
            lease_id: "scope-native-1".into(),
            owner: LocalInteractionOwner {
                window: rust.window.unwrap(),
                session: HintSessionId::from_display("hintsessionid-1").unwrap(),
                target: original.target,
                configuration_revision: 0,
            },
            element_id: 9,
            frame_path: "0".into(),
            kind: HintKind::Region,
            selector_policy_revision: original.selectors.revision,
            parent_scope_lease_id: None,
        };
        rust.interaction.active_hint_scope = Some(scope);
        assert!(rust.interaction.hint_session.is_none());
        let request = begin(&mut rust);
        assert!(request.is_current(&rust));
        let mut captured = HintRequestCapture {
            request: Some(Arc::clone(&request)),
            annotation: None,
            ..Default::default()
        };
        assert!(finalize(
            &rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        let stamp: serde_json::Value =
            serde_json::from_str(&captured.annotation.as_ref().unwrap().stamp_json().unwrap())
                .unwrap();
        assert_eq!(stamp["scope_lease"], "scope-native-1");
        rust.interaction
            .active_hint_scope
            .as_mut()
            .unwrap()
            .element_id = 10;
        assert!(!request.is_current(&rust));
        assert!(!original.is_current(&rust));
    }

    #[test]
    fn publication_and_actual_navigation_reject_pending_original_capture() {
        let mut rust = ready();
        let request = begin(&mut rust);
        let original_revision = request.policy.as_ref().unwrap().revision();
        rust.state
            .as_mut()
            .unwrap()
            .replace_configuration(ferric_browser_config::Config::default())
            .unwrap();
        let mut captured = HintRequestCapture {
            request: Some(Arc::clone(&request)),
            annotation: None,
            ..Default::default()
        };
        assert!(!finalize(
            &rust,
            &mut captured,
            &QString::from(r#"{"structural_owners":[],"focused_receiver":null}"#)
        ));
        assert_eq!(
            request.policy.as_ref().unwrap().revision(),
            original_revision
        );
        let mut rust = ready();
        let request = begin(&mut rust);
        let state = rust.state.as_mut().unwrap();
        state
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
                target: state.capture_target(request.tab).unwrap(),
                url: ValidatedUrl::parse("https://example.test/other").unwrap(),
            }))
            .unwrap();
        assert!(!request.is_current(&rust));
    }

    #[test]
    fn finalization_rejects_unknown_duplicate_and_oversized_native_role_catalogues() {
        let mut rust = ready();
        for owners in [
            r#"["bad-owner"]"#,
            r#"["",""]"#,
            r#"{"owners":[""]}"#,
            r#"{"structural_owners":[]}"#,
            r#"{"structural_owners":[""],"focused_receiver":null}"#,
            r#"{"structural_owners":["frame-11111111-1111-4111-8111-111111111111","frame-11111111-1111-4111-8111-111111111111"],"focused_receiver":null}"#,
            r#"{"structural_owners":[],"focused_receiver":{"owner":"","origin":"null","depth":1,"composed_depth":1}}"#,
            r#"{"structural_owners":[],"focused_receiver":{"owner":"","origin":"https://example.test","depth":0,"composed_depth":0}}"#,
            r#"{"structural_owners":[],"focused_receiver":{"owner":"","origin":"https://example.test","depth":1,"composed_depth":1,"role":"top"}}"#,
        ] {
            let mut captured = HintRequestCapture {
                request: Some(begin(&mut rust)),
                annotation: None,
                ..Default::default()
            };
            assert!(!finalize(&rust, &mut captured, &QString::from(owners)));
            assert!(captured.annotation.is_none());
        }
        let mut captured = HintRequestCapture {
            request: Some(begin(&mut rust)),
            annotation: None,
            ..Default::default()
        };
        assert!(!finalize(
            &rust,
            &mut captured,
            &QString::from(" ".repeat(256 * 1024 + 1))
        ));
        let weak = Arc::downgrade(captured.request.as_ref().unwrap());
        rust.interaction.hint_request = None;
        assert!(weak.upgrade().is_some());
        drop(captured);
        assert!(
            weak.upgrade().is_none(),
            "no registry or self-reference retains a stopped request"
        );
    }

    #[test]
    fn native_role_finalization_requires_separate_composed_depth_before_grants() {
        let mut rust = ready();
        for (native, composed, valid) in [
            (1, Some(1), true),
            (1, Some(2), true),
            (2, Some(8), true),
            (1, None, false),
            (1, Some(0), false),
            (2, Some(1), false),
            (1, Some(9), false),
        ] {
            let mut captured = HintRequestCapture {
                request: Some(begin(&mut rust)),
                annotation: None,
                ..Default::default()
            };
            let mut roles = serde_json::json!({"structural_owners":[],"focused_receiver":{
                "owner":"frame-22222222-2222-4222-8222-222222222222","origin":"https://frame.example.test","depth":native}});
            if let Some(composed) = composed {
                roles["focused_receiver"]["composed_depth"] = serde_json::json!(composed);
            }
            assert_eq!(
                finalize(&rust, &mut captured, &QString::from(roles.to_string())),
                valid
            );
            assert!(
                captured.work.is_none(),
                "Native role binding alone grants no collection work"
            );
            if valid {
                let receiver = captured
                    .annotation
                    .as_ref()
                    .unwrap()
                    .selected_collector()
                    .unwrap();
                assert_eq!(receiver.depth, native);
                assert_eq!(receiver.composed_depth, composed.unwrap());
            } else {
                assert!(captured.annotation.is_none());
                if composed.is_none() {
                    roles["focused_receiver"]["composed_depth"] = serde_json::Value::Null;
                    assert!(!finalize(
                        &rust,
                        &mut captured,
                        &QString::from(roles.to_string())
                    ));
                    assert!(captured.annotation.is_none());
                }
            }
        }
    }

    #[test]
    fn selector_projection_never_relabels_an_original_capture_after_policy_replacement() {
        let mut rust = ready();
        let request = begin(&mut rust);
        let original = request.selectors.serialized.clone();
        let captured = HintRequestCapture {
            request: Some(Arc::clone(&request)),
            annotation: None,
            ..Default::default()
        };
        assert_eq!(selectors(&rust, &captured), original);
        let mut replacement = ferric_browser_config::Config::default();
        replacement.hints.marker_scale = 1.1;
        rust.state
            .as_mut()
            .unwrap()
            .replace_configuration(replacement)
            .unwrap();
        assert!(selectors(&rust, &captured).is_empty());
        assert_eq!(request.selectors.serialized, original);
        let state = rust.state.as_mut().unwrap();
        let window = rust.window.unwrap();
        let snapshot = state.refresh_site_policy_snapshot(window, URL).unwrap();
        rust.interaction.cached_hint_policies = Some(
            CachedHintPolicies::prepare(
                snapshot,
                SitePolicyMatchContext::for_url(&ValidatedUrl::parse(URL).unwrap()),
                QString::from(URL),
                rust.interaction.hint_configuration_revision,
            )
            .unwrap(),
        );
        assert!(
            selectors(&rust, &captured).is_empty(),
            "a newly prepared cache cannot revive the old capture"
        );
        let newer = begin(&mut rust);
        let fresh = HintRequestCapture {
            request: Some(Arc::clone(&newer)),
            annotation: None,
            ..Default::default()
        };
        assert!(!selectors(&rust, &fresh).is_empty());
        assert_ne!(newer.generations.request_id, request.generations.request_id);
        assert!(selectors(&rust, &captured).is_empty());
    }
}
