# Ferric: Keyboard-Accessible Web Applications
## Consolidated architecture and implementation proposal — v0.2

**Date:** 11 October 2026  
**Status:** Proposed; not an assertion that unimplemented features already exist  
**Repository:** `thomasaiwilcox/ferric-browser`  
**Validated baseline:** `main` at `4337056f632de0c710943fa6df76c67cf14b8b47` (merged PR #9, one-key webpage shortcut pass-through)  
**Related concurrent work:** `feature/versioned-browsing` is separate and out of scope.  
**Purpose:** Make sophisticated web applications meaningfully operable from the keyboard, without creating site hacks, weakening origin boundaries, duplicating existing input modes, or compromising simple-page usability.

> **Core decision:** Ferric will make web application interaction progressively keyboard-addressable using distinct, composable authorities: semantic targets, spatial targets, key ownership, editing, scroll ownership and native prompt ownership. It will not implement a universal, opaque “webapp mode.”

---

## 1. Executive summary

Modern web applications blend links, toolbars, rich editors, canvas surfaces, independently scrolling panes, virtualized lists, custom widgets, embedded frames, and extensive application shortcuts. The friction visible in qutebrowser discussions is not simply that these sites require typing: it is that the browser and the application often disagree about who owns a key, what has focus, which element is actionable, or where scrolling and pointer gestures should be directed. Historical qutebrowser reports are useful as failure archetypes, **not** proof every bug persists on current releases.

Ferric has already implemented significant answers: validated Hints; a 3×3 recursive native Grid; explicit document-local scroll policies; transient and locked Insert; persistent Pass-through; and, in PR #9, **Pass Next Key**. The new feature forwards one real key press, repeats and release, then restores Normal; it is already bound to `Ctrl+Shift+v` and `;v`. Persistent Pass-through uses `Ctrl+v`. `Ctrl+Escape` is the designated recovery chord in both variants; `Ctrl+Shift+Escape` retains its existing emergency behaviour. Do not duplicate any of this.

The remaining high-value work is, in order: reproducible webapp task benchmarks; selective browser/page shortcut ownership; interaction and native-prompt ownership integrity; semantic regions and surfaces; region-scoped Grid; and a bounded, searchable palette of live page targets. Drag gestures, framework-aware external editor adapters, independent cross-origin frame enumeration and accessibility-tree targeting should be gated later work, not prerequisites for the primary navigation improvements.

**Product success criterion:** A user can complete a specified task in a modern webapp solely with keyboard input, without unintended browser commands or page actions, with understandable targeting and recovery, and without the browser claiming access to semantics it cannot actually inspect.

## 2. Evidence and failure taxonomy

Evidence from qutebrowser's documentation and user issues should be treated as *classes of failure*. Historical reports often have platform, Qt, browser-version and configuration dependencies.

| Problem class | Representative evidence | What Ferric should address |
|---|---|---|
| Competing page/browser shortcuts | qutebrowser issue [#4271](https://github.com/qutebrowser/qutebrowser/issues/4271) asks to forward just one key; issue [#8031](https://github.com/qutebrowser/qutebrowser/issues/8031) describes injected keys being typed | Pass Next Key **already implemented**; selectively allocate recurring keys without giving up Normal mode |
| Rich editing and hidden editor proxies | Issues [#2545](https://github.com/qutebrowser/qutebrowser/issues/2545), [#8802](https://github.com/qutebrowser/qutebrowser/issues/8802), [#8463](https://github.com/qutebrowser/qutebrowser/issues/8463) | Preserve native typing, inspect bounded focus facts, never generically overwrite a framework document |
| Mode, focus and window desynchronization | Issues [#8308](https://github.com/qutebrowser/qutebrowser/issues/8308), [#7234](https://github.com/qutebrowser/qutebrowser/issues/7234), [#6691](https://github.com/qutebrowser/qutebrowser/issues/6691) | Epoch-bound input, explicit prompt leases and safe fallbacks |
| Multi-pane applications | Multiple independently scrollable panes are common; focus is not reliable scroll authority | Ferric **already has** explicit scroll target; expose it alongside region actions |
| Semantically opaque controls, canvas and drag-only UIs | Qutebrowser's [double-click request #446](https://github.com/qutebrowser/qutebrowser/issues/446); custom widgets and canvas limitations | Spatial Grid **already covers** point click/hover; region-scoped Grid and bounded gestures remain |
| Cross-origin frames / closed shadow roots | Real web-platform origin and encapsulation boundaries | Do not bypass; frame-as-surface Grid fallback; independent frame/accessibility providers only if supported by public APIs |
| Compatibility not caused by keyboarding | Engine version, codec, user-agent, permissions, application denial | Diagnose separately; do not try to repair with keyboard heuristics |

Google Docs, Sheets, Slides, JupyterLab, Gmail, Discord, Slack, Trello, GitHub's rich interfaces, YouTube, diagram editors and map applications are useful **benchmark targets**, not site-specific implementation requirements. For each failure, distinguish whether it is reproduced in *current Ferric*, a hypothetical gap, or a historical qutebrowser report.

## 3. Verified Ferric baseline and architectural boundaries

The following is present on `main` at the pinned SHA. Anything explicitly marked **proposed** below is not yet in Ferric.

| Capability | Status and verified source | What remains |
|---|---|---|
| Hints | **Implemented.** `core/src/hints.rs` owns `HintTarget {tab,generation,document}`, candidate identities, typed kinds, prefix matching and fresh activation. `BrowserScripts.js` collects in the isolated application world, walking same-origin frames and open shadow roots. | Region and surface kinds; scoped collection; visible candidate search; independent frame research |
| Grid | **Implemented.** `core/src/spatial_navigation.rs` owns `SpatialSession`, `SpatialOwner`, `SurfaceStamp`, precision/depth and dispatch lifecycle; Qt adapter provides native pointer dispatch. | Region-root geometry; continued native qualification; gestures |
| Scroll targets | **Implemented.** ADR-0014 specifies Auto / Document / exact Element with `WeakRef`, axis checks, fallback on invalidation and stable queued TabId. | Expose in region target action palette, without introducing new scroll policy state |
| Insert | **Implemented.** ADR-0015: `i` transient; `I` locked; native Enter path; input epochs, IME and focused-editor observation. | Broader editor and IME qualification, optional interaction diagnostics |
| Pass-through | **Implemented.** `Ctrl+v` persistent; ordinary Escape belongs to page; `Ctrl+Escape` exits. | Consider preserving explicit persistent pass-through across same-document SPA changes after qualification |
| Pass Next Key | **Implemented in PR #9.** `Mode::PassThroughOnce`, `Ctrl+Shift+v` / `;v`, actual Qt press/repeat/release, entry-key and delayed-event handling, automatic return to Normal. | No new feature; retain and regress-test during future key policy work |
| Site rules | **Implemented**, `SiteRule {id, pattern, priority, set}` and precedence in `ferric-browser-config`; current site-scope validation allows a narrow set including `input.entry_mode`. | Add strictly typed *new* shortcut-policy settings and validation; do not presume they already parse |
| Binding explanation | **Implemented**, typed `binding-explain`; existing source and conflict provenance. | Include actual page-owner decision and policy provenance; do not leak page text |
| External editor | **Implemented for bounded plain-text contexts**, not a generalized rich document adapter. | Separate opt-in framework adapters after research |
| Live WebEngine testing | **Implemented**, QML WebEngine runner and native key router suite. PR #9 reported 49 native cases, 783 workspace Rust cases and 41 QML cases; CI passed. | Webapp task matrix and continuous qualification on sites |

**Important current code constraints:**

1. `FerricBrowserRuntimeSurface.qml::handleBrowserKey` special-cases Normal-mode `:`, `/` and `?` before the general binding path. Therefore selective page ownership **must be resolved before this logic**, not only inside Rust's binding trie.
2. `SpatialSession::observe_surface` currently rejects nonzero `root.x` and `root.y`. A region-scoped Grid **requires an explicit geometry-model change**. Resizing the overlay alone is insufficient.
3. `BrowserScripts.js` has a shared versioned script source and existing bounded Hint collection, candidate IDs, frame paths and freshness. Extend it; do not add an unrelated DOM registry.
4. Existing `matching_site_rules()` resolves HTTP(S) URL patterns in priority/source order. Extend its validated settings carefully; never add hostname `if` statements in C++, QML or page scripts.
5. The `feature/versioned-browsing` branch is concurrent, but not included in this `main` baseline. Do not assume its unmerged code, mutate its implementation pack or couple this proposal to it.

## 4. Product model and non-negotiable decisions

**Normal is home.** Key ownership, editor focus, scroll target, semantic target and pointer target are separate. The user may focus an editor, select a different scroll pane, and still invoke a semantic target without Ferric silently rewriting focus or scroll authority.

**One physical gesture, one owner.** A key press, repeats and release must remain owned by the same browser/page/native prompt authority even when DOM focus, mode or window activation changes. If its original destination disappears, cancel safely; never reassign the gesture to another tab.

**The page supplies facts, not policy.** Page DOM may report an editor, ARIA role, region, candidate or modal. It cannot instruct Ferric to forward shortcuts, enter Pass-through, alter user configuration, or gain privileged browser capabilities.

**Native when native matters.** No synthetic DOM `KeyboardEvent` or `PointerEvent` used as a generic substitute for user input. Keep trusted native key and pointer routing at the existing Qt boundary.

**Do not confuse a visual fallback with semantic control.** Grid can act at an observed point without knowing whether it is a button, cell or painting surface. It cannot promise semantic selection, offscreen search, or drag support where none exists.

**Privacy:** No raw editor contents, typed keys, clipboard, arbitrary page text, page target snapshots or app-specific tokens in persistent browser history/telemetry. Debugging must be local, opt-in where necessary and redacted.

## 5. Target interaction architecture

Conceptually:

```text
QWindow native keyboard events
      |
      +--> validated, synchronous key-owner decision (browser / page / native)
      |          |                                  |
      |          +--> existing Ferric modes          +--> Qt/WebEngine native delivery
      |                    |
      |                    +--> Normal / Insert / Locked Insert / Pass-through / Pass Next Key
      |
Typed user commands and target actions
      |
      +--> live semantic candidates (existing Hint identity + new Region/Surface kinds)
      |          +--> Hints / Page Target Search / action palette
      |          +--> scoped hinting / assign existing scroll policy
      |
      +--> SpatialSession (view surface stamp + optional validated region scope)
      |          +--> Grid point actions / future bounded gestures
      |
      +--> bounded interaction diagnostics
```

**Ownership placement:** Rust retains typed configuration, command/action registry, mode state, session/target identity and policy provenance. C++ owns original physical key and native pointer gesture boundaries. QML presents UI and routes validated requests. Isolated-world page scripts collect bounded DOM facts and perform exact validation; page-world JavaScript never becomes the authority for browser policy.

Do not make `InteractionContext` a new persisted global source of truth. Prefer a **bounded ephemeral observation/projection** assembled from existing mode state, focus probes, frame facts, current URL and native prompt state, versioned with input epoch and document generation.

## 6. Workstream A — Selective page-owned shortcuts (P1)

### 6.1 Already done: do not reimplement

Keep the current `Mode::PassThrough`, `Mode::PassThroughOnce`, current bindings, original Qt events, Ctrl+Escape recovery and Ctrl+Shift+Escape emergency behaviour. Do not introduce `page-keys next`, another one-shot mode, a second native key filter, or a separate JS key-synthesis service.

### 6.2 v1 proposed policy

Add a **per-site, exact-key Mixed policy** while leaving all default browsing behaviour unchanged. A minimal typed model is:

```rust
// Proposed conceptual types, not current APIs.
enum ShortcutPolicy { BrowserFirst, Mixed }
enum KeyOwner { Browser, Page, NativePrompt }
struct ShortcutRule {
    policy: ShortcutPolicy,
    page_keys: BoundedSet<CanonicalPhysicalKey>,
}
```

`BrowserFirst` is the default everywhere. `Mixed` sends precisely configured physical single-key gestures to the page **only while in Normal mode**, except genuinely reserved Ferric/native controls. Existing Insert, Locked Insert, Pass-through and Pass Next Key retain their current semantics. Do not implement broad `PageFirst`, wildcard key families, focus-derived automatic key switching, or page-supplied shortcuts in v1. Those remain possible later extensions only after evidence.

Example proposed configuration (requires new typed validation before it becomes valid):

```toml
[[site_rules]]
id = "docs-shortcuts"
pattern = "https://docs.google.com/document/*"
priority = 20
set = { "input.shortcut_policy" = "mixed", "input.page_keys" = ["Ctrl+Alt+m", "Ctrl+Shift+7"] }
```

A page-key entry must be one canonical physical key gesture, not a multi-key browser chain. Validate case, modifier combinations, representation, duplicate/collision handling, count/keychain ambiguity and current binding compatibility. **For v1, reserve `:` as the guaranteed Normal-mode command entry, plus `Ctrl+Escape`, `Ctrl+Shift+Escape` and `Ctrl+Shift+v` as required browser controls.** Reject attempts to put these in `input.page_keys` with a clear validation error. The user may still forward a literal `:` using the already-implemented Pass Next Key mode; more flexible command-entry remapping requires a separate usability/security review. Use existing binding-key canonicalization where compatible. Do not add a new arbitrary command language. `input.page_keys` should be bounded (e.g. no more than 64 entries per rule, after a test-backed schema decision), and full effective policy is resolved from the existing site-rule layering and validated atomically. The existing `input.entry_mode` and `site_rules.set` must not silently interpret these new values until their schemas are extended.

### 6.3 Physical event-routing contract

Resolve the page-vs-browser decision **before** QML special handling of `:`, `/` or `?`, and before the ordinary Normal binding resolver. Do not merely unbind keys in Rust while QML continues to consume them. The synchronous dispatch path must communicate the resolved owner back to the native router so that the original gesture is tracked end-to-end. Reuse the existing `acceptedKeys_`, `forwardedKeys_`, `entryKeys_`, `passGesture_` and epoch concepts where they remain appropriate.

Rules:

- A single nonrepeat physical press establishes owner, originating window/tab/view and epoch.
- Repeats and final release follow that decision, including after a transient mode transition; delayed WebEngine parent-item deliveries cannot invoke a second Ferric action.
- No new browser binding is executed on a release or repeated native event already owned by the page.
- A mode switch or tab change cannot cause the same held key to cross to a new owner. If the original receiver no longer exists, fail closed without synthesizing an unrelated key to another page.
- Compose/IME and modifier-only events are not interpreted as shortcuts without the native event facts needed to decide ownership safely.
- Native prompts and guaranteed rescue controls outrank per-site page mappings.
- The active site rule is computed from the actual owning tab URL, not whichever tab is active when an asynchronous response returns.
- Re-evaluate policy on the next *new* gesture after a rule/config or same-document URL change, not in the middle of a held gesture.

### 6.4 Recovery and UX

`Ctrl+Escape` remains the recovery from the two explicit pass-through modes. `Ctrl+Shift+Escape` retains emergency/fullscreen behaviour. A future Mixed policy must never take away the ability to enter browser command mode or reach a documented recovery control. In v1, `:` remains browser-owned for entering commands, and `Ctrl+Shift+v` remains browser-owned for one-shot forwarding; reserve these together with both rescue chords, browser chrome focus, and all native security/permission prompt controls from page mapping. Any further reservations must be explicitly documented and test-protected, not inferred by websites.

Extend `binding-explain` so it states **effective owner, matched site rule, overridden Ferric binding, reserved status and reason**, without logging the actual typed input stream. UI indications for persistent Mixed policy should remain unobtrusive: show effective ownership in help/diagnostics and concise status on explicit request, not a constant large badge.

### 6.5 SPA navigation decision

Currently, `note_navigation_url_change()` exits both Pass-through and Pass Next Key on any active URL change, including same-document route/fragment updates. Preserve this current behaviour until regression tests are established.

Then qualify the following **target behaviour**:

- **Document replacement, tab switch, view destruction, renderer death:** cancel explicit page-key mode, invalidate epochs.
- **Same-document History API change or fragment-only URL change:** keep *user-entered persistent* Pass-through if the same document, view and owner remain; do not promote to or enable it automatically. Re-evaluate new site-rule Mixed mappings before the next new gesture.
- **Pass Next Key:** retain its stricter cancellation on any URL change or deactivation, unless native tests demonstrate an ordinary legitimate key sequence is broken; do not weaken one-shot safety for generic SPA convenience.
- **Page focus/editor changes:** alone do not automatically grant a new key policy.

This is a future behaviour change, not a claim about present code. Verify same-document identity explicitly; comparing only URL strings is insufficient. Never preserve ownership across true navigation just because two pages share an origin.

### 6.6 Acceptance

A site-configured `f`, `/`, `?` or `Ctrl+f` goes exclusively to the page; **attempting to configure `:` for page ownership is rejected in v1**, leaving Normal command entry available. No duplicate DOM key events; no accidental Normal command after async WebEngine propagation. Default/unconfigured websites behave exactly as before. Existing Pass Next Key and Insert native suites remain green.

## 7. Workstream B — Interaction context and native prompt leases (P1/P2)

### 7.1 Ephemeral observation

Extend current focused-element and input-epoch observations with a small, typed, bounded report of facts:

```text
window / tab / document generation / frame path
current Ferric mode and input epoch
focus kind: none | plain-text | rich-text | proxy | widget | unknown
editable yes/no/unknown
active page-key policy (browser-first/mixed/explicit pass-through)
scroll-policy summary (auto/document/element)
native prompt owner and bounded lifecycle state
last failure code (not sensitive text)
```

A page may supply an ARIA role or element observation, but this is only a *hint* about context. No generic automatic switch to Mixed or Pass-through based on DOM claims. Retain ADR-0015's weak focus identity, bounded ancestry and asynchronous query rules. Unknown means unknown, not editable.

### 7.2 Native interaction leases

Implement a bounded lease only at native boundaries Ferric can actually observe (file chooser, popup/dialog, permission/auth prompt or similar). A lease carries owner window/tab/view, document generation and input epoch; it suspends ordinary page/binding dispatch to the relevant native receiver and provides a safe restoration outcome.

- Completion restores a prior mode only if **the original owner and epoch still match**; otherwise Normal.
- Lost native focus, view destruction, prompt cancellation and tab switch invalidate the lease.
- No deferred release may activate a newly focused page.
- Do not invent synthetic key release events to solve uncertain native prompt state.
- Distinguish a browser-owned Qt dialog from an in-page DOM modal; page modal Escape should use Pass Next Key or existing Pass-through.

If Qt does not expose a reliable lifecycle for a particular prompt, document it and qualify the safer fallback instead of making a false claim.

## 8. Workstream C — Region and surface target providers (P1)

### 8.1 Reuse existing Hint identity

Do not create another independent page-element registry. Extend `HintKind` or introduce a thin typed target family mapped to the existing validated `HintTarget`/`HintCandidate` representation. The current `element_id`, frame path, live document identity, geometry and fresh-check boundary remain authoritative. Provider-returned capabilities must be validated by Rust, not blindly accepted from DOM JSON.

Introduce two **distinct candidate families**:

- **Region:** meaningful structural/focus/scroll areas, such as main, navigation, dialog, toolbar, tablist, tree, listbox, grid, editor pane and independent scroller. Region selection is not equivalent to clicking it.
- **Surface:** visual interaction areas such as canvas, video, large non-semantic widget container, iframe and embedded object. A Surface may have no inspectable child semantics.

No unbounded `div` enumeration. Candidate collection remains capped, deduplicated, visible, composed-clipped and hit-tested. Adopt evidence-based thresholds to avoid 5,000 meaningless region labels. Preserve the recent YouTube viewport-overflow propagation fix by reusing shared clipping rather than writing new visibility logic.

### 8.2 Action capabilities

A validated region may expose `hint-within`, `search-within`, `grid-within`, and `set-scroll-target` if those actions make sense. A surface may expose `grid-within` and focus but not necessarily click or navigation. Native activation and clipboard actions remain kind- and policy-gated. Cross-origin frame contents are **not** exposed through parent DOM traversal; the parent iframe box is only a surface anchor.

### 8.3 Interaction flow

1. User invokes region selection through a typed command or an action palette entry.
2. Ferric shows a bounded set of labelled candidate regions/surfaces.
3. User selects a candidate by existing Hint labels or Page Target Search.
4. Ferric freshly validates identity, frame path, view geometry and action capability.
5. The selected action launches a scoped, ephemeral interaction or safely fails with a reason.

Do not implicitly focus, scroll or change page-key mode on selection. Focus and scroll ownership are separate.

## 9. Workstream D — Region-scoped Grid (P1)

**Concrete repository gap:** `SpatialSession::observe_surface()` currently requires `root.x == 0 && root.y == 0`. Implementing Grid within a selected panel therefore needs explicit model changes, not a QML mask. Preserve the existing full-viewport Grid and its tested invariants.

### 9.1 Proposed geometry model

Separate:

- `viewport_rect`: full view-local logical CSS/dip rectangle, normally `(0,0,w,h)`;
- `interaction_rect`: freshly validated region or surface rectangle **clipped within the viewport**; may have nonzero origin;
- `current_rect`: recursive 3×3 refinement inside `interaction_rect`;
- `crosshair`: view-local point, never region-local point accidentally passed to native Qt.

The `SpatialOwner`/`SurfaceStamp`, tab, document generation and region candidate identity are captured. Add a typed `SpatialScope::Viewport | Region(ScopedTarget)` or equivalent without broadening native pointer authority.

### 9.2 Validity and lifecycle

Before native dispatch:

- Revalidate owning window, tab, document, view, surface serial/revision and the original region identity.
- Recheck that the region is visible, its clip still intersects the viewport, the click point falls inside the current intersection, and movement/reflow has not invalidated the capture.
- The region must not be reselected by CSS selector, old coordinates or approximate DOM proximity.
- Scaling, zoom, scroll, repaint, tab switch, modal, fullscreen, renderer replacement and physical-pointer override follow existing fail-closed rules; add scoped-specific tests.
- For a cross-origin iframe, validate its **outer element rectangle** only, then send native pointer input into the visual surface. Do not inspect or access its content document.
- No automatic synthetic click after a hover. User sees revealed controls and explicitly chooses hints/search/Grid next.

### 9.3 UI

Expose `Grid within region` from the region action palette. Initially avoid a new default keybinding until ergonomic qualification; normal `;g` must continue to mean full-viewport Grid. A scoped overlay must draw inside the selected view-local region, while native event dispatch uses absolute view-local coordinates. `Backspace`, `0`, hover and Escape retain current meanings but reset to the **scoped** root, not the full viewport. Show the scope label in Grid help/status.

**Completion condition:** canvas, video and a cross-origin iframe exterior can be selected and clicked by native Grid without touching the mouse, and stale/reflowed targets reject rather than redirect.

## 10. Workstream E — Page Target Search (P1)

Implement the planned “search and select links” workflow as a **searchable view over existing freshly validated candidates**, not as a separate JS find result that guesses a clickable ancestor.

### 10.1 Scope

First release indexes **currently visible, actionable candidates** from existing Hint providers and new Region/Surface providers. Tell the user that search is currently scoped to visible targets; do not promise offscreen or virtualized items can be selected without changing view. Unbounded whole-document search and virtualized offscreen enumeration are later research tasks.

### 10.2 UX

Proposed commands (new names; not currently implemented):

```text
:target-search links
:target-search controls
:target-search regions
:target-search all
```

Search considers bounded accessible label, visible text, role/kind, and a safe URL display for actual links. Reuse `ferric-browser-core/src/switcher.rs` deterministic ranking primitives and, where appropriate, the native switcher presentation shell, but create an **ephemeral, non-persisted page target scope**. Source matches, target identity and frame remain opaque to the page.

After selecting a result, open the applicable action palette (or execute an explicit target-specific default). Freshly revalidate before executing, return a clear error if the candidate has moved or vanished, and do not silently match by the same text elsewhere.

No accidental opening of arbitrary URLs embedded in page labels; link URLs come only from validated link candidates. Large or hidden target lists are bounded. Avoid hot-path collection on every keystroke: snapshot, rank locally, coalesce page refreshes, and revalidate at action time.

## 11. Workstream F — Bounded native pointer gestures (P2)

Current Grid supports left/middle/right click and hover, not a complete mouse replacement. Research and then implement in separate reviewable slices:

1. **Double-click:** qualify real Qt double-click semantics (timing, event ordering, button ownership, cancellation); do not simulate two unrelated JavaScript `click()` calls.
2. **Bounded drag:** two explicitly selected points, bounded native press/move/release, captured owner, movement revision, session timeout, guaranteed cleanup, no automatic retries after uncertain dispatch.
3. **Wheel/local scrolling:** only if specific benchmark cases cannot be solved through existing explicit scroll targets. Do not duplicate scroll policies.

The gesture state machine belongs to the Rust core and Qt native adapter, not arbitrary QML timers. No cross-window, desktop-wide or unrestricted IPC mouse automation. Separate ADR and native Wayland qualification are required before any release claim.

## 12. Workstream G — Framework-aware rich editor adapters (P3, capability-gated)

Generic external-editor writeback remains restricted to plain-text controls that can be captured and safely conflict-checked. A modern rich editor may store document state outside the DOM text value; assigning `innerText`, dispatching synthetic input or replacing HTML risks data loss and broken undo/collaboration state.

Future trusted local adapters may expose bounded `probe`, `focus`, `extract`, `apply`, `targets`, and `commands` capabilities. Each must be tied to an explicitly supported framework/version and application origin, with revision/conflict checks and user permission for extraction/writeback. Candidates for independently evaluated support include CodeMirror, Monaco, Ace, ProseMirror/Tiptap and Quill. None is guaranteed until a supported API and live test prove the path.

**Google Docs:** do not claim generalized document extraction/writeback. The immediately achievable approach is Locked Insert, Pass-through/Pass Next Key, selective Mixed shortcuts, region/target search, scroll ownership and spatial fallback. Page-owned native typing and app state remain authoritative.

Never download executable adapters from the visited site. Never automatically flatten a rich document to plain text. On uncertainty, refuse writeback without mutation.

## 13. Workstream H — Cross-origin frames and accessibility providers (research only)

**Initial supported fallback:** target the parent-visible iframe as a Surface, then use native Grid within its rectangle. The parent page does not inspect frame DOM.

Investigate only public Qt WebEngine APIs for independent per-frame isolated-world collection with authenticated frame identity, bounded results and fresh activation. If unsupported on Ferric's target Qt versions, explicitly stop; do not use private Qt methods, DevTools remote automation or cross-origin parent DOM access.

Separately assess whether public accessibility APIs expose useful roles, labels, bounds and actions for custom widgets/closed shadow implementations. Treat accessibility as a complementary provider, not an origin bypass or universal solution for canvases. No new provider is shipped until it can be independently revalidated and safely deactivated on navigation.

## 14. Workstream I — Interaction diagnostics and optional profiles (P2)

Extend `binding-explain`, existing Site Doctor and typed status projections to explain **why an action did or did not work**. Diagnostics should include:

```text
Ferric mode: normal
Input key: Ctrl+f [display only if explicitly requested]
Resolved owner: page
Policy: mixed
Rule: docs-shortcuts
Browser binding shadowed: search
Originating window/tab: stable current owner
Focus class: editor-proxy / unknown
Scroll policy: element
Hint/region availability: bounded counts
Last rejection: stale-region / unsupported-frame / native-prompt / etc.
```

No raw keyboard logging, editor text or candidate-label persistence. Reports that include page-derived labels must be explicitly generated and clearly marked private.

Optional versioned interaction profiles may bundle validated site rules, known target-provider settings and documented qualifications. They must be disableable, auditable, locally trusted and **never** privilege-giving purely on the basis of page script requests. No scattered `if hostname == ...` in the key router.

## 15. Representative real-world task journeys

### Google Docs
Open document → manually enter `I` for Locked Insert → edit and use real app shortcuts (`Ctrl+v` persistent or Mixed page bindings when implemented) → return to Normal → find named Share/comment controls via Hints or Page Target Search → choose document outline as region → if needed pin a specific scroll pane with `;s` → use Grid within the rendered editor for opaque visual interactions. **Do not assert rich-document editor writeback or every Docs shortcut works until live qualified.**

### JupyterLab / web IDE
Search for notebook/panel → select Region → scope Hints or Grid → give notebook its configured native shortcuts without surrendering all browser keys → use Locked Insert for code entry → target scrolling in long outputs → native prompt lease guards file chooser → framework adapter is optional later.

### Kanban board
Hint/search card → page-owned Escape via the existing `;v` + Escape → return to Normal → next phase adds explicit bounded drag between card positions. No generic JS drag spoofing.

### YouTube/player
Focus player → `;v`, then `f` (already tested in PR #9) → resume Normal → hover Grid to reveal controls → hints/search locate captions/settings. `Ctrl+v` supports longer native player shortcut sessions.

### Diagram/map canvas
Select the visual Surface → Grid within rectangle → click/hover item → only after separate native gesture qualification, bounded pan/drag/resize. Do not infer semantic canvas objects without application accessibility or adapter evidence.

## 16. Benchmark and test strategy (P0 before product slices)

Create a deterministic WebEngine fixture suite under `crates/ferric-browser-engine-qt/tests/fixtures/webapp-interaction/` (or another repository-consistent fixture directory), with browser-side success/failure observations that do not rely on screenshot OCR.

| Fixture | Mandatory behaviour |
|---|---|
| `shortcut-conflicts.html` | `f`, `/`, `?` and Ctrl combinations with Ferric bindings; reserved `:` validation; exactly one owner, native down/repeat/up |
| `one-shot-key.html` | Existing Pass Next Key, modifier-only, repeated, overlapping, cancelled and lost-release behaviour |
| `spa-routes.html` | History API vs fragment vs full navigation; persistent vs one-shot semantics and revalidation |
| `editor-proxy.html` | Hidden textarea, focus replacement, contenteditable, IME/preedit with declared limitations |
| `multi-pane.html` | Independent focus, scroll target, region selection and nested scrollers |
| `virtualized-list.html` | Disappearing/replaced candidates and no silent retargeting |
| `toolbar-aria.html` | Landmark, toolbar, listbox, grid, tree, custom role and label discovery |
| `canvas-iframe.html` | Native scoped pointer actions; same-origin and cross-origin-frame exterior |
| `reflow-geometry.html` | Region crop, scale/zoom, clipping, overlaps, transforms and invalidation |
| `modal-escape.html` | DOM modal requiring a real page Escape while Ferric recovers |
| `native-dialog.html` | Keyboard-only chooser completion, focus restoration and canceled stale callbacks |
| `hover-gesture.html` | Hover exposes controls; later bounded double-click/drag |
| `rich-editor.html` | Plain-text support versus unsupported rich writeback, zero mutation on refusal |

Also maintain a **dated live task matrix** on disposable profiles for Google Docs/Sheets/Slides, Gmail, JupyterLab, a Monaco and CodeMirror editor, Trello or similar, Discord/Slack, GitHub heavy views, YouTube, a map/diagram canvas and an embedded iframe tool. Distinguish actual Ferric app tests from isolated Chromium page-script and static-source tests. Record OS/compositor, Qt/WebEngine/Chromium version, page version where possible, exact task, input sequence and outcome; do not claim results not run.

Core metrics:

- keyboard-only task completion rate (target: 100% for each *declared supported* benchmark task);
- unintended browser commands/page actions (target: zero in deterministic tests);
- lost/duplicated/differently owned press-release sequences (target: zero);
- unintended mode/focus/scroll transitions (target: zero);
- number of interactions and escapes needed, before/after each feature;
- failure recoverability and explanation accuracy;
- p50/p95 target collection, search, routing and pointer dispatch latency;
- CPU and memory overhead on constrained hardware such as the X270.

**Performance guardrail:** no continuous whole-document polling; no page script installed per ordinary Normal key; no high-frequency DOM snapshot on every keystroke. First benchmark current baseline and establish measured, hardware-specific regression thresholds before inventing absolute latency promises. Retain the existing Hint p95 target of ~200 ms for 500 visible candidates as the initial reference, subject to its recorded hardware/test conditions.

## 17. Implementation plan, slices and definition of done

Each slice should land as an independently reviewable PR from the pinned/current `main`, with a focused ADR where it introduces a new input/target authority. Keep separate branches for concurrent work and explicitly reconcile/rebase with `main`; never drop commits from the separate versioned-browsing branch. No agent should introduce an incompatible cross-crate model before a shared typed contract and tests are merged.

| Slice | Responsibility | Primary code areas | Exit gate |
|---|---|---|---|
| 0 | Benchmark baseline, fixtures, evidence and failure classification | engine-qt tests, testing docs | Reproductions and measured current results, no product behaviour changed |
| 1 | Exact-key Mixed ownership policy and recovery provenance | config `InputConfig`/`SiteRule` validation; core typed policy and bindings; C++ key router; QML early key dispatch; IPC/help | Native key-ownership suite; no regression to Pass Next Key, Insert or reserved keys; explicit `: / ?` cases |
| 2 | Same-document vs document-replacement lifecycle; interaction facts | `browser_ui_tab_state.rs`, input router, focus probes, testing | SPA tests, epochs/held-key safety, mode transition documentation |
| 3 | Native prompt leases and safe restoration | Qt native adapter, mode state, prompt presentation | File picker and permission modal fixture tests; mismatch returns Normal |
| 4 | Region/Surface candidate families and typed action capabilities | `core/src/hints.rs`, `BrowserScripts.js`, Rust Hint adapter, QML overlay | Live region/iframe identity, visibility, clamping, no unbounded scan |
| 5 | Region-scoped Grid geometry and dispatch | `core/src/spatial_navigation.rs`, `browser_ui_spatial.rs`, QML Grid overlay/native pointer | Nonzero scope geometry, stale-region rejection, scale/zoom and native Wayland tests |
| 6 | Ephemeral Page Target Search and scoped Hints | core switcher ranking helpers, Hint provider, Qt target palette | Bounded visible candidate search and fresh execution; no persistence/leakage |
| 7 | Interaction Doctor/help and optional site profiles | binding explanation, Site Doctor, typed config | Accurate cause provenance, redaction and user overrides |
| 8 (separate) | Double-click then bounded drag | new reviewed ADR, core SpatialSession/native pointer | Full native gesture matrix, no stuck buttons or unbounded mouse control |
| 9 (gated) | Editor adapters, accessibility and frame API spikes | isolated provider/adapters and qualification docs | Explicit go/no-go with public API proof and consent boundaries |

**Repository gate for each relevant PR:**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
cargo xtask test engine
cargo xtask test adapter
```

Run `cargo xtask test wayland` where supported; record its actual keyboard-seat coverage, not just the startup smoke. Native Qt/WebEngine input suite must run whenever a change touches key gesture ownership. Use installed-release live checks for Google Docs/Jupyter/YouTube tasks, not merely source tests. Confirm generated command/binding documentation stays current. If unrelated test gates fail, diagnose them rather than weakening tests for convenience.

## 18. Acceptance invariants

**WEBAPP-001** Normal is the stable return mode; no webpage can silently grant itself browser policy.  
**WEBAPP-002** Existing `Ctrl+v`, `Ctrl+Shift+v`, `;v`, `Ctrl+Escape`, `Ctrl+Shift+Escape`, `i` and `I` retain their established behaviour unless an explicitly reviewed compatibility change is approved.  
**WEBAPP-003** One native physical key gesture has one owner from first press through terminal release, including repeats and delayed renderer deliveries.  
**WEBAPP-004** Normal-mode `:`, `/`, `?` and all registered keys pass through the same early ownership decision; `:` remains reserved for browser command entry in v1, while `/` and `?` may be page-owned; no QML bypass.  
**WEBAPP-005** Reserved rescue, security/prompt and native controls cannot be captured by per-site page mappings.  
**WEBAPP-006** Site rules are typed, bounded, precedence-consistent and atomic; unconfigured sites remain browser-first.  
**WEBAPP-007** A held gesture cannot be reassigned across tab, document, view, focus or policy changes.  
**WEBAPP-008** Full document replacement resets explicit page-key sessions; same-document preservation is allowed only after identity-based qualification.  
**WEBAPP-009** Existing Insert and Pass Next Key preedit/release safety remains intact.  
**WEBAPP-010** Focus, scroll and target selection stay independent unless the user invokes an explicit action.  
**WEBAPP-011** Region/Surface candidates use existing document/element/frame freshness contracts, with bounded and shared geometry logic.  
**WEBAPP-012** Cross-origin iframe DOM and closed shadow contents remain outside parent traversal.  
**WEBAPP-013** Scoped Grid preserves view-local pointer coordinates, validated region containment and `SpatialOwner`/`SurfaceStamp` lifecycle.  
**WEBAPP-014** A moved/replaced/hidden region rejects safely; no target is silently substituted by text or coordinates.  
**WEBAPP-015** Target Search is ephemeral, bounded, labelled as visible-only in v1 and freshly revalidated.  
**WEBAPP-016** Native prompt restoration requires matching owner and epoch; otherwise Normal.  
**WEBAPP-017** Pointer gestures have bounded lifetime and terminal cleanup; uncertain dispatch is never blindly retried.  
**WEBAPP-018** Generic rich editor writeback is rejected unless a validated adapter safely supports it.  
**WEBAPP-019** No page text, arbitrary target list, raw typed key stream or editor contents enter durable browser data/telemetry.  
**WEBAPP-020** Failures are explainable; current capability limits are documented without overstated qutebrowser or Ferric compatibility claims.  
**WEBAPP-021** Ordinary document, link and scrolling behaviour remains responsive; performance regressions are measured on constrained hardware.  
**WEBAPP-022** Native test evidence, deterministic fixtures and real-site qualification are accurately separated.

## 19. Out of scope and go/no-go boundaries

Not in the initial implementation: universal “WebApp mode”; hostname-specific key-router branches; remote downloadable adapters; synthetic browser-privileged key/pointer injection; unrestricted desktop input automation; persistent DOM identity or page target lists; generic rich-editor overwrite; automatic page-owned keys inferred from untrusted DOM; fake cross-origin DOM access; arbitrary closed-shadow inspection; fully semantic canvas navigation without accessibility evidence; generalized gestures before native release cleanup qualification; virtualized offscreen document-wide target search; and broad product compatibility claims based only on synthetic tests.

A capability that Qt does not expose through supported public APIs becomes a documented limitation or separate research spike—not a reason to breach the architecture. Prefer a narrower, understandable keyboard workflow over a powerful but unpredictable fallback.

## 20. Final recommendation

Ship the incremental product in this order: **(1) baseline webapp tests → (2) precise site-scoped Mixed key ownership → (3) interaction/native focus safety → (4) named regions and surfaces → (5) Grid within a region → (6) visible-target search**. The existing Pass Next Key feature handles one-off conflicts now and must not be rebuilt. Defer native gestures and editor adapters until task measurements establish their value and test boundaries.

This gives Ferric a distinctive promise: **when semantics exist, name the target; when they are weak, narrow the region; when they are absent, point spatially; and always know which system owns the next physical key.**

---

## Repo references checked for this version

- [PR #9 — Add one-key webpage shortcut pass-through](https://github.com/thomasaiwilcox/ferric-browser/pull/9)
- [Pinned current `main` merge commit](https://github.com/thomasaiwilcox/ferric-browser/commit/4337056f632de0c710943fa6df76c67cf14b8b47)
- [`docs/user/webpage-shortcuts.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/docs/user/webpage-shortcuts.md)
- [`docs/architecture/ADR-0014-explicit-scroll-target.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/docs/architecture/ADR-0014-explicit-scroll-target.md)
- [`docs/architecture/ADR-0015-transient-insert.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/docs/architecture/ADR-0015-transient-insert.md)
- [`crates/ferric-browser-core/src/hints.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-core/src/hints.rs)
- [`crates/ferric-browser-core/src/spatial_navigation.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-core/src/spatial_navigation.rs)
- [`crates/ferric-browser-engine-qt/src/browser_key_router.cpp`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-engine-qt/src/browser_key_router.cpp)
- [`crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml)
- [`crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js)
- [`crates/ferric-browser-config/src/lib.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/main/crates/ferric-browser-config/src/lib.rs)
