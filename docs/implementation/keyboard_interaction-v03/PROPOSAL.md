# Ferric — Reliable Keyboard Interaction for Modern Web Applications
## Unified proposal and implementation specification • v0.3

**Prepared:** 11 October 2026  
**Status:** Proposed implementation contract; no new Ferric feature is claimed implemented by this pack.  
**Repository baseline:** `thomasaiwilcox/ferric-browser` at `4337056f632de0c710943fa6df76c67cf14b8b47`. The main-branch ref was checked again during preparation and matched.  
**Comparison baseline supplied in Source B:** qutebrowser `2bd7591`; comparison claims are source-inspection recommendations, not reproduced compatibility results.  
**Sources merged:** Source A, the attached 6,092-word v0.2 proposal; Source B, the targeted recommendations F-01–F-08 pasted in this conversation. See `SOURCE_MAP.md` for complete coverage and explicit reconciliations.  
**Execution:** One goal prompt, `GOAL_PROMPT.md`; one dependency plan, `plan.json`; individually bounded cards in `slices/`.  
**Other work:** The versioned-browsing branch and its implementation pack remain out of scope.

> **Product principle:** Correctly activate and focus the controls Ferric already discovers before expanding what it can discover. Then make more of the page addressable through shared semantics, scoped spatial interaction and explicit keyboard ownership.

## Reading and authority

This is the canonical design. A coding agent reads its assigned slice, the contract sections that slice names, and the existing source files—not an unrelated feature backlog. Source archives are historical inputs, not competing instructions.

Precedence inside this pack is: this document's contracts and explicit decisions; the test matrix; the slice's bounded work order; the machine-readable scheduling plan. These must agree. If they do not, record a specification conflict and block that slice rather than choose the weaker rule. Repository security/architecture constraints remain binding unless this document explicitly identifies a narrow amendment and the slice records that amendment in an ADR. Higher-level repository instructions always apply.

`MUST` is required. `SHOULD` permits a documented, test-backed exception only where the text supplies a decision path. Numbers labelled **chosen limits** are new design decisions, not measurements of current Ferric. Existing APIs mentioned as anchors are not permission to assume new APIs already exist. Proposed symbols can be renamed to repository conventions without changing their semantics.

The pack validator checks document/plan consistency and evidence bookkeeping. It does not prove browser correctness. No Ferric build, Qt fixture, live Google Docs journey or native Wayland test was executed while authoring this pack.

---

## C01 — Scope, outcomes and delivery order

The unified programme has four delivery bands:

| Band | Required result | Included work |
| --- | --- | --- |
| Foundation | Discovered targets behave consistently and safely | Baseline matrix; shadow-root investigation; shared validation; explicit native hint activation; validated proxy focus; source readability |
| Core product | Complex pages become easier to address without surrendering browser control | Exact-key Mixed policy; navigation/prompt integrity; declarative selectors; regions/surfaces; scoped Hints/Grid; visible Target Search |
| Hardening | Changes remain responsive, explainable and maintainable | Measurements; justified optimisation; teardown/resource tests; diagnostics; documented optional profiles; qualification |
| Capability-gated expansion | Research or implementation only where the defined evidence permits it | Double-click; in-view drag; framework writeback; child-frame/accessibility providers; alternative labels; wheel investigation |

Pass Next Key, persistent Pass-through, transient/locked Insert, full-view Grid and explicit scroll targeting are **existing foundations**, not new projects. All research items remain represented in the plan. A documented no-go is a completed research result, never a claim that the corresponding feature shipped.

The success unit is a **user task**, not a website brand or hint count: locate the intended control, enter the correct editing context, operate the page, recover browser control, and scroll the intended panel without an unintended effect. Ordinary link/document browsing is the regression control group.

## C02 — Evidence, baseline and change control

Source A records Ferric's broad interaction architecture. Source B records narrower suspected inconsistencies and comparative opportunities. This pack resolves their overlaps explicitly rather than presenting either document's unverified claims as established defects.

Fresh source inspection confirmed:

* The baseline ref and the existing one-shot forwarding merge. [R01]
* `hintFocus()` and `hintClick()` have separate final checks; their visible source does not descend into open shadow roots in the same way as collection/fresh validation. This is a **suspected behavioural inconsistency**, not a reproduced defect. [R02]
* `HintInteraction` already supports text mode, local ranking, selection and activation. Target Search must reuse it. [R03]
* The accepted Grid ADR restricts pointer delivery to `FerricPagePointerAdapter` and excludes IPC, macros, userscripts and action-discovery coordinate injection. A local palette handoff therefore requires the narrow amendment in C05. [R04]
* Hint command/rapid-action policy already has a closed grammar. New targets must extend that authority, not bypass it. [R05]

A full qutebrowser re-audit was not performed for this merge. Its source anchors are retained as Source B's evidence. Public Qt/WHATWG documentation was consulted only to check platform boundaries; those checks are separately labelled E01–E03 in `SOURCE_MAP.md`.

At implementation start, S00 records actual HEAD, dirty files, repository instructions, Qt/Chromium build, Rust toolchain and supported platforms. If HEAD differs from the pinned baseline, inspect the changed symbols and map equivalent APIs. Already implemented work is verified and marked as such. Do not overwrite it. No merge, reset, force push, installation, profile migration or work on the concurrent versioning branch is authorised by this pack.

Unrelated failing tests remain visible. Do not disable assertions, weaken Clippy, change the MSRV, update dependencies or claim a failing baseline is green. A separate, approved repair may unblock the programme; the agent must not smuggle it into an interaction slice.

## C03 — Preserve the established user model

Normal remains home. Defaults remain unchanged unless a slice's explicit contract authorises an opt-in extension or a narrowly proved correctness repair.

| Existing interaction | Preserve |
| --- | --- |
| `i` / `I` | Transient / locked Insert; real native text entry; ordinary Escape exits editing |
| Transient Enter / modified Enter | Ordinary Enter completes the transient interaction; Shift/Ctrl/Alt/Meta Enter retain the established editing behaviour; IME rules remain |
| `Ctrl+v` | Native Paste by default; explicit configuration bindings retain precedence |
| `Ctrl+Shift+v` | Native plain-text Paste by default |
| `;V` and `;v` | Persistent Pass-through and Pass Next Key respectively |
| `Ctrl+Escape` | Recovery from either explicit pass-through variant |
| `Ctrl+Shift+Escape` | Existing emergency/fullscreen behaviour, not a renamed recovery shortcut |
| `f`, `F`, existing Hint families | Current semantic actions, labels, rapid rules and configured behaviour |
| `;g` | Full-viewport Grid, not the most recently selected region |
| `;s`, `;S`, `scroll-target` | Existing Auto / Document / exact Element semantics |

Editing focus, browser mode, key ownership, semantic target, spatial scope and scroll ownership are separate. Selecting a region does not focus it, select a scroller or enable page shortcuts. Only the chosen action may do so.

Do not claim the physical recipient of a native key can be pinned to a DOM element merely by pinning its **routing owner**. Qt/Chromium may move focus during a gesture. C10 preserves native delivery and prevents Ferric from reclassifying a release as a new browser command; it does not redirect native releases to a guessed old element.

## C04 — One target identity and action-specific validation

Extend the existing Hint registry and `HintTarget { tab, generation, document }`; do not create independent DOM registries for Regions, Search, proxies or native activation.

A session-bound target record resolves to the **same retained DOM object(s)**. Its effective identity includes the originating tab/document generation, document/frame identity, session, element ID and captured configuration revision. A frame-path string or URL alone is not a document identity. A recycled DOM element can also change its meaning; action-relevant observations such as kind, link destination, associated control and proxy relationship must be checked again.

Page data is untrusted. Rust validates bounded facts and derives allowed actions from a closed table. A DOM object cannot supply a browser command, grant pointer authority, request clipboard access or select a privileged transport.

### Shared primitives, not one universal visibility boolean

Collection, fresh selection and final actions MUST use shared, testable primitives for composed ancestry, clipping, frame reachability, hit resolution, disabled/inert state and coordinate conversion. Their **policies remain action-specific**:

| Predicate | Uses |
| --- | --- |
| Visible representative | A displayed control, region, surface or proxy's visible editor/label |
| Exact point hit | Native pointer preflight; final visible click; point inside the clipped area reaches the same selected element or a legitimate descendant |
| Focus eligibility | An actual focusable target, with the proxy exception from C07; not an assumption that all visible elements accept focus |
| Scroll eligibility | Existing axis-aware scrollability plus visibility/reachability; never focus as a side effect |
| Frame reachability | Exact live owner document reachable within the allowed frame boundary |

Composed traversal must handle open shadow roots and assigned slots. Guard against loops and excessive traversal. Preserve established frame/shadow-depth and candidate bounds unless a separately justified change is recorded. Inert/disabled ancestors, including disabled form-control state, must not be bypassed by new candidates. Preserve the YouTube body/root overflow-propagation fix and per-document clipping.

A same-URL sibling is **not** the selected object. Equivalent-link heuristics may deduplicate discovery, but may not authorise a native click on a covering sibling or silently substitute a replacement at activation. Handle any default discovery change as an explicitly documented correctness repair with fixtures.

### Final action and observable outcome

Immediately before a DOM focus/click action, re-resolve the retained record, verify owner/frame/action eligibility, then execute the action once in that invocation. Preserve normal website event handlers; do not infer application success from `element.click()` returning normally. Focus success requires observing the intended deep active element, including permitted open-shadow/frame descent, rather than returning success unconditionally after `focus()`.

Every asynchronous callback verifies its original owner/session/revision. A mismatch is a bounded rejection. Closing a session must not destroy a map before an accepted callback consumes it. Explicitly transfer a short-lived lease when handing a target to another local interaction, then release the source map. Long-lived scroll/scope references remain weak; nothing is persisted.

## C05 — Action intent, transport and local interaction authority

Actions and transports are distinct. Selectors or page roles choose neither on their own.

| User intent | Default transport | Important exclusions |
| --- | --- | --- |
| Open/yank/download a valid link | Existing typed URL action | Never treat label text as a URL; preserve opener/target and permission policy |
| Focus a normal text control | Exact DOM focus | No implicit click, Enter, submit, blur or value replacement |
| Focus an approved proxy | Exact paired focus in C07 | No click on an invisible input |
| Activate an ordinary button/control | Existing semantic DOM activation | Do not globally replace `click()` with native input |
| Explicit **Pointer click** | Existing native pointer adapter, C06 | Local-only; no silent fallback or second transport |
| Set scroll target | Existing isolated-world policy setter | No focus change; preserve axis rules |
| Hint/Search within | Scoped candidate session | No automatic whole-page fallback |
| Grid within | Scoped SpatialSession | Requires the local capability handoff below |

**New local-only target:** `hint --target pointer FAMILY`, where FAMILY is one existing family appropriate for a visible target. The visible Hint action palette may also offer **Pointer click** when C06 eligibility passes. No default keybinding changes. Native pointer selection uses explicit confirmation: selecting a label highlights the target; Enter commits. It ignores auto-follow for this new target only. Reject `--first`, `--index`, `--rapid`, macros, repeat, userscripts, CLI and IPC execution for this target before any effect.

### Narrow ADR amendment

A direct physical browser-UI interaction may transfer an ephemeral, non-serialisable `LocalInteractionPermit` from a Hint/Target Search action palette into native pointer or scoped Grid orchestration. This is a proposed internal concept, not a new page bridge.

The permit carries the owning window/session and allowed operation category; it is minted only from the existing visible local UI invocation route, checked again at dispatch, and consumed once for the handoff. A keybinding triggered by a real user may begin a selection session. A later explicit selection/confirmation completes it. The permit is not a claim that the website has transient user activation; those are separate concepts.

Update ADR-0013 to allow precisely this local handoff. Do **not** enable arbitrary switcher entries, user-defined script actions, recorded macros or remote commands to inject coordinates. Preserve existing restrictions on all other Grid entry and commit paths. Add negative tests through the generic command/action executor as well as direct CLI/IPC calls; routing through an alias must not launder provenance.

## C06 — Explicit native hint activation, without false atomicity

Purpose: reach visible custom controls that depend on pointerdown/mousedown/hover semantics rather than only semantic DOM click. Source B recommends this; the exact supported sequence must be qualified in the installed engine.

Use `FerricPagePointerAdapter`; do not create a second C++ pointer service or send JS-created pointer events. Separate shared native dispatch machinery from Grid's user-facing reducer if necessary, but keep Grid's existing state and restrictions intact.

### Commit protocol

1. The user selects **Pointer click**, identifies a candidate and confirms. Consume the entire entry/confirmation gesture so its remaining keyboard events cannot type into the clicked control.
2. Capture owner, exact candidate, action, configuration revision and adapter surface stamp. The window/view must still be active, unobscured by Ferric chrome/prompts, and eligible for local native input.
3. In ApplicationWorld, resolve the retained candidate. Verify frame reachability, shared visible/interactable predicates and the chosen point. Sample the clipped target's centre and bounded inset points; choose a point that hits the exact target/legitimate descendant. Never a same-URL neighbour.
4. Convert the verified point according to C15. Return bounded preflight data, not a DOM object.
5. In the owning UI callback, reject any mismatch, intervening invalidation, changed surface/configuration or expired preflight. **Chosen preflight lifetime: 250 ms**, measured by the host from request start to dispatch; this is a conservative expiry, not an atomicity guarantee.
6. Remove passive overlays that would intercept native hit testing without changing layout. Deliver one adapter-controlled move/press/release sequence to the owning view. Do not additionally call DOM `click()`.
7. Consume the request ID and publish exactly one terminal outcome. `delivered` means Qt delivery was attempted/completed according to the adapter contract, not that the application saved, opened or accepted anything.

Native hint state is `Selecting → AwaitingConfirmationRelease → Preflight → DispatchPending → Terminal`. During the two pending states, Escape cancels; other target-selection input is consumed without dispatching another action. The confirmation gesture completes before fresh preflight starts. Do not replace this with a fixed wait timer.

One in-flight native action per window. Reuse the adapter's request deduplication/watchdog. An uncertain outcome is never automatically retried. A user may deliberately start a fresh action.

### Mandatory limitation and qualification

JavaScript preflight and later native hit testing are not atomic. Renderer animations, handler-driven changes or an overlay appearing in the intervening gap can still change the point's recipient. No public API assumption in this pack eliminates that race.

Tests must reject **observed** tab/document/frame/geometry/overlay invalidation and exercise adversarial scheduling around preflight. Record any residual unobservable interval. The UI calls this **Pointer click**, not **Guaranteed exact element activation**. It remains explicit and non-default. Keep semantic activation as the default and offer manually aimed Grid when the preflight cannot establish a usable point. If the adapter cannot preserve view/chrome/prompt isolation, the feature is not enabled on that configuration.

Measure `isTrusted` and activation-gated behaviour in fixtures, but do not infer that Qt-synthesised input satisfies every permission, popup, fullscreen, clipboard or file-picker restriction. Normal engine policies remain authoritative. [E01]

## C07 — Paired targets for editor proxies and associated controls

A focus proxy is not an ordinary invisible clickable target. Add a typed internal candidate relation between a **visible representative** and an **exact focus receiver**, both in the same live owner document in the initial implementation.

**New opt-in setting:** `hints.proxy_targets = "off" | "validated"`, default `off`, available globally/profile/site through existing precedence. Off keeps current discovery unchanged. This is a new design decision to preserve defaults while qualifying broader editor behaviour.

### EditorFocus

Initially implement only narrowly evidenced ACE-style structure, not a universal framework detector. Require a visible editor container, a unique associated textarea or supported text input inside that structure, and positive structural/role evidence beyond one CSS class. No global lookup by class name. Multiple plausible receivers, mismatched documents or an uncertain association are rejections.

The visible representative must pass normal clipping/hit/reachability rules. The receiver may fail opacity/size/pointer-hit checks **only because it is a recognised typing proxy**. It must remain connected, enabled, not inert, not `type=hidden`, not password/file/checkbox/radio, not read-only, and not `display:none` or `visibility:hidden` through its applicable ancestry. Never modify CSS, remove `inert`, change tabindex or enable a control to force eligibility.

At activation, validate the pair again, authorise the existing explicit-focus observation, call focus on that exact receiver, then check the settled deep active element. Preserve the established automatic/transient Insert behaviour; never infer Locked Insert or Pass-through from a framework name. `I` remains the user's explicit editing choice. Replaced receiver or representative invalidates the pair; do not discover a new receiver inside the old surface during activation.

### LabelControl is a different case

Bootstrap custom checkbox/radio controls are not text-editor proxies. Test the behaviour separately. An exact native `label.control` association may provide a visible representative for an enabled associated checkbox/radio. Class spelling alone is insufficient. The new typed candidate has a normal label activation action, not `EditorFocus` or external-editor capability. Label activation uses the label's ordinary semantics once; explicit Pointer click can target the visible label under C06. Verify checked-state/event counts and disabled/inert behaviour.

These opt-in candidates belong to `inputs`/`all` with deduplication against an already usable visible underlying control. CSS selector configuration cannot enable proxy exceptions when `hints.proxy_targets` is off, and cannot manufacture a pair.

Use deterministic **ACE-like/Bootstrap-like** fixtures to prove the contract, labelled as such. Claim named-framework compatibility only after a pinned actual framework build is tested with licence/provenance recorded. CodeMirror, Monaco, ProseMirror and Docs remain investigation targets until separately qualified. Proxy focus grants no content extraction/writeback permission.

## C08 — Readable browser-owned scripts and lifecycle

F-01's minimal shared-helper extraction belongs in the correctness repair. Broader readability work is separate, after those tests exist and before many new providers duplicate the code again.

Move the touched interaction helpers/operations into ordinary, versioned resource assets while retaining `BrowserScripts.js` as a small compatible builder/facade where required. A suggested new asset is `qml/scripts/interaction/InteractionRuntime.js`; its exact resource registration must follow the existing build, manifest/hash, packaging and test paths discovered in S00. Do not invent a build include path or load remote code.

First extraction is **behaviour-preserving**: use one source of helper functions, structured/JSON-encoded data arguments and the same isolated world. Do not simultaneously change to permanent per-document injection, alter world IDs or replace the Hint/scroll identity maps.

A later, measured optimisation may install a versioned helper once per document using supported script APIs. If pursued, a version/ready handshake, navigation reinjection, BFCache/fresh-world tests, teardown, one-time ownership and missing-runtime failure path are mandatory. Script availability is not inferred from a page-supplied global in MainWorld. ApplicationWorld isolates JavaScript variables, but the page still controls DOM contents. [E02]

DocumentCreation is only appropriate for runtime definitions, not eager DOM work; actual collection waits for a valid document. Do not inject a privileged web-channel bridge, expose arbitrary evaluation, monkeypatch `attachShadow`, or use DevTools in production.

## C09 — Declarative selector configuration and atomic acceptance

Implement F-05 for the seven existing families only: `all`, `links`, `inputs`, `buttons`, `images`, `media`, `scrollables`. Regions/surfaces/proxy recognisers are not a new arbitrary selector/plugin language in this increment.

**Final proposed setting shape:** `hints.selectors.FAMILY = { mode = "extend" | "replace", selectors = [CSS_STRING, ...] }`. Defaults remain the current built-in selectors. Configuration is data-only; no callback or executable JS value.

Chosen limits: at most 16 strings per family entry; at most 1,024 UTF-8 bytes per string; at most 16 KiB of selector strings per rule; at most 32 KiB in any resolved active selector policy. Reject NUL/control characters, empty strings and identical entries in the same list. An empty `replace` list intentionally disables discovery for that family; an empty `extend` is a valid no-op.

### Layering

Start from built-ins, then apply the existing global/profile/site/runtime precedence. At each participating family entry, `extend` appends unique strings in stable order; `replace` replaces the current list. Missing family means inherit. Site order remains the repository's existing priority/source order; do not add a different specificity algorithm. Effective `all` is its own family, not an undocumented union of every customised family. Secondary discovery passes, including implicit scrollable discovery, must honour an explicit replacement or disabled family.

The selector only broadens/narrows **discovery eligibility**. It does not grant a kind, native input, focus, disabled/inert bypass, URL interpretation or new action capability. Actual type/action classification still uses the normal semantic predicates. A visible custom div matched in `all` may be represented as a generic control; it is not automatically a link/editor. Link URL operations require a real validated destination. A `scrollables` replacement still requires actual axis scrollability.

Capture effective selector-policy revision and discovery provenance in the session. Fresh/final checks must apply that effective policy, not the hard-coded default `all` selector. Config changes invalidate open candidate/scope/preflight sessions; no stale policy is used to activate a now-excluded control.

### Atomic syntax validation

Rust performs structure/limits/layering checks. The supported WebEngine CSS parser validates every selector against a small browser-owned local validation document before committing a candidate configuration. Do not write a regex CSS parser. DOM selector APIs report invalid syntax, but matching cost is a separate concern. [E03]

Use a prepare/validate/commit operation with a configuration-generation token. Keep the old effective configuration active until all selector strings pass. The callback verifies the original prepared revision. If validation fails, times out or the engine is unavailable, none of that candidate configuration becomes active; show the rule/family/index and bounded reason. A stale success cannot commit a newer user edit. Do not write runtime override persistence before validation/commit succeeds, and never overwrite an invalid user-authored file to simulate rollback.

At startup with no prior accepted config, retain built-in safe behaviour and an actionable error until validation can complete; do not partially activate non-selector settings from the rejected candidate. A CLI-only structural check may report `structure-only`, but may not report full selector validation success. Requests requiring full validation return unsupported/pending according to the existing operation envelope until a real engine can validate.

Selector syntax validation is not a runtime complexity proof. C17 bounds traversal work between operations and records that a single engine CSS match is not preemptible.

### Runtime persistence publication outcome

Prepare the resolved application state, presentation data and bindings without changing the active revision. After WebEngine validation and a final request/revision check, perform private atomic persistence before publishing the prepared state. Validation and write failures before replacement preserve the full old effective configuration, overrides, revision, selector fingerprint and interaction sessions.

Atomic replacement is the publication point. A final directory-sync failure after replacement is a committed change with uncertain crash durability, not a rejected write. Publish the prepared in-memory state to match the visible file; expose `persistence: durability-uncertain` in accepted runtime command results and `config.changed` events and a readable status. Successful synchronized persistence reports `durable`; temporary/private overrides report `memory-only`. Do not attempt implicit rollback or report that the old file is intact after successful replacement.

## C10 — Exact-key Mixed ownership, integrated before QML shortcuts

Keep only `browser-first` and `mixed` in this increment. Broad page-first, wildcard key classes and per-DOM-context automatic remaps remain deferred. Mixed applies in **Normal mode only**; it does not reinterpret typing or pass-through modes.

Final proposed settings: `input.shortcut_policy = "browser-first" | "mixed"`; `input.page_keys = [CANONICAL_SINGLE_GESTURE, ...]`. Defaults are browser-first and an empty list. Support existing global/profile/site/runtime precedence. A supplied list replaces the inherited list rather than appending silently. Effective maximum: 64 entries. Reject duplicates after canonicalisation, modifier-only entries, multi-key sequences and unknown key syntax.

Use the repository's logical key-token representation for matching. A **native gesture identity** tracks press/repeat/release; this does not mean key mappings become physical-scancode, layout-independent bindings. AltGr, dead keys, layout changes and composition require native facts and regression tests; do not collapse AltGr into a user-written Ctrl+Alt mapping.

### Required decision order

1. Physical/native delivery provenance and an existing held gesture's ownership bookkeeping.
2. Actual native prompt/chrome ownership and established emergency/media handling.
3. Existing explicit modes, including Hint, Grid, Insert, Locked Insert, Pass-through and Pass Next Key.
4. In Normal, reserved browser escape/command-entry controls.
5. A pending browser keychain/count, if any.
6. Effective exact-key Mixed mapping.
7. Existing Normal handling and binding resolver.

Resolve this **before** QML special-cases `/` and `?` or ordinary bindings. Rust supplies a typed synchronous result; C++ owns native bookkeeping; QML consumes that same decision. No per-keystroke JavaScript probe, second event filter, fabricated key event or asynchronous page-first test.

### Reserved controls and chain semantics

In Normal, `:`, ordinary `Escape`, `Ctrl+Escape`, `Ctrl+Shift+Escape` cannot be assigned to `input.page_keys`. Existing native security/prompt and enabled media handling also remain outside site mappings. Normal `:` remains command entry; `/`, `?`, `f`, digits and supported modified keys may be page-owned.

A new gesture starts a browser keychain/count only when it is browser-owned. Once started, its continuation is browser-owned through completion, mismatch, timeout or cancellation. Never replay a buffered prefix or a rejected suffix to the page. A page mapping for `g` can make Normal `gg` unreachable before a chain starts; `binding-explain` must identify the conflict. A page mapping for a digit takes precedence over starting a count, but not over an already active browser-owned count/chain. Configuration changes cancel incomplete chains before the next new gesture; they do not retarget a held physical key.

Preserve current original-event and overlapping-key behaviour. Routing ownership is stable; native receiver selection remains Qt's job. No saved key events may be replayed into a new window/tab/document. Browser-originated synthetic events or delayed WebEngine copies cannot mint a new local-user action. C05 permits are unrelated to DOM `isTrusted`.

`binding-explain` reports current/requested mode, matched rule, effective policy, owner, reservation/prefix outcome and source. It must describe real routing, including QML-special keys, not just trie lookup.

## C11 — Same-document navigation and lifecycle

The baseline resets explicit pass-through on active URL changes. Preserve that until S09 can establish document identity through supported, version-tested events/observations.

Target behaviour, enabled only after the matrix passes:

| Boundary | Persistent Pass-through | Pass Next Key | Mixed rules |
| --- | --- | --- | --- |
| Full document replacement/reload/renderer replacement | Exit | Cancel | Resolve for new document before a new gesture |
| Tab/view destruction or switch | Exit | Cancel | Resolve for new owner |
| Proven same-document History API or fragment change | Retain **user-entered** persistent mode | Cancel | Re-resolve route-dependent settings before next gesture |
| Window deactivation | Preserve existing persistent mode; no replay to another window | Cancel | No extra page grant |
| Page editor/focus change alone | No inferred new policy | Existing rules | No DOM-derived policy grant |

URL equality or same origin is not identity proof. Do not preserve an old mode during an unresolved genuine navigation merely because a delayed JavaScript nonce has not returned. If supported APIs cannot distinguish a case safely, retain the baseline reset and record `same-document-preservation: unavailable` for that case. This is an explicit fallback, not an invitation to guess.

A new configuration revision is computed independently of persistent mode. Ending/recovering explicit mode reveals the correct current Mixed policy. No automatic entry into Pass Next Key is added to site-entry settings. Existing Insert focus/composition rules are unaffected.

## C12 — Interaction Context and native prompt leases

Interaction Context is a bounded ephemeral **projection** of existing mode, target, epoch, focus, frame, native prompt and scroll-policy facts. It is not durable state, a new policy engine or a global DOM index. Unknown focus/editor state remains unknown. No page role or focus notification may grant shortcut ownership.

Native prompts include file choosers, browser permission/authentication UI and Qt-owned menus for which Ferric has an observable lifecycle. Do not treat an in-page modal as a native lease; page Escape is already available via `;v` then Escape.

A native lease records owner window/tab/view/document, lease ID, saved mode/policy revision, and the **expected suspended input epoch after the prompt transition**. Comparing only the pre-transition epoch would incorrectly invalidate every restoration. Support nested observed prompts with a bounded LIFO stack, maximum depth 8; out-of-order completion invalidates the affected stack and returns to Normal. Do not restore command/hint/grid transient UI sessions or an armed Pass Next Key after a prompt.

On entry, cancel incomplete browser chains, new target/preflight actions and new page gestures. The native receiver owns input under Qt. Existing releases retain their original routing bookkeeping; do not synthesise keyboard releases or manually deliver them into guessed recipients.

On final completion, restore saved persistent Pass-through or editing mode only if owner/document, expected suspension epoch, configuration and required focus identity remain valid. Transient Insert additionally needs the same supported editable context. Otherwise return to Normal with a bounded reason. Restoration is never permitted into a new tab/document. A genuine user mode change while a prompt is open outranks restoration.

If a particular system dialog lacks a reliable lifecycle hook, do not fake a lease with elapsed-time guesses. Record it as unsupported and use a tested safe Normal-mode recovery. Browser control cannot override shortcuts the OS/compositor never delivers to Ferric.

## C13 — Regions and surfaces over the existing target system

Extend the existing typed Hint family/kind boundary, validation and payload schema; no second registry. Keep default `f` discovery stable. New families are entered explicitly as `hint regions` and `hint surfaces`; their default action is the applicable action palette, not a click.

Initial **Regions**: visible main/nav/aside and named sections, dialogs, toolbars, tablists, trees, listboxes, grids, validated editor representatives, and independently scrollable panes. Initial **Surfaces**: visible canvas/video, iframe/frame exterior and object/embed elements. Do not infer arbitrary generic divs are interactive from area alone. A later evidenced adapter can declare a specific container.

Chosen family cap: 256 regions and 256 surfaces, still within the existing 5,000 overall candidate limit. Ordinary semantic controls retain their existing minimum visibility policy. Unnamed purely visual surfaces require at least 16 by 16 CSS pixels; structurally named semantic regions are not excluded merely for being small. Exclude hidden buffers and offscreen canvases. Deduplicate by retained identity, not name; represent multiple eligible actions on one row where needed.

Regions may offer Hint within, Search within, Grid within and Set scroll target when independently eligible. A surface may offer Grid within and explicit focus **only if actually focusable**. Do not insert tabindex to make a surface focusable or select a hidden child input by guesswork. Cross-origin iframe exterior is a surface only; it conveys no child DOM authority.

Candidate labels use bounded accessible/visible naming; see C18. They are never executable commands. Selecting a region creates no persistent automatic routing state.

## C14 — Scoped interaction and lease handoff

Scope represents one exact selected region/surface in one live document, not a remembered CSS selector. A local handoff acquires a validated weak scope lease before ending the source Hint/Target Search map. If the supported engine cannot supply the required weak-reference behaviour, refuse the scoped handoff and offer the existing full-view Grid; do not retain a hidden strong-reference fallback. Only one active page-target interaction session per window; no simultaneous independent label authorities.

`Hint within` collects visible eligible descendants of the selected region, excluding the root itself. Search within uses the same descendant snapshot. Traverse only permitted open roots and same-origin frames; do not query outside the scope to find more matches. Empty scope returns `no-visible-targets`; it does not silently revert to the whole page.

The retained scope's original identity, document/frame reachability, visibility and policy revision are checked at collection and final action. Descendants replaced by virtualisation do not inherit the old candidate's identity. If the same DOM object changes action-relevant meaning, invalidate its explicit selection and require a new confirmation.

A scroll-target action hands the exact element to the **existing** setter, preserving weak state, axis rules and Auto/Document distinction. Scoped Hints/Search/Grid do not themselves change scroll ownership. Ordinary Hint/Search within remains focus-preserving and rejects cross-origin traversal. A cross-origin surface offers scoped Grid and a distinct local Hint inside frame action: explicit focus must prove the exact parent iframe/native child association before independently collecting and activating child controls. Already-focused verified frame chains may contribute semantic hints. Unverified, stale, sandbox-restricted and unsupported mappings retain their exterior only. Closed roots remain uninspected.

## C15 — Region-scoped Grid and coordinate contracts

The current zero-origin Grid root constraint is a genuine model boundary. Add an explicit distinction between the native viewport and the interaction rectangle; do not implement scoped Grid by drawing a smaller QML overlay over an unchanged core rectangle.

Use separate coordinate concepts:

* **OwnerDocumentCssPoint/Rect:** DOM client coordinates in the owning frame.
* **TopDocumentCssPoint/Rect:** those coordinates projected through supported frame geometry and clipped to the top viewport.
* **ViewLocalPoint/Rect:** Qt logical coordinates of the captured WebEngineView, used by SpatialSession/native dispatch.
* **Window/global/device pixels:** used only inside the existing native adapter's tested conversion path.

DOM CSS pixels and Qt logical pixels must not be assumed identical at arbitrary zoom. The adapter's measured/qualified mapping owns the conversion. Do not multiply by DPR twice. Border offsets, scrolling, clipping, zoom and scale are fixtures, not guessed arithmetic.

Initial semantic frame projection may reject rotated/skewed/3D transforms or unsupported mappings with `unsupported-geometry`. Do not pretend an axis-aligned bounding rectangle is an exact transform. Full-view manually aimed Grid remains available as the existing raw-coordinate fallback. Scope geometry is freshly validated and converted before entering the core.

Core scope: `Viewport` or `Region` with original owner/weak lease identity. Maintain `viewport_rect` starting at `(0,0)`, nonzero-origin `interaction_rect` clipped within it, and `current_rect` contained within that region. Crosshair and dispatch points are always ViewLocal, not region-local.

Before commit, verify owner/surface/configuration, scope validity, the captured region geometry and point containment. Chosen geometry-stability tolerance for a scoped commit: 0.5 ViewLocal logical pixel on each rectangle edge; otherwise cancel and let the user re-aim. This does not create semantic knowledge of moving canvas internals.

Do not cancel on every repaint: that would make video/canvas interaction unusable. Cancel on actual observed owner/surface/geometry/scroll invalidation according to the existing policy and the scoped additions. Internal canvas animation and the JS/native interval retain C06's explicit limitations.

Full-view `;g` is unchanged. Within a scope, 0/reset and hover reset to that scope, Backspace traverses its refinement history, and Escape cancels. Space hover may reveal controls; it never auto-clicks them. After leaving Grid, Hints/Search must recollect. Grid remains a visual tool, not a substitute for screen-reader accessibility.

## C16 — Visible Page Target Search without duplicate search logic

`HintInteraction` already owns text mode and ranking. Build the new direct-entry/native palette as a view of that machinery, reusing existing ranking helpers. Do not add a second matcher, candidate cache, element registry or durable universal-switcher scope.

New local-interactive command grammar:

```
target-search [links|controls|regions|surfaces|all]
```

Default is `all`. No query-text CLI argument in v1; query input is ephemeral UI state. No new default binding. The normal command bar and local action palette provide entry. `controls` filters eligible visible buttons, generic actions and input/control candidates; it does not indiscriminately include decorative images. `all` may aggregate explicit provider facets but deduplicates exact records and derives one valid action list.

First release is **visible targets only**, labelled in UI. Offscreen/unloaded virtualised content is not searchable until it enters the collected scope. Search does not scroll through the application or change the editor's own selection.

Initial collection once, local bounded ranking per query change, coalesced live refresh. Enter explicitly confirms the selected result; no auto-activation because a query briefly matches one item. Actionable ordinary controls may use the existing semantic default; regions/surfaces open their palette. Native Pointer click remains explicitly chosen and confirmed under C06.

Preserve selected identity on refresh only if it is still eligible and action-relevant meaning is unchanged. Stable sort uses existing deterministic rank then captured reading order/identity. If the selected candidate disappears, clear selection; never let the same list index activate a replacement row. Escape closes the palette without clearing the webpage's text selection or changing its scroll policy.

## C17 — Performance, budgets and teardown

One test/measurement programme covers F-04/F-06 and all new workflows. Measure before optimising and keep correctness as the gate.

Record separately: root traversal, selector matching, geometry/hit checks, candidate payload transfer, Rust labels/ranking, QML overlay time-to-visible, refresh, preflight and action delivery. Use monotonic durations, no raw page text. Compare the same commit/build mode, fixture, hardware, Qt/Chromium, window size and sampling method.

Reference ambition from Source B: p95 at or below 200 ms for 500 visible candidates at 80%, 100% and 125% zoom. This becomes a measured qualification result, not a promise derived from source code. Run 10 warm-up and 100 measured samples per scenario; retain raw durations and document quantile calculation. Compare 1,000/5,000-candidate pages and high-mutation pages separately. On the same host, a new common-case p95 above baseline by both 10% and 10 ms requires investigation before release; do not loosen the threshold to obtain a pass. Record variance instead of hiding noisy runs.

Chosen resource limits for new collection work: keep 5,000 output candidates and existing depth bounds; maximum 50,000 inspected nodes per snapshot, maximum 32 selector strings in a resolved family after stable deduplication, one collection/preflight in flight per window, and maximum 10 coalesced refresh starts per second while the target UI is active. Cancel on session end; no observer or timer remains active merely because a feature was used earlier. Truncation is visible, not an empty success. The 32-selector cap is in addition to C09's per-entry byte/count limits.

Do not claim hard wall-time preemption of a synchronous CSS selector/layout operation. Check budgets between units of work; measure pathological selectors and disclose that limitation. Proposed optimisations—per-pass link indexing, avoiding repeated sibling queries, incremental overlay diffs, reduced observation scope and one-time runtime installation—are allowed only with before/after evidence. No permanent full-page poller.

Run 1,000 start/cancel cycles with observer/listener/map counts returning to baseline, plus dynamic-scope handoff and forced-GC tests. JS heap bytes alone are not a deterministic leak oracle; inspect retained objects/counters and report trend. Keep DevTools, when needed to force GC for tests, on disposable local qualification instances only, never as production functionality.

## C18 — Privacy, diagnostics and optional profiles

Before new target/search/proxy providers ship, remove any reliance on control `.value` as a generic display label for these paths. Passwords, form text, hidden editor buffers and external-editor contents must not leak into hints, search labels, diagnostic logs or persisted command history. Use a bounded accessible name/associated label/title/visible representative text; omit value fallback. Limit output labels to 512 characters and also enforce a UTF-8 byte bound in the payload validator. Render as plain text, not rich HTML. Strip control characters; safely display URLs without credentials, query strings or fragments in diagnostics/search secondary labels. Actual URL actions retain the validated destination under existing privacy rules, not the redacted display URL.

`binding-explain` and a new local `interaction-status` view may show mode, requested key explanation, rule provenance, focus category, native prompt category, scroll policy and bounded counts/rejection codes. Do not implement continuous raw keystroke recording. An explicitly requested explanation of a configured token is not permission to store the typing stream. Page-derived private labels in an exported report require separate user consent; default reports omit them.

Minimum bounded errors: `stale-target`, `wrong-owner`, `wrong-frame`, `blocked-by-overlay`, `not-interactable`, `ambiguous-proxy`, `unsupported-geometry`, `unsupported-capability`, `invalid-selector`, `selector-validation-unavailable`, `config-revision-changed`, `native-prompt-active`, `dispatch-expired`, `dispatch-uncertain`, `traversal-limit`, `no-visible-targets`.

Use current Site Doctor and binding-help surfaces rather than a second diagnostic subsystem. The view assembles current facts; it never changes ownership automatically.

Optional interaction profiles are auditable, disableable **data bundles** of typed rules, adapter IDs and dated qualifications. They default to disabled, have source/version information and contain no remote executable payloads. No hard-coded hostname branches in the key router, general collector or focus observer. A document containing `role=application` does not enable a profile.

## C19 — Double-click and bounded drag

These remain separate capability-gated increments, not reasons to broaden C06 silently.

**Double-click:** qualify public Qt event ordering and the existing adapter extension against native/DOM event counters. Provide an explicit Grid/action choice, no default key remap. Do not issue two DOM click fallbacks. Abort before delivery on invalid ownership and never repeat an uncertain transaction. Ensure one double-click does not unintentionally invoke two navigation actions through Ferric's URL path.

**Drag scope:** initially in-view pointer-driven drag only, left button, no keyboard modifiers, no cross-window/desktop/file drag, no long-held button while the user selects a destination, and no automatic edge scrolling. Both points are selected before the first press. C05's local authority and C15's coordinates apply.

Before production code, S20 proves the adapter can deliver a bounded press/move/release and perform teardown on Escape, deactivation, prompt, view destruction and renderer loss. The native adapter owns the clock and terminal cleanup, not arbitrary QML timers. Chosen initial path: straight line with 12 steps over 200 ms and an absolute 1,000 ms operation watchdog. The experiment may reject unsupported widgets rather than add a secret OS-level fallback.

A live accepted drag maintains an explicit owner and held-button state. Cancellation releases the browser-generated pointer gesture only to its still-live original receiver as permitted by the adapter, then clears bookkeeping. If that receiver is destroyed, clear native state and report `target-gone`; no event is sent to a replacement view. This is pointer-gesture cleanup, not synthesis of keyboard releases. Releasing a button may commit a site's drop: cancellation cannot promise to undo effects already delivered. The UI/documentation must say so.

HTML drag-and-drop, pointer capture, canvas panning and sliders must be qualified separately. Passing one pointermove fixture is not universal drag support. If a required public-API boundary fails, record a no-go and do not enable the feature. S21's implementation is conditional on S20's positive evidence. An evidenced no-go makes S21 not_applicable; an undecided or environment-blocked predecessor leaves it blocked_environment.

**Wheel:** retain the requested investigation but add no new wheel mode by default. A wheel-only custom widget first tries page shortcuts and existing explicit scroll targeting. Only an evidenced unsolved task can justify a separate future wheel ADR; it does not reuse or corrupt a selected DOM scroll target.

## C20 — Framework editing and provider research

Focus adapters in C07 are deliberately separate from document extraction/writeback adapters. Being able to focus an editor does not make `innerText`, `.value` or a synthetic input event authoritative for its state.

Retain generic external editing for supported plain-text contracts only. A future trusted local adapter may expose probe/focus/targets independently of extract/apply/commands. Each capability requires a tested framework version range, exact live instance identity, user consent where content leaves the browser, and a conflict token. Do not download adapter code from the visited page or invoke private guessed framework properties.

Investigate ACE, CodeMirror, Monaco, ProseMirror/Tiptap and Quill independently. A focus-only result is useful and does not imply writeback. An apply path must preserve undo/formatting/selection where claimed, reject revision conflicts and demonstrate no partial mutation on validation failure. Do not roll back a failed apply by overwriting collaborative state speculatively.

Google Docs is a native typing/shortcut/targeting qualification target, not a promised rich-document external-editor adapter. No silent plain-text flattening, generic `innerHTML` replacement or accessibility-mode toggle on behalf of the user.

**Independent frames:** public Qt offers frame-related APIs on some versions, but API existence does not establish Ferric baseline support, correct geometry, child-world messaging or lifecycle safety. [E02] S23 evaluates compile/runtime gates, return-data bounds, destruction callbacks and same-origin isolation. Parent DOM traversal remains restricted; an independently executed browser-owned provider would be a separate capability, not a bypass.

**Accessibility provider:** S24 tests publicly available roles, names, bounds and actions for relevant closed/custom controls. Do not assume a canvas exposes semantic children. Preserve origin/lifecycle/privacy controls, avoid destructive actions without local intent, and demonstrate coexistence with screen readers. Until qualified, use the existing visual fallback.

## C21 — Optional hint-label ergonomics

Source B's F-08 is an evaluation, not a mandate to copy qutebrowser. Keep the current fixed-width, prefix-free deterministic labels, stable refresh behaviour, rapid semantics and Hint text search as the baseline.

Compare numeric, scattered and word-derived candidates in an isolated prototype against both existing labels and the direct text-search workflow. Measure key gestures, identification time, errors, label churn on mutation and readability. Use the same visible candidate corpus and record that automated keystroke modelling is not a human usability study.

A production strategy is not enabled by this pack merely because a prototype exists. An affirmative design decision requires deterministic/prefix-free output, documented refresh rules, no error/latency regression, and a meaningful demonstrated ergonomic gain on the chosen tasks. **Chosen evaluation trigger:** at least 10% lower median task gestures or completion time on the targeted task set, without higher observed error rate; this is a decision threshold, not a current result. Current default remains unchanged even after an optional strategy is approved. No page-value-derived word labels and no nondeterministic unseeded assignment.

## C22 — Unified qualification and evidence policy

One permanent matrix, `TEST_MATRIX.json`, owns test identities; slices add their regressions as they work rather than defer testing until the end. Its assertions—not brand names—define required behaviour.

Evidence classes are: `static-inspection`; `pure-unit`; `qml-component`; `webengine-fixture`; `native-qt-offscreen`; `native-wayland`; `live-site`; `performance`. A QTest-injected original Qt-event test is not physical desktop keyboard qualification. A live Chromium page-script probe is not an installed Ferric end-to-end pass. A source string assertion is not DOM behaviour proof. All are useful when labelled correctly.

For a suspected bug, reproduce before fixing where possible. Run the new fixture against the baseline in a disposable worktree and the patch. If it does not fail before, record non-reproduction; do not manufacture a failing test or call the change a proved bug fix. A new feature's test may be absent/unsupported at baseline; document that rather than claiming a red regression.

All applicable repository gates remain: formatting, strict Clippy, workspace tests including the CLI crate, `cargo xtask check`, engine/native suite, adapter/startup smoke, generated docs, resource manifests and `git diff --check`. Offline flags apply only when dependencies are actually available; lack of cached dependencies is an environment limitation, not permission to alter the lockfile. Native Wayland qualification records compositor, keyboard seat, scale and actual interaction coverage; startup-only smoke is never a substitute.

Manual journeys use disposable profiles and authorised test documents/accounts. Cover Docs/Sheets/Slides, Gmail, JupyterLab or a hosted advanced editor, GitHub, Reddit, Slack/Discord/WhatsApp where accessible, YouTube, Kanban, maps/diagrams and embedded viewers. Record exact URL privately when it contains a document/account token; publish a sanitised URL and reproducible task. Do not send messages, share documents, make purchases or change production data merely to complete a test.

Important journeys include autocomplete Enter versus submission, page Escape, rich-text paragraphs, roving tabindex/ARIA widgets, tab/file-picker transitions, nested scrolling, input proxies, canvas controls and cross-origin exteriors. For each distinguish target fixture support, native Ferric support and live-site outcome. Authentication denial, engine version, codecs, PDF UI and permissions are separate compatibility failures, not automatically keyboard defects.

## C23 — Agent execution, dependencies and completion

`plan.json` is the dependency graph, not a collection of independent prompts to paste manually. The default is one coordinator executing slices serially in numeric order when dependencies pass. Optional parallelism is permitted only for ready slices with disjoint declared lock groups and separate worktrees. Disjoint lock-group names alone are not enough: the coordinator must also compare actual file scopes. Shared test-runner, resource and generated-registry registration edits belong to the coordinator even when workers add separate fixture files. No two agents edit `BrowserScripts.js`, core Hint schemas, the key router, configuration commit path or generated registries concurrently.

Each slice has a bounded intent, source mappings, relevant contracts, allowed existing anchors/new modules, implementation order, tests, prohibitions and exit criteria. Verify actual local symbols before editing; no fictitious method calls or unreviewed replacement modules. A change outside the write scope requires a short scope-amendment record explaining the necessary call chain and preserving all contracts; architectural/behavioural changes outside this specification require the slice to stop.

Allowed statuses: `not_started`, `in_progress`, `blocked_environment`, `blocked_specification`, `implemented_unqualified`, `complete`, `research_no_go`, `not_applicable`. Do not equate a source edit, CI pass or documentation-only no-go with a shipped feature. Research is complete only with a concrete capability/evidence report. Production slices requiring native evidence remain `implemented_unqualified` until it exists; downstream pure work may proceed if its contract dependency is implemented, but release cannot claim qualification.

A context reset reloads the ledger, current git diff, the assigned slice and contract sections. Do not reconstruct progress from memory. Each slice produces its exact files changed, tests/commands and outcomes, owner/epoch/identity review, risks, capability decisions and next ready work. Record both successes and unavailable tests. Re-run affected tests on the integrated head, not merely on each agent's isolated branch.

The agent may make local commits on its dedicated implementation branch if the execution environment permits, but this pack does not authorise pushing, merging, releasing, installing into the user's daily browser, destroying worktrees or changing personal profiles. Read-only source/research and disposable test state are the default.

## C24 — Definition of done and non-goals

A release slice is done only when the intended problem/feature is exercised, exact-target and input lifecycles remain valid, the relevant positive/negative tests pass, performance and teardown evidence exist where required, documentation/config/codecs agree, and native/live claims match recorded evidence. An independent available review should be used for the native/config/identity boundaries; when only self-review is available, label it self-review rather than imply independence.

No architectural rewrite; no universal WebApp mode; no duplicated one-shot mode, search engine, page registry, scroll authority or native input service; no arbitrary remote scripting; no cross-origin parent traversal; no forced closed-root access; no privacy regression through labels/logs; no blanket native-click fallback; no duplicated action after an uncertain result; no defaults replaced by an unmeasured label strategy; no guarantee of atomic DOM/native targeting; no claim that all applications, PDFs, IMEs or accessibility technologies are supported.

The merged programme retains the broader vision **and** the targeted recommendations: reliable activation/focus first, shared machinery next, then better ways to locate and operate modern application surfaces. Capability limits become explicit tested decisions instead of assumptions delegated to a smaller implementation model.

---

## Canonical invariants

The source invariants WEBAPP-001–022 are retained in the traceability map. The following unified invariants are the normative version when wording conflicts with a platform limitation:

| ID | Invariant |
| --- | --- |
| UI-001 | Existing Normal/Insert/Pass-through/Hint/Grid/scroll defaults remain, except explicitly qualified opt-in work or proved repairs. |
| UI-002 | Page observations never authorise browser capabilities, policy or configuration. |
| UI-003 | One native key gesture has one routing owner; native receiver focus is not rewritten by Ferric. |
| UI-004 | New/queued actions use their original owner and revision, not whichever tab is later active. |
| UI-005 | Shared DOM primitives have action-specific eligibility; no universal relaxation for hidden inputs. |
| UI-006 | Exact semantic records and paired proxy identities are revalidated; no selector/name/URL replacement at activation. |
| UI-007 | Native pointer actions are explicit, local-only, one-transport and at-most-once; residual non-atomicity is disclosed. |
| UI-008 | Local palette handoff does not open CLI/IPC/macro/script coordinate injection. |
| UI-009 | Configuration is typed, bounded, precedence-consistent and atomically activated/persisted. |
| UI-010 | Mixed policy is decided before QML shortcuts; reserved escape/command controls survive. |
| UI-011 | Browser chains/counts never replay their buffered keys into the page. |
| UI-012 | True document replacement invalidates page-key sessions; same-document retention needs positive identity evidence. |
| UI-013 | Native prompt restoration validates the suspended epoch and original owner; one-shot/target UI is never resurrected. |
| UI-014 | Focus, target selection and scroll policy remain independent unless explicitly changed. |
| UI-015 | Scoped geometry uses qualified coordinate conversions; no CSS/DIP/device-pixel conflation. |
| UI-016 | Existing target registry, Hint text ranking, scroll setter and pointer adapter remain the single authorities. |
| UI-017 | Scope/search/proxy/native sessions release maps, weak leases, observers and listeners at terminal boundaries. |
| UI-018 | Search is visible-only, ephemeral and confirmation-based; virtualised replacements cannot inherit selection. |
| UI-019 | Parent access to cross-origin/closed-root internals is not enabled; provider research is capability-gated. |
| UI-020 | Page values, secrets and arbitrary target text do not become persistent history/telemetry. |
| UI-021 | Drag has bounded native ownership and teardown; cancellation does not promise to undo a drop. |
| UI-022 | Editor focus and rich writeback are distinct permissions; no generic rich overwrite. |
| UI-023 | Performance claims use measurements on a specified platform and sampling plan, not candidate caps alone. |
| UI-024 | Missing tests/infrastructure are not passes; research no-go is not feature completion. |
| UI-025 | Default label strategy stays stable; alternatives require measured, explicit opt-in approval. |
| UI-026 | Unresolved specification/security/platform gaps block only the dependent slice, not justify invented semantics. |

The clipboard shortcut compatibility change is user-approved in
[ADR-0018](../../architecture/ADR-0018-native-clipboard-ownership.md).
Historical source documents and evidence retain their original observations.
