# Progressive hints implementation handoff

Date: 17 November 2026. Source baseline: `277d72c74bb40d747ae8150f529b4f0eea2e7779`, branch `feature/site-interaction-policy`.

Status: proposed implementation approach following the owner's agreement on progressive presentation. This document specifies the work for the next implementation agent; it does not describe an implemented feature. Check the actual checkout before starting and record any intervening changes.

Ferric should show useful, stable hint labels earlier, allowing the user to read and type while remaining validation completes. Every effect must retain Ferric's current action validation. The product change is that a displayed label may initially be provisional and may subsequently be withdrawn. A displayed code must never move to another target or acquire a different action during that interaction.

The expected benefit is overlap between validation and the user's reading/typing. Do not assume a minimum human reaction time or that corrections are invisible. Measure early input, corrections and completed actions as well as first presentation.

## 1 Scope and decisions

Keep the ordinary `f` workflow, current policy configuration, supported target families, native page-key routing and existing settings. No additional user-facing hint mode is required.

Implement one collector pipeline with progressive results, one Rust label allocator and one policy engine. Reuse the existing native request/document authorities, work ledger, selection machinery and overlay. Preserve strict collection as an internal fallback through the same implementation, not a separately maintained hint engine.

The following decisions govern this proposal:

1. Early rows are presentation and selection proposals. They grant no action permission.
2. Rust owns their labels, identities, state transitions and pending selection. JavaScript and QML cannot promote a row to actionable by setting a flag.
3. A displayed code has one target and one action meaning for the entire session. Validation can confirm or retire it; it cannot retarget it.
4. Special codes are reserved from captured configuration before matching. A policy code appears only after the evidence determining its exact binding is complete. Later native sealing and action freshness may still be pending.
5. An unresolved policy group may delay its own labels and any ordinary candidates whose ownership it could change. Other independent rows may appear earlier.
6. Explicit early selection may wait within existing deadlines for validation of that exact binding. Cancellation is immediate. No native website key is queued waiting for validation.
7. Expensive checks continue automatically while hints are open. Selecting a row prioritizes its required work where dependencies allow; it never skips dependencies.
8. Keep each optimization only with measured benefit and preserved behavior. Do not turn an unsuccessful experiment into a general framework.

The owner has accepted the direction of earlier presentation followed by correction. This authorizes designing the provisional display contract; it does not authorize weaker activation proofs, guessed policy ranks, reduced coverage, increased work limits or silent relabeling. Trial timing parameters below are experiment gates, not new frozen product deadlines.

## 2 Evidence and starting point

The previous Ferric profile found ordinary discovery, including policy inventory, to be the largest stage. Policy extraction and native verification also cost meaningful time. Publication and drawing were smaller. The historical medians were approximately 148–346 ms ordinary wall time, 56–90 ms policy extraction, 29–91 ms verification and 12–21 ms publication. These are independent/nested stage medians, not an additive decomposition of a current build. See [the profiling report](../testing/hint-profile-2026-11-16.md).

The subsequent optimization retained sparse policy inventory for non-element nodes. Classification hoisting and style-before-geometry changes were rejected by measurements. A proposed predicate cache had no repeated element/compound pairs to reuse. Native verification probes protect distinct asynchronous boundaries; no safe local removal was established. See [the optimization report](../testing/hint-optimization-2026-11-16.md). That report also records qualification gaps and a label-availability exception; do not describe it as an entirely green baseline or reopen the deferred 5,000-target performance target.

Installed qutebrowser 3.7.0 uses native CSS queries for candidate matching, followed by simpler visibility checks. It also scans for open shadow roots. Its helpers are installed at document creation and called through small requests. These techniques are useful references, not Ferric's behavioral specification. [Collector source](https://github.com/qutebrowser/qutebrowser/blob/v3.7.0/qutebrowser/javascript/webelem.js), [script installation](https://github.com/qutebrowser/qutebrowser/blob/v3.7.0/qutebrowser/browser/webengine/webenginetab.py).

The shared Qt 6.11.2 renderer experiment found 12.4 ms for qutebrowser and 27.5 ms for Ferric on a simple links fixture with the same nine target IDs. Default-family fixtures differed in coverage. In a separate eligibility fixture qutebrowser returned disabled, inert, covered and ancestor-transparent controls that Ferric excluded. These are collector-body measurements, excluding compilation, policies, frontend and presentation; they do not establish browser-wide speedup factors.

Research artifacts are currently local and ignored under `target/qute-hint-research-20261117/`: `provenance.json`, `sources.json`, `probe-summary.json`, `tst_compare.qml` and source copies. If unavailable in another checkout, reproduce the small experiment rather than treating untracked paths as delivered evidence. Relevant installed qutebrowser files matched upstream v3.7.0 byte for byte; the inspected main collector also matched at `a2a9c9dd88e659107c70f2fa9a9968ea4a9e643c`.

## 3 Preserve the existing contracts

Read [the original site policy plan](../../../Ferric_Site_Policy_Advanced_Hints_Implementation_Plan.md), especially Sections 3.6–3.12, and [current user semantics](../user/site-policies.md). The original plan is a workspace-level file and may need to be obtained from this workspace when handing off elsewhere. Read the actual code where historical documents differ from current qualification.

Preserve these properties:

- Supported shadow roots, slots, nested frames and scoped roots; no broadening of foreign-frame or pointer capabilities.
- Action-specific eligibility, correct clipping/projection, disabled/inert handling, and supported editor/label proxies.
- Distinct physical targets remain distinct even when URLs match. Existing preferred-link and proxy rules still apply.
- Anchors require complete uniqueness evidence. Collections retain logical item/primary identity, eligible-item ordering, anchor precedence, ambiguity behavior and reserved holes.
- Collection numbers do not compact after withdrawal. A retired, consumed or changed-meaning special slot cannot bind a replacement.
- Native tab/document/frame ownership and configuration identity remain authoritative. Matching a URL, renderer element ID or mutation revision is insufficient.
- Final activation revalidates the current target and all applicable policy dependencies. This includes membership/order, not just the selected element's rectangle.
- Query text, IME, browser UI, modal input, pointer confirmation, action palettes and protected recovery retain their ownership.
- A successfully delivered Hint page-key gesture exits to Normal after its final release. Blocked delivery preserves the label prefix. Normal mode does not inherit a Hint mapping, and tabs do not share pending state.
- The current candidate, inspection, depth, annotation, retention, native-work and deadline limits remain in force.

This work needs narrow, explicit amendments to presentation and initial-session construction. Existing special slots can be Live, Unavailable, Retired or Consumed; a new initial Pending state must not be represented as Unavailable and later resurrected. Pending may settle once as part of the original initial collection. A slot that has reached a terminal unavailable/retired state cannot become live through passive refresh.

Update affected current documentation and tests to state that distinction. Do not rewrite historical qualification evidence to make it appear that provisional publication was already tested.

Preserve the existing distinction between a temporarily absent ordinary target and a retired/consumed binding. Where ordinary refresh already permits the same identity and meaning to return with its reserved code, keep that behavior. It does not permit a rejected initial proposal, changed-meaning target or retired special slot to be silently rebound.

## 4 End to end behavior

### 4.1 Request capture

On `f`, capture the existing native request context: tab/view, document ownership, collection/input epochs, configuration and selector revisions, invocation family, scope and viewport dependencies. Compile/check the codebook and page-key exclusions from that captured configuration before publishing any label.

Initialize one bounded progressive session under the existing interaction owner. Use existing identities; add only the minimal purpose/sequence metadata required to distinguish presentation updates from validated results. Retiring the session invalidates all of its pending callbacks and selection intents.

### 4.2 Initial discovery and presentation

Perform bounded discovery and initial facts gathering. Prefer native CSS matching for supported built-in selectors if the equivalence experiment succeeds. Retain compatible handling for custom selectors, `:scope`, scope-root inclusion, malformed selectors, traversal order and root boundaries. Do not issue an unrestricted full-document query and claim it preserves a bounded physical scan merely because Chromium performs it internally.

Keep inexpensive checks that prevent obviously misleading rows, finite/bounded geometry validation and the identity/action facts required for binding. Which style/geometry operations are inexpensive must be measured; do not repeat the rejected style-first experiment by assumption.

Discovery must retain advanced target coverage. Continue through offscreen ancestors; positioned, overflowing and sticky descendants can be visible. Keep root discovery and the policy inventory work still required by the contracts.

Rust allocates labels only for candidates with settled ownership and sufficiently known identity/meaning. QML displays these rows using the existing overlay and treats their validation status as internal state. Provisional badges should not add routine flashing, animations or a new explanation the user must learn. If a selected row waits noticeably, use a concise existing-style status such as `Checking target…`.

Reserve capacity for delayed discoveries before the first allocation. Do not choose a width that necessarily excludes later valid targets, or choose the maximum possible width on every page to avoid planning. The initial candidate inventory, codebook and allocation policy must establish bounded capacity; if this cannot be done without worse code lengths or lost coverage, delay publication for that case.

### 4.3 Continued validation

Validate the outstanding candidates and policy groups through the current eligibility and native proof machinery. A successful result promotes the matching row; a negative result withdraws it and reserves its code. A result from an old request, batch, document or configuration has no effect.

Publish small batches, coalesced with normal UI updates, rather than rebuilding the whole overlay after every element. Surviving rows keep their code and identity. Preserve placement when valid, but never leave a misleading badge over a different target merely to avoid movement. Do not animate a correction between targets.

Advanced targets that cannot be proposed reliably may arrive after validation. They must use unused, preplanned code capacity without stealing an early binding. Policy-dependent candidates follow Section 5.

On unchanged fixtures, completed collection must contain the same eligible targets and policy assignments as the strict path, subject to actions already consumed. Ordinary letters may differ because provisional labels remain reserved; measure that difference, code lengths and capacity pressure explicitly. Identical target counts alone do not establish equivalence.

### 4.4 Early input and selection

Before any rows exist, preserve the existing bounded early-intent reducer, including early `/` query ownership. Once provisional rows exist, process editing/filtering against their actual stable codebook. Do not replay the same input later against a freshly allocated label set.

A full-code or explicit-confirmation selection captures the exact session, binding revision, candidate identity, action meaning and intent. Keep at most one pending selection. Give its prerequisite validation work priority at the next scheduling boundary. If the current implementation only supports validating its whole world/policy group, wait for that unit; do not pretend selected-only validation is equivalent.

Execute exactly once only after that binding is validated and the existing fresh action checks succeed. Use the existing action execution route. A provisional URL must not bypass it through direct navigation, copy, download or another apparently low-impact action.

Cancellation rules are concrete:

- Escape, hint exit, tab/view/window change, native focus loss, navigation/document replacement, configuration replacement and shutdown cancel pending selection and retire the relevant work.
- A subsequent editing, query, cycling or new selection intent cancels the older pending selection before being processed. A repeated confirmation cannot create duplicate effects; prevent held-key repeats from rearming it.
- A rejected or expired selection clears that selection, keeps useful hints where appropriate and reports that the target is unavailable. It never selects the next row or an item that inherited the code.
- Pending selection uses an absolute bounded expiry no later than the applicable existing request/action deadlines. Chunks and retries do not restart that timer. After expiry require a fresh explicit selection.
- Native page-key gestures are never held in this queue. They use the existing immediate receiver checks and gesture lifecycle. Successful delivery cancels label selection and progressive work as hints exit; blocked delivery does not reinterpret the source key.

Full-code activation remains an explicit input effect. Background completion or removing other rows must never manufacture unique-match/singleton activation. For existing ordinary auto-follow options that require complete discovery, hold the decision until the initial collection is complete and preserve explicit-input precedence. Enhanced sessions keep their existing compatible auto-follow rules.

Pointer confirmation still requires its actual native gesture. Do not synthesize a press/release later from an early Enter. Action palettes and Target Search must not gain speculative action authority through shared row types; use the validated path until explicitly covered by tests.

### 4.5 Completion and failure

When all required initial units settle, mark initial collection complete. Pending special reservations settle once; passive refresh then follows the existing retention/retirement rules. A late initial-completion callback cannot reopen a closed or newer session.

Before anything is shown, an unsupported progressive case may use strict collection. After labels are shown, fallback must reconcile validated facts into the same session and allocator. It cannot replace the overlay with a newly numbered strict session or replay already-consumed input.

On timeout, retain only rows with independently valid evidence if existing contracts permit it. Otherwise withdraw the affected rows, cancel selection and report once. Distinguish Pending, unavailable and no matching targets. Never report a complete snapshot when a required scope or policy search is incomplete.

## 5 Policy-aware publication

The policy engine is not a decorative step that can always run ahead of visibility. Under the current contract, collection membership and ordering use the eligible, clipped/hittable primary fragment. Therefore some of Ferric's expensive checks directly determine which item receives `1`, `2`, and so on.

Use the existing rule plan to identify the smallest complete unit whose ownership can be resolved. A unit may contain overlapping collections, anchors that can claim their targets and all worlds/roots required by those decisions. Keep this grouping local to the policy planner; do not introduce a general dependency-graph framework.

For each unit, distinguish these facts:

| Facts | Required before its code appears | Required before any action |
| --- | --- | --- |
| Native request ownership and captured configuration/codebook | Yes | Recheck current ownership |
| Exact target, related proxy/item identity and action meaning | Yes | Fresh equivalence check |
| Root/selector completeness, uniqueness, anchor precedence and overlapping ownership | Yes for affected special and fallback rows | Applicable live reproof |
| Eligible membership, canonical fragment and order | Yes when these determine collection binding | Applicable live reproof |
| Structural payload validity, finite geometry and bounded fields | Yes | Retain decoder/native validation |
| Remaining clipping/occlusion/geometry checks unrelated to an already settled binding | May be deferred for ordinary/provisional presentation | Yes |
| Final native catalogue sealing and current action permit | May still be pending for provisional presentation | Yes; never replaced by an early result |

An early renderer claim is still a proposal. Rust may use bounded observations for presentation planning only after structural checking and matching them to the captured browser-owned request. It must not mint `Checked` proof/permit types from those observations. If later verification disproves the proposal, withdraw the binding; do not repair it by choosing a different target.

Special codes are reserved even for groups that have not appeared. An ordinary target that could later be claimed by an unresolved special assignment must wait too. Assigning it `af` and later changing it to `1`, or giving it two live labels, would violate this design. If independence cannot be established conservatively, hold the affected fallback rows until ownership resolves.

A group's initial binding is committed once. The evidence must form a coherent logical snapshot within the captured request and satisfy cross-group ownership dependencies. Do not concatenate chunks observed at incompatible times into a claim of complete membership/order. Relevant changes invalidate or trigger only the existing bounded recovery; an unrelated dirty notification alone does not prove that membership changed.

For difficult groups the first frame may contain ordinary controls while numbered items arrive later. Measure time to policy labels separately. The implementation is not successful merely because one unrelated early badge appears while the controls the user wants still arrive late.

Corrections follow the same dependency boundaries. If rejecting one proposal disproves a collection's membership/order or an anchor's uniqueness, invalidate the affected dependent bindings, not just that one visible row. Invalidate any pending selection that relies on them. Do not leave later numbers actionable under a proof that no longer holds.

For example, if an ordinary `af` proposal is later found to be covered, remove it and leave unrelated `ag` unchanged. If a collection provisionally displayed `1` on item A and `2` on B after resolving its binding facts, but final verification then disproves the collection snapshot, retire the affected collection bindings. Do not move `1` to B or allow a queued `2` action to proceed on stale membership. A design that routinely reaches this outcome on unchanged pages fails the quality gate; move the decisive check before publication for that group.

## 6 Internal state and authority

Use a small explicit state model within existing types, with names adapted to repository conventions:

| Row state | Display | Selection | Effects |
| --- | --- | --- | --- |
| Provisional | Visible once identity/ownership requirements are met | May create one bounded pending intent | Forbidden |
| Validated | Visible | Existing selection path | Only after current activation checks |
| Withdrawn | Hidden; binding remains reserved | Unavailable | Forbidden |
| Consumed | Existing rapid-mode behavior | Cannot repeat the consumed binding | No replay |

Keep pending special declarations separate from live rows. `PendingInitial` is not `Unavailable`, and it is never silently revived after initial completion. Session status needs only collecting/presenting/complete/closed distinctions necessary to enforce these transitions; avoid a parallel interaction manager.

Conceptually, updates are `initial proposals`, `validation results` and `initial completion`, bound to the existing request plus a monotonic batch sequence. Define duplicate, reordered and missing-batch behavior explicitly. Make application idempotent or reject duplicates without changing bindings. Final validated facts must match the recorded identity and meaning before promotion.

Version any changed internal wire contract explicitly. Do not reuse a historical proof version for data with weaker meaning, or change the public configuration schema for this scheduling feature. Continuations must make bounded progress; an empty or repeated response cannot schedule an unlimited stream of callbacks even when it reports zero DOM work.

QML may render readiness and request selection. The action boundary must enforce readiness in Rust/native code, including command/IPC routes and indirect effects. Unit tests must demonstrate that provisional data cannot reach every existing effect route even if a QML caller attempts it directly.

Text and metadata remain bounded, sanitized, plain text and subject to existing privacy restrictions. If a name participates in action meaning, it cannot be filled in later as though it were merely cosmetic. No new logging of page text, cookies, editable contents or private URLs is required for measurements.

Use the existing definition of action meaning. Do not claim it detects arbitrary replacement of page JavaScript event handlers; this proposal preserves the current guarantees rather than inventing a stronger DOM identity model.

## 7 Scheduling and retained data

Renderer DOM work is not made parallel by calling it background work. A long synchronous task can delay later selection checks. Return control between bounded validation batches so native input handling and priority changes can take effect. A chain of resolved promises is not an adequate substitute for yielding to another task. [HTML event-loop model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loops).

Start with a simple queue driven by the existing provider: one in-flight renderer batch for a participating world, cancellation checked before/after dispatch, and selected dependencies first at the next boundary. Do not build a general scheduler or promise preemption of a DOM call already executing.

Choose batch size from measured durations in Phase 0/3, with both an operation cap and an elapsed-time yield check. A costly individual DOM call can exceed a slice target; that target is not a hard real-time promise. Avoid tiny batches whose IPC, callback and revalidation costs exceed their responsiveness benefit. Keep cross-world ordering and optional-world failure isolation intact.

Caches of style, rectangles, hit tests and ancestor facts are valid only in the synchronous observation interval for which they were established. Across a yield, reuse only facts with adequate dependency evidence; otherwise reread them. Neither a matching MutationObserver revision nor an unchanged element proves unchanged layout, animation or policy rank.

Retain the existing weak inventory/document lifecycle where work crosses callbacks. Bound queues, provisional rows and retired bindings. Release scheduled work and references on cancellation, pagehide and timeout, even when a late callback is still possible. No scanning or full DOM index should continue while hints are closed.

If measurements justify it, install versioned immutable helper functions in ApplicationWorld and invoke them with request data. Keep per-request caches fresh and do not close over a previous request or document. This is an optional optimization, not a prerequisite for the prototype; verify first use during loading, second tabs, reloads and subframe replacement before retaining it.

## 8 Work accounting and deadlines

Extend the existing native schedule and ledger only where a new dispatch purpose requires it. Every batch, refresh and recovery belongs to its original request. Native code grants work before dispatch, and only established outcomes return unused grants. Renderer-reported completion or counters cannot authorize additional work.

Keep the 50,000 physical inspection, 5,000 ordinary candidate, depth 8, 100,000 semantic predicate, 2 MiB annotation and applicable facet/retention limits with their current scopes. Keep the distinct native graph/document verification pools distinct; do not describe the semantic predicate cap as the complete native-work bound.

Define accounting for provisional descriptions explicitly. They must not create a fresh candidate/inspection allowance per batch, and false-positive proposals must not silently consume final admission capacity that the strict path would have had. Total retained presentation rows and queues must remain bounded. Account for all auxiliary traversal and ownership work; tests must cover cap boundaries with many rejected proposals followed by valid targets.

Cancellation prevents further dispatch and publication but does not prove that an already running renderer script stopped. Preserve the existing settlement/recovery behavior. Reusing the strict path after provisional failure must not reset budgets or grant unlimited retries.

Inventory every existing timeout in Phase 0. Preserve its authority and absolute expiry while splitting work. Do not turn each small chunk into a fresh one-second allowance. This work does not reopen the deferred extreme-load performance target.

## 9 Source integration map

Paths are relative to the repository root. Confirm current entry points before editing.

| Area | Primary files | Intended change |
| --- | --- | --- |
| Candidate discovery and eligibility | `crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js` | Separate reusable discovery/initial facts from validation while sharing existing checks; native-selector experiment |
| Retained roots and witnesses | `qml/scripts/InteractionRootRuntime.js`, `InteractionWitnessRuntime.js` under the same crate | Preserve bounded completeness and freshness across continuation boundaries |
| Label/session model | `crates/ferric-browser-core/src/hints.rs`, `hints/enhanced.rs`, `hint_codebook.rs` | Provisional readiness, initial pending slots, immutable bindings and exact pending intent |
| Payload validation | `crates/ferric-browser-engine-qt/src/hint_payload.rs`, `hint_payload/decoding.rs` | Bounded proposal/update decoding that cannot masquerade as validated proof |
| Policy evidence | `src/interaction_proof.rs` and its modules under the same crate | Reuse policy semantics; distinguish presentation planning from checked authority |
| Request accounting | `src/hint_collection_work.rs`, `hint_collection_work/ledger.rs`, `src/hint_request_adapter.cpp` | Admit progressive operations under existing request ownership and aggregate budgets |
| Native proof and geometry | Existing `FerricFrameBroker` implementation and request-adapter callbacks | Retain seals and action freshness; instrument before considering any consolidation |
| Collection orchestration | `qml/components/FerricHintFrameProvider.qml` | Bounded continuations, stale-update rejection, priority and same-session fallback |
| Publication and effects | `src/browser_ui_navigation.rs` and existing selection/action routes | One allocator across proposal and completion; readiness enforced before every effect |
| Input and overlay | `qml/components/FerricHintWorkspace.qml`, `FerricHintOverlay.qml`, `qml/scripts/HintPresentation.js` | Existing input reducer, pending exact selection, batched presentation and stable placement |

The existing provider has Ordinary/Policy phases, a bounded stage count and a final sealed merge. Progressive publication cannot be achieved just by invoking its final callback earlier. Distinguish semantic stages from continuation batches without removing the bounded stage guard or marking an unfinished policy snapshot sealed.

## 10 Ordered implementation phases

Work sequentially. Finish the exit checks and record the keep/revert decision for each phase before beginning the next. A supporting phase can be retained as part of the measured feature without claiming it independently speeds up hints. Keep changes reviewable; use local commits at completed checkpoints when implementation is authorized. No push, PR or installation is part of this handoff alone.

### Phase 0 Baseline and feasibility

Read instructions, the source map and current qualification limits. Record source/binary/SDK identity and capture a current strict-path baseline. Add narrowly scoped diagnostic timestamps and candidate/binding fingerprints using the existing harness.

Produce a dependency table for each deferred check: what fact it establishes, which policy bindings depend on it, whether it can move after presentation, and how it is refreshed before activation. Identify the earliest truthful presentation point for ordinary rows, anchors and ordered collections.

Verify capacity planning, aggregate accounting and PendingInitial semantics before changing the UI. Demonstrate a useful early set on both a YouTube-like collection and a Reddit-like feed without hiding the difficult targets from the measurement. Stop or narrow the design if it requires relabeling, lost coverage or a second policy engine.

Exit: baseline receipt, dependency/accounting table, proposed batch boundaries and concrete test cases. Keep this phase small; it is not a new profiling infrastructure project.

### Phase 1 State and input contract

Implement proposal readiness and exact pending intent in the existing core/session owner, initially behind an internal test switch. Reserve special codes and distinguish initial pending from unavailable/retired slots. Keep strict production behavior active.

Test label immutability, codebook capacity, all readiness transitions, stale/reordered updates, cancellation, early text mode, no completion-driven auto-follow and attempts to execute provisional rows through every action entry point.

Measure input handling and retained-state overhead against strict mode. Exit: core/state and focused native cancellation tests pass, with no second input queue or allocator and bounded retained state.

### Phase 2 Shared discovery

Extract only the seams required by the progressive pipeline. Prototype built-in native CSS selection independently against the current implementation, retaining physical/root work where still necessary. Keep full validation before publication in this phase so collector changes can be judged separately.

Run differential coverage, order, geometry, scope, selector and work-limit tests. Measure both policy-enabled and policy-free collection and full hint latency. Revert native selection if it adds another scan without benefit; progression must not depend on keeping an unsuccessful optimization. Evaluate resident helpers separately only if timing indicates worthwhile overhead.

Exit: one shared discovery path, unchanged full-validation behavior, and a documented keep/revert result for each performance experiment.

### Phase 3 Progressive publication and validation

Connect proposal publication, bounded continuations and native validation to the session model. Reconcile results without creating a fresh hint session. Keep proposal construction and continuation on the same candidate inventory wherever freshness permits.

Introduce policy units in dependency order: ordinary independent rows, independent anchors, then collections/overlaps. Do not enable a case until its ownership and capacity tests pass. Keep supported complex cases on strict publication when necessary, and measure how often that fallback occurs.

Measure first useful labels, policy-label arrival, complete validation, renderer busy intervals and corrections after each integration step. Remove duplicate discovery rather than disguising it as background work.

Exit: unchanged-DOM final coverage and special assignments match strict mode, codes never change meaning, all new work remains accounted, and useful early publication is demonstrated in representative policy-enabled cases.

### Phase 4 Selection and lifecycle integration

Integrate early full-code/Enter selection, priority at batch boundaries, exactly-once execution and absolute expiry. Preserve pointer confirmation and immediate native page-key behavior. Exercise primary/secondary windows and tabs, YouTube-to-Reddit navigation, focus/modal changes and session replacement.

Use controlled renderer pauses and callback ordering to test races deterministically. Do not rely on sleeping until a bug is unlikely. Inject edits, Escape, page keys and new selections while validation is deliberately pending.

Exit: valid early selection completes on the intended target, invalid selection performs no effect, cancellation prevents delayed actions, and normal page-key behavior remains unchanged. Measure completed actions using the same input schedule for baseline and candidate.

### Phase 5 Quality and performance decision

Run the full relevant regression matrix and paired measurements below. Review the final diff, remove unused experiments and ensure production resources match the tested binary. Record retained changes, rejected experiments, actual measurements and remaining limitations.

Enable progressive presentation by default only after the gates pass. Do not leave a permanent user-facing switch as a substitute for resolving regressions. A temporary internal comparison switch can remain only where the test harness needs it.

Deliver the tested implementation, reproducible evidence, updated current contracts and a concise testing guide. Installation and user testing follow the user's instructions at that point; preserve a rollback build when installing.

## 11 Validation matrix

Extend maintained tests rather than creating a parallel test framework. Relevant starting points include `tests/qml/tst_hint_quality.qml`, `tst_hint_geometry.qml`, `tst_hint_controls.qml`, `tst_hint_actions.qml`, the `tests/site-policy-http` deferred/root/witness suites, core enhanced-interaction tests, and native `application_hint_controls`, `application_hint_media_tabs`, request-ownership and page-key suites.

| Case | Required assertion |
| --- | --- |
| Ordinary unchanged page | Early labels remain stable; final eligible identities, kinds, meanings and geometry match strict behavior |
| Disabled/inert/covered/transparent controls | Initial obvious rejects stay excluded; any deferred negative withdraws safely; no effect is possible |
| Offscreen parent with visible overflow/sticky child | Child remains discoverable; no subtree pruning regression |
| Shadow roots, slots and supported frames | Correct identity, scope, clipping and depth; replacement invalidates old batches |
| Proxies, scroll targets and preferred links | Existing coverage and action semantics retained; no duplicate labels for one physical action |
| Ordered collection and tall primary | Correct eligible ranks, canonical fragments, row ordering and reserved holes |
| Ambiguous anchors/primaries and overlapping collections | No guessed owner, incomplete proof or temporary fallback-to-special relabeling |
| Pending special slot | Settles once during initial collection; terminal unavailable/retired slots never become live via refresh |
| Collection mutation between batches | No mixture of incompatible membership/order snapshots; affected bindings fail safely |
| CSS-only layout/visibility change | Fresh checks catch relevant changes without relying solely on mutation notifications |
| Invalidated selected row | No substitute action, no code reuse, concise feedback, useful remaining hints preserved |
| Early input and repeated Enter | Exact displayed binding, exactly one action, correct query/slash/space handling and held-key behavior |
| Escape/edit/new selection/focus/modal/navigation | Old pending intent and late results cannot act or replace newer state |
| YouTube page keys in two tabs | `f`, then `p` or fullscreen gesture follows current one-action behavior; no normal-mode leakage |
| Budget and capacity boundaries | No per-batch budget reset, false-positive starvation, uncontrolled label widening or unbounded retention |
| Reordered/duplicate/late updates | Idempotent application or safe rejection; no promotion from stale or forged readiness |
| Strict fallback after publication | Same bindings and typed input preserved; no replacement session/automatic replay |
| Timeout and renderer settlement | Absolute expiry, bounded recovery, no new work after cancellation, no late action |
| Pointer, palette and Target Search consumers | Existing authority/gesture boundaries remain; unsupported progressive cases use strict behavior |

Use separate controlled fixtures for initial false positives and page mutations. Otherwise a withdrawal caused by a real page change can be confused with a poor initial heuristic. Add tests at the actual production caps for arithmetic, accounting and storage without reintroducing the deferred 5,000-target latency qualification.

## 12 Measurement and retention gates

Measure these events from one monotonic timeline:

- `T0`: the production input path accepts `f`.
- `Tfirst`: the first swapped Qt frame containing usable labels from a declared workload target group.
- `Tpolicy`: the first frame containing that workload's intended special/numbered labels.
- `Tcomplete`: required initial validation settles, including unavailable outcomes.
- `Tselect`: an explicit selection intent is accepted.
- `Taction`: the expected action completes or produces an observable acknowledgement.

Record time to a predetermined representative target as well as the first label. If an action's actual effect cannot be observed, label the endpoint dispatch acknowledgement rather than action completion. These measurements do not include physical keyboard transport or monitor presentation unless explicitly instrumented.

Collect stage timings, maximum synchronous batch duration, yield/round-trip counts, target fingerprints, withdrawals, movement of surviving badges, code-length distribution, capacity failures, early-selection wait/failure counts, retries, renderer work and retained memory. Keep diagnostic counters separate from production timing runs.

Use controlled fixtures plus YouTube homepage/search/watch and Reddit feed cases, top and scrolled, including a second tab and cross-site navigation. Pin Qt 6.11.2, viewport, zoom, power state and test configuration. Test both the primary full-width viewport and the narrower layout that previously changed results. Primary acceptance uses policies enabled; policy-free runs isolate costs only.

Start with two warmup pairs and ten alternating baseline/candidate pairs per primary case; repeat successful cases in a second batch. Compare stable identities/geometry or controlled fixtures, not unrelated live-page loads. Keep failed entries in the results and distinguish setup failures. Use a larger sample for tail estimates; ten observations do not establish a reliable p95/p99. Reuse valid measurements instead of repeatedly running expensive suites without new evidence.

For early-selection tests use identical schedules relative to `T0`, including selection before any labels, during provisional display and after completion. Also measure a schedule relative to first visible labels to assess the new experience. Report these separately; shifting input timing can manufacture an apparent action-latency gain.

When timestamps cross native, QML and renderer processes, establish clock offsets or use native receipt/frame timestamps for end-to-end intervals. Do not directly subtract a renderer `performance.now()` value from an unrelated native monotonic clock. Keep nested stage durations and end-to-end measurements separately named.

Proposed experiment gates:

| Gate | Decision rule |
| --- | --- |
| Authority and identity | Zero wrong-target effects, label retargets, duplicate effects or late effects after cancellation in deterministic tests |
| Feature preservation | Same final eligible targets and special assignments on unchanged fixtures; no new required restart, namespace failure or missing advanced target |
| Ordinary local optimization | Retain a performance-only change after at least 5% and 10 ms repeatable end-to-end benefit on a primary case, with no material regression |
| Progressive architecture | Seek at least 20% and 30 ms improvement in time to useful labels in representative policy-enabled YouTube and Reddit cases, reproduced in a second batch; report `Tpolicy` independently |
| Completed interactions | No reproducible median regression exceeding both 5% and 10 ms for the same action/input schedule; investigate slower runs and never discard failures to pass this gate |
| Work and memory | Stay within existing limits; investigate a repeatable total renderer-work increase over 10%. A one-off allowance change is not a performance fix |
| Corrections and readability | Report rates and examples of withdrawals, movement and longer codes. Repeated distracting corrections or materially worse code lengths fail the experience review even if timing gates pass |

The architectural thresholds deliberately require more benefit than a small local optimization. They are proposed development stop conditions, not promises of a particular absolute response time. If the prototype misses them, retain independently useful discovery improvements and remove the unsuccessful progressive machinery. Do not chase an arbitrary universal sub-100-ms or stress-load target.

The human-facing gate is actual keyboard testing: the owner should be able to start reading/typing earlier without noticing unstable targets, delayed surprises or missing controls. Do not declare this passed solely from timing numbers or an assumption about human processing speed.

## 13 Execution discipline and deliverables

Use the qualified Qt 6.11.2 SDK and matching private headers. Preserve the QML interpreter safety workaround and the KDE-free build/install/runtime policy. Follow `AGENTS.md`; changing the SDK or introducing KDE is outside this plan.

Use at most two build jobs and serialize builds with performance runs. Reuse the existing runner for small experiments. Do not repeatedly swap production source files to select baselines: previous timestamp changes caused avoidable native rebuilds. Use isolated build/resource inputs, record hashes, and never bypass archive freshness or falsify timestamps. Verify that final measurements exercise production resources and the intended executable.

Do not replace historical test assertions simply because a new path fails them. Reproduce relevant failures on the baseline and distinguish documented product changes from regressions. Keep the original one-second frame boundary and current bounded recovery; old extreme-load failures remain separately reported.

No persistent DOM index, speculative scanning while hints are closed, worker-DOM scheme, Qt fork, new framework, site-specific workaround or new policy configuration format is required. Resident helper installation and batching refinements earn their place through measurements. Do not weaken checks merely to improve first-frame numbers.

At each completed phase record the commit/source identity, scope, tests, measurements and keep/revert decision in one concise report under `docs/testing/`. Store raw logs and traces under ignored `target/` with reproducible commands and hashes. Commit maintainable fixtures/tests and the essential summarized evidence needed by another checkout; do not rely solely on untracked artifacts.

The implementation handoff is complete when it includes:

- One integrated progressive path with stable labels, exact pending selection and enforced action readiness.
- Preserved strict behavior for cases whose early publication cannot yet meet the contracts, with measured fallback frequency.
- Passing targeted correctness/lifecycle tests and an honest account of broader qualification gaps.
- Reproduced first-label and completed-action measurements, correction/readability results and resource accounting.
- Updated current presentation/session contracts, no rewritten historical evidence, and no abandoned experimental code.
- A short owner testing guide covering ordinary browsing, numbered policies, early typing, cancellation, two YouTube tabs and navigation to Reddit.

Suggested instruction to the implementing agent:

> Read this plan and the applicable repository instructions. Implement the phases in order, beginning with the current strict baseline and the policy-dependency feasibility check. Preserve label identity, policy semantics, native action authority and aggregate limits. Reuse the existing collector, allocator and input owner. Measure each phase, retain only useful changes, and report a failed gate directly rather than adding complexity or weakening the contract. Do not install, publish or push merely because the implementation is complete; follow the user's current delivery instructions.
