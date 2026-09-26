# Ferric Spatial Grid Navigation

## Implementation handoff proposal v0.1

**Status:** Implemented in sliced form; native desktop qualification remains the final release gate recorded in `docs/testing/spatial-grid-native-input.md`.

**Review date:** 26 September 2026.

**Repository reviewed:** local `main` implementation worktree on 26 September 2026; the implementation, generated documentation, fixtures, and qualification checklist referenced below are included in this handoff.

**Evidence boundary:** The implementation has passed focused Rust/Qt tests, workspace tests, repository checks, the offscreen Qt/QML adapter smoke, and the nested Wayland startup smoke. The interactive desktop qualification matrix remains pending because this environment's compositor lacks the virtual-keyboard protocol; no usability study or performance benchmark was performed.

### How to use this handoff

This document is normative where it uses **must**, **must not**, or names an exact v0.1 command, type, state, or slice exit criterion. Text labelled as a design sketch or future work is non-normative.

Implement the slices in order. A slice is complete only when its focused tests and the repository gate named for that slice pass. Do not start a dependent slice with a red predecessor, and do not expose `:grid` to users before Slices 0–5 are complete. Slice 0 is a stop/go gate: if public Qt APIs cannot produce the required page input safely, record the failure and stop rather than substituting DOM activation.

The reviewed worktree contains in-progress changes in several files this feature will eventually touch, including command, input, hint, Qt bridge, QML runtime, IPC, and documentation files. Before each slice, inspect `git status` and the relevant diffs. Preserve and integrate with those changes; a slice must not restore, overwrite, or reformat unrelated work.

## 1. Recommendation

Introduce **Grid mode**, a browser-owned, keyboard-controlled way to address a point in the active page viewport through repeated 3×3 subdivision. Keep ordinary element hinting as a separate, complementary feature.

The product contract should be:

> Choose a region, refine the point, and explicitly perform a pointer action at the visible crosshair.

It should not be:

> Ferric identifies what the user meant and activates it automatically.

The first release should concentrate on reliable point selection, left/right/middle clicking, explicit hover, cancellation, and lifecycle safety. It should not include automatic semantic snapping, drag-and-drop, coordinate macros, screenshots, artificial page freezing, or desktop-wide mouse control.

The principal engineering uncertainty is not drawing or subdividing the grid. It is delivering a correctly routed, browser-equivalent pointer action through the actual Qt Quick/WebEngine composition without introducing a remote-debugging dependency or weakening engine security.

**Implementation gate:** Prove the native pointer path before spending substantial effort on the polished overlay or additional interaction modes.

### Fixed v0.1 decisions

The following are settled for implementation and are not left to the slice author:

- The feature is named **Grid mode** in user-facing text and **spatial navigation** in code and architecture prose.
- The public entry command is `grid`; the default Normal-mode binding is `; g`.
- The default layout is the row-major 1–9 layout shown below. Alternative presets and dedicated layout configuration are deferred; users may eventually remap registered Grid-mode bindings, but the first implementation need not expose a preset switch.
- The maximum refinement depth is eight. Reaching the limit or one-logical-unit precision stops further refinement and never commits an action.
- The overlay is passive: it has no `focus`, `MouseArea`, `TapHandler`, or pointer-accepting delegate.
- Selection and session policy remain in Rust. QML maps and renders an immutable projection and reports host observations; it never decides that a request is fresh enough to dispatch.
- The native adapter uses only public Qt APIs. DOM `.click()`, `elementFromPoint`, JavaScript-created events, Qt private headers, QtTest in production, DevTools/remote debugging, and OS-wide injection are prohibited fallbacks.
- Pointer commands are local-interactive only in v0.1. IPC, CLI forwarding, userscripts, macro record/playback, generic repeat, and switcher/action discovery cannot enter Grid mode or commit a point.
- Hover is explicit and separate from aiming. A successful click ends the session. A successful hover resets selection to the root of the still-valid surface. Any rejected dispatch ends the session.
- A session stores no page text, DOM identity, screenshot, URL-derived locator, or durable coordinate history.

## 2. Precedent and positioning

Windows Voice Access documents a numbered grid that can be subdivided for finer pointer positioning. Keyboard-first precedent also exists: `warpd` provides a grid mode using repeated quadrant selection and separate click actions. Consequently, neither recursive region selection nor keyboard-driven grid pointing should be presented as Ferric inventions. [E1][E2]

Ferric's proposed contribution is integration: a page-scoped spatial mode with the browser's existing key registry, input ownership, tab/document identity, cancellation rules, configuration, and policy boundaries.

The intended relationship is:

- **Hints:** select an identified element and apply a semantic action, such as opening its validated URL.
- **Grid:** select a point in the rendered page and request a pointer action there.

Grid is particularly worth evaluating where element discovery is insufficient: canvas controls, custom widgets, opaque embedded content, image regions, and precise positions within a larger interactive element. These are qualification targets, not an assertion that every such application will work without additional support. Pointer locking, continuous drawing, path-dependent hover, dragging, and OS dialogs are different interaction problems.

## 3. What the current repository provides

| Existing component | Observed behaviour | Proposed use |
|---|---|---|
| `ferric-browser-core/src/model.rs` | Explicit modes; reducer-only mutation of application internals; tab generation and document identity; `capture_target()` | Add `Mode::Grid`; reuse target freshness while the adapter-bound session remains ephemeral in `BrowserUiRust` |
| `ferric-browser-core/src/input.rs` | Mode-specific binding trie; numeric counts apply only in Normal mode; changing modes resets pending input | Route grid keys through the shared input system, not a separate QML-only key parser |
| `ferric-browser-core/src/dispatcher.rs` | Command provenance distinguishes Keyboard, UI, CLI, IPC, Userscript, Macro, and Switcher | Enforce local interactive authority rather than exposing unrestricted coordinate injection |
| `ferric-browser-engine-qt/src/browser_key_router.*` | Application event filter routes key presses and Escape shortcut overrides before WebEngine, but does not currently route key releases or expose auto-repeat | Extend this boundary so a consumed Grid gesture owns its matching release and repeated presses can be rejected [R11] |
| `ferric-browser-engine-qt/src/lib.rs` and `browser_ui_navigation.rs` | `BrowserUiRust` owns ephemeral `HintSession` state built from pure core types, while mode changes still enter the reducer | Follow this ownership pattern for the view-bound spatial session instead of putting Qt view identity in `ApplicationState` |
| `FerricBrowserRuntimeChrome.qml` | Active `WebEngineView` instances live inside the `webViews` stack; the hint overlay is anchored over that stack | Bind a native page-pointer adapter to the exact active view instance and render the passive grid in the same view-local frame [R12] |
| `qml/components/FerricHintOverlay.qml` | Browser-owned presentation component, but also `focus: visible` and clickable `MouseArea` delegates | Reuse presentation conventions; do not copy focus or pointer interception indiscriminately |
| `qml/scripts/BrowserScripts.js` | Hint collection retains element identities, traverses bounded same-origin frames/open shadow roots, and has a `hintClick()` helper calling `el.click()` | Keep semantic discovery separate; do not mistake this helper for a native coordinate-click backend |
| `FerricBrowserRuntimePresentation.qml` | Uses the hint script helper for the `click` action | A new coordinate path must not silently fall back to this existing DOM action |
| ADR-0007 | Describes validated hint boundaries and rejects remote-debugging/automation sockets | Preserve those boundaries; document spatial input as a distinct contract |
| `m0-163-ui-automation-boundary.md` | Explicitly says offscreen smoke tests do not replace native input qualification | Require real desktop evidence for the new pointer path |

These are observations of the reviewed worktree, not claims about later branches. [R1][R2][R3][R4][R5][R6][R7][R8]

A particularly important distinction is that the development specification already warns that synthetic DOM `.click()` does not necessarily provide trusted user activation. The existing DOM helper is therefore not enough to establish that file inputs, fullscreen controls, or canvas coordinates will work as required. [R5][R9]

## 4. Scope and non-goals

### Initial interaction surface

The root rectangle is the **currently visible content area of the active WebEngine view**. It excludes the tab bar, command bar, status bar, browser dialogs, permission prompts, DevTools, other windows, and the desktop.

Do not interpret a point outside that rectangle as a request to control another surface. Do not translate a stale request onto whichever tab happens to be active later.

Browser-window fullscreen is in scope after the transition: entering or leaving it cancels the current selection, and the user may start a fresh session once the same view is stable. Page-requested content fullscreen is not a Grid target in v0.1; entry there is rejected. A Grid click may still trigger a fullscreen request, which the existing browser policy adjudicates after Grid exits. Browser-owned prompts always take precedence.

### Explicit exclusions for v0.1

No DOM target snapping; no automatic clicking at a depth threshold; no page zoom or screenshot magnifier; no continuous mouse movement; no dragging or button-held state across user keystrokes; no double-click command; no OS cursor warping; no global input-injection service; no coordinate replay through macros, repeat, IPC, or userscripts.

Do not suspend scripts or freeze a live renderer just to keep the image still. A frozen picture is not proof that the underlying click target is unchanged.

## 5. User-facing interaction

### Entry and vocabulary

Use **Grid mode** in the interface and **spatial navigation** in architecture documentation.

Entry command: `:grid`.

Default binding: `; g`. The reviewed default binding table already groups several hint actions under `;`, and does not contain this sequence. User overrides remain authoritative. [R10]

Initially, entry is supported from Normal mode or a directly user-operated command bar. A user in Insert or PassThrough mode first returns to Normal mode. Supporting additional entry surfaces later must preserve their focus and composition contracts.

### Default grid

```text
┌───────────┬───────────┬───────────┐
│     1     │     2     │     3     │
├───────────┼───────────┼───────────┤
│     4     │     5     │     6     │
├───────────┼───────────┼───────────┤
│     7     │     8     │     9     │
└───────────┴───────────┴───────────┘
```

Use row-major numbering as the only built-in v0.1 layout. Never infer orientation from the physical keyboard or Num Lock. A numpad-oriented preset, letter preset, or dynamic legend is post-v0.1 work and requires its own interaction review. Existing binding overrides may remap command keys, but the core cell identity remains its row-major digit; the rendered label may instead show the active remapped key as specified in Section 6.

### Controls

| Input | Meaning |
|---|---|
| `1`–`9` | Replace the current rectangle with that child rectangle; update the crosshair to its centre |
| `Enter` | Left-click the displayed crosshair, then end the session |
| `Shift+Enter` | Right-click the displayed crosshair, then end the session |
| `Ctrl+Enter` | Middle-click the displayed crosshair, then end the session |
| `Space` | Explicitly hover at the displayed crosshair without clicking; on successful dispatch, restart selection from the current viewport |
| `Backspace` | Restore the previous refinement rectangle; at the root, do nothing |
| `0` | Reset refinement to the root rectangle; never click |
| `Escape` | Cancel the session without issuing a new pointer action |
| `?` | Show concise help without resizing the page |

The shared command/binding vocabulary is fixed as follows:

| Command | Valid mode | Arguments | Notes |
|---|---|---|---|
| `grid` | Normal, Command | none | Start `AwaitingSurface`; reject without a live active tab and local interactive authority |
| `grid-refine` | Grid | one of `1` … `9` | Refine once; no count and no auto-repeat |
| `grid-click` | Grid | exactly `left`, `right`, or `middle` | Create one dispatch request and enter `DispatchPending` |
| `grid-hover` | Grid | none | Create one hover request and enter `DispatchPending` |
| `grid-back` | Grid | none | Restore one parent rectangle; no-op at root |
| `grid-reset` | Grid | none | Restore root; never dispatch |
| `grid-help` | Grid | none | Toggle the concise, non-focus-stealing help projection |
| `grid-cancel` | Grid | none | End the session and return to Normal without dispatch |

The default Grid-mode bindings are `1`–`9`, `Enter`, `Shift+Enter`, `Ctrl+Enter`, `Space`, `Backspace`, `0`, `?`, and `Escape`, mapped to the commands above. `grid-*` commands are internal interactive commands: they may be registered for validation, help, and user key remapping, but must be rejected at non-interactive ingress points. They must not be advertised as remotely invokable actions.

The key normalizer must produce the exact tokens `Enter`, `Shift+Enter`, `Ctrl+Enter`, `Space`, `Backspace`, and `Escape`. Alt, Meta, unsupported modifier combinations, IME/composition input, and an empty token are consumed while Grid owns keyboard focus but do not mutate selection. This prevents an unsupported modified key from leaking into the page.

The binding modifiers select the action. They are **not** automatically forwarded as pointer modifiers. For example, `Ctrl+Enter` means an unmodified middle click, not Ctrl+middle click. Modified pointer actions would need separately named commands later.

A middle click is a mouse-button action, not a promise to open a background tab. The page and engine decide its effect. A right click likewise follows the normal page/browser context-menu path; it does not bypass the page's event handling or Ferric's policy.

### Example

```text
; g       Enter Grid mode
8         Keep the bottom-centre ninth
2         Keep the top-centre ninth of that region
6         Keep its middle-right ninth
Enter     Click the centre of the current region
```

There is no ambiguity about which of the newly displayed nine cells Enter selects: it selects **none of them**. Enter acts on the crosshair at the centre of the current rectangle. To target a different child centre, select that child first, then press Enter.

Every grid selection requires a deliberate key press. Holding a digit must not race down multiple levels through auto-repeat. Holding Enter must never generate repeated clicks. Repeated independently pressed digits remain valid. `QKeyEvent::isAutoRepeat()` must cross the native key-router boundary; Grid consumes repeated presses without feeding the binding resolver.

### Refinement is not page zoom

The page stays at its current scale and position. Only the targeting rectangle shrinks. Do not animate the page, enlarge the selected content, or recenter it. Preserve the surrounding visual context.

### Hover is separate from aiming

Do not send page pointer-move events while the user is merely selecting grid cells. Otherwise refinement can open menus, show tooltips, or move the target before the user commits.

`Space` is the deliberate exception: send a hover action, then let the user inspect the changed page and choose the next target. Do not automatically activate a menu that appeared.

Escape cannot undo effects of a hover already sent. Documentation must distinguish cancelling the current selection from reversing previous page interaction.

## 6. Geometry and presentation

### Coordinate contract

Define the authoritative geometry in **WebEngine-view-local Qt logical coordinates**, using finite floating-point values. Qt's higher-level item and event geometry generally uses device-independent pixels; physical framebuffer pixels and DOM CSS coordinates are separate spaces. [E3][E4]

For raw grid selection:

```text
view-local logical rectangle
    → view-local logical crosshair
    → mapping into the owning Qt window/scene event space
    → native input dispatch
```

Do not multiply the point by device pixel ratio or page zoom as a generic rule. Map between explicitly named spaces only when the receiving API requires it. The browser has already rendered the zoomed page into the view.

Any future DOM-assisted feature requires its own tested CSS-to-view mapping. It must not contaminate the basic spatial coordinate model.

### Subdivision

For parent rectangle `(x, y, w, h)` and zero-based column `c` and row `r`:

```text
child.x = x + c × w / 3
child.y = y + r × h / 3
child.w = w / 3
child.h = h / 3
point   = centre(child)
```

Retain a bounded history of authoritative rectangles, or an equivalent exact subdivision representation, so Backspace restores the previous geometry. Avoid rounding every division to integers. Any rendering alignment to physical pixels must not silently move the logical click target.

For a hypothetical 1920×1080 logical viewport:

| Selections | Region width | Region height |
|---:|---:|---:|
| 1 | 640.00 | 360.00 |
| 2 | 213.33 | 120.00 |
| 3 | 71.11 | 40.00 |
| 4 | 23.70 | 13.33 |
| 5 | 7.90 | 4.44 |
| 6 | 2.63 | 1.48 |

These values follow directly from `width / 3^depth` and `height / 3^depth`. They measure positional resolution, not selection speed or guaranteed success. If the desired point lies inside the selected rectangle, the centre's error is bounded by half its width and half its height. A narrow control can still require another subdivision.

Use a hard maximum of eight refinement steps and an earlier precision stop once both dimensions are at or below one logical unit. A precision stop only stops subdivision; it must never click. Validate this bound against supported view sizes and the actual engine input resolution before release. Do not market it as physical-pixel-perfect until that has been measured.

### Visual design

Draw only the active 3×3 rectangle, a distinct outer boundary, a small crosshair, and a compact status line. Do not retain every ancestor grid on screen.

Rust projects one display label for each cell. With defaults these are `1`–`9`. If active user bindings remap a `grid-refine N` command, select its display label deterministically from the final active binding set: shortest key sequence first, then lexical order; join a multi-key sequence with a visible space. Show `—` when a cell has no active binding. QML renders these projected strings and does not inspect binding configuration itself.

Use restrained, high-contrast lines and small label backgrounds. Keep the target area visible rather than covering the whole viewport with an opaque tint. Reserve a clear gap at the crosshair centre so a tiny target is not hidden by the indicator itself.

When nine readable labels no longer fit, keep the target rectangle and crosshair and display a fixed miniature key legend away from the target. Do not continuously shrink text into illegibility. This is a presentation change, not a change to key semantics.

An example status line:

```text
GRID  8›2›6  |  Enter click  Space hover  Backspace back  Esc cancel
```

Update immediately without decorative transitions. A held keyboard key should not cause an animation or inertia. No continuous repaint loop is required when neither the selection nor its owning surface changes.

## 7. Lifecycle and cancellation contract

### Session ownership

A session is ephemeral and belongs to one window, one tab, one document/generation, and one specific live WebEngine view instance.

The existing `Target` captures tab generation and document identity. Extend the session with explicit surface/view identity and a surface revision; do not assume the current `Target` already represents viewport or presentation changes. [R1]

Cancel before an action can be delivered after any of the following known changes:

- active tab or owning window changes; window deactivates or closes;
- navigation/document replacement, renderer termination, view destruction/replacement, or resource discard;
- viewport dimensions, clipping, page zoom, device-scale mapping, or relevant Qt scene transforms change;
- an observed top-level scroll invalidates the selection;
- fullscreen transitions or a browser/OS modal prompt changes input ownership;
- pointer lock or an existing physical-button-held interaction makes the action ambiguous.

For v0.1, increment the adapter `surfaceRevision` and cancel the Rust session on: target-item replacement/destruction; target window change/deactivation; view visibility or lifecycle loss; view-local width/height change; mapping/scene-transform change reported by Qt; page zoom change; active-tab change; navigation start, URL change, same-document navigation callback, renderer termination, discard, or view close; entry/exit of fullscreen; and presentation of a browser-owned prompt or attached DevTools. Duplicate observations are harmless because cancellation is idempotent.

An observed same-document navigation should conservatively invalidate the session as well. Do not wait for a full document replacement to recognise a changed browsing surface.

Ferric does not currently receive a complete native signal for every page- or inner-container scroll. A physical wheel gesture is covered by pointer takeover, browser scroll commands are unavailable in Grid mode, and known top-level navigation/geometry callbacks cancel. Script-driven scroll, inner scrolling, animation, and reflow remain part of the raw-coordinate limitation and must be stated in user documentation; do not add periodic JavaScript polling to claim exhaustive coverage.

Use these bounded status families: user cancellation returns `Normal mode`; surface/target/geometry/renderer changes report `Grid cancelled: page view changed`; window or physical-pointer takeover reports `Grid cancelled: pointer or window took control`; prompt ownership reports `Grid cancelled: browser prompt took control`; pre-dispatch rejection reports `Grid action rejected`; and watchdog expiry reports `Grid action outcome uncertain`. Do not include coordinates or page data. Do not queue a click for delivery when the page later becomes available. A new session requires new intent.

### What this cannot guarantee

No session stamp proves that every pixel under the crosshair stayed unchanged. In-place animations, inner scroll containers, cross-origin frame content, canvas redraws, and arbitrary page reflow can change the visible target without replacing the top-level document.

Do not promise exhaustive mutation detection. The contract is a native-like click on a live visible point, with protection against known ownership/geometry changes—not element-identity protection equivalent to hints.

Do not cancel on every DOM mutation or every video frame. That would make the feature unusable and would reintroduce the DOM dependency it is intended to avoid.

### Physical mouse takeover

A genuine physical pointer movement should end Grid mode and return pointer authority to the mouse. Avoid ending a session merely because a queued old event is observed; distinguish fresh physical input from this feature's own dispatched actions.

For a mouse press or wheel event received while the grid is active, the conservative initial rule is: cancel and consume that initiating gesture, rather than letting it unexpectedly activate the page under an overlay. Consume the corresponding button release too. The user can then interact normally.

This rule must be tested for usability; it may be relaxed later only with a clear, consistently documented handover policy. Synthetic events generated by the grid must not trigger its own takeover handler.

The native pointer adapter's application event filter is authoritative for takeover. While dispatching its own event sequence it sets an internal guard; only unguarded spontaneous mouse move, press, wheel, tablet, or touch input produces the physical-pointer signal. Slice 0 must verify which event attributes reliably distinguish real and adapter-generated events on the supported Qt build. If that distinction is not reliable, the safe fallback is to cancel before dispatch and suppress only the exact internally generated sequence, not to ignore all pointer input for a time window.

### Keyboard ownership

Grid keys must not leak to the website. This includes the key release after an action changes focus or closes the overlay. Consume the entire triggering key gesture and suppress its auto-repeat.

Extend `FerricBrowserKeyRouter` rather than adding a QML `Keys` handler on the overlay. The router must emit press metadata including `isAutoRepeat`, and it must remember the native key identity of every accepted non-repeat press. Check matching releases before the normal enabled/focused-window press gates, so a release is consumed even if handling its press exited Grid mode or changed focus. Remove an entry on its matching release; replace stale tracking when a new non-repeat press with the same identity arrives; clear everything only when the router is destroyed. Do not consume unrelated releases, and do not synthesize releases toward the page.

`ShortcutOverride` remains special: retain the current Escape protection and add no broad “accept every shortcut” rule. The router's held-key set is bounded by the finite set of simultaneously pressed physical keys, and duplicate accepted auto-repeat events do not add entries.

Entering the mode clears pending normal-mode prefixes and numeric counts. The existing resolver already resets on mode changes and interprets counts only in Normal mode; use that structure instead of adding a second count parser. [R2]

Do not interfere with compositor-level shortcuts. Do not inject arbitrary key-up events into page content as a substitute for correct ownership tracking.

After an ordinary click, exit Grid mode and allow the existing focus policy to observe the result. A click on an editable control can lead to Insert mode through that policy; a click elsewhere does not imply that the browser should enter PassThrough mode. Do not use an unconditional delayed `forceActiveFocus()` that could steal focus from a newly opened prompt.

## 8. Native pointer delivery: the proof-of-concept gate

### Why DOM activation is insufficient

The current hint helper calls `el.click()` on a retained element. A coordinate mode must also work without a discoverable element, including locations within a canvas. It must not be implemented as `elementFromPoint(...).click()` and then described as equivalent to a pointer. [R5][R6]

The HTML standard treats user activation as dependent on particular trusted input events. Calling a DOM method, constructing a JavaScript event, or setting a source enum in Rust is not proof that the engine grants the required activation. [E5]

### Preferred investigation route

Prototype a narrow native input adapter using public Qt APIs, with a view-local point mapped into the correct owning Qt Quick window/scene. Qt exposes mouse-event construction and application event delivery, but those APIs alone do not establish the correct receiving object or WebEngine behaviour in Ferric. [E4][E6][E7]

Verify the event-routing path through Ferric's actual composition, including the WebEngine render item, overlays, clipping, focus ownership, and any separate fullscreen surface. Do not assume that sending an event directly to the outer QML `WebEngineView` is sufficient.

A small C++ bridge is acceptable where public Qt APIs require it. Keep selection policy in Rust. Do not add a private Chromium hook, DevTools protocol endpoint, Qt private-header dependency, OS-wide injection daemon, or production QtTest dependency to avoid proving the proper route.

The production boundary, if the gate passes, is one QML-registered C++ type named `FerricPagePointerAdapter` in `browser_page_pointer_adapter.h/.cpp`, added to the existing `CxxQtBuilder::cpp_files` list. It owns guarded `QPointer`s to the exact target `QQuickItem` and `QQuickWindow`. `surfaceSerial` and `surfaceRevision` are nonzero and monotonic for the adapter lifetime: a target change/destruction advances both, while any other geometry/authority invalidation advances the revision. It exposes only:

- an `enabled` property, target-item/window properties, a trusted browser-owned `inputBlocked` property, and read-only `surfaceSerial`/`surfaceRevision`;
- `dispatch(requestId, sessionId, surfaceSerial, surfaceRevision, x, y, action)`;
- an acknowledgement signal carrying the same request/session IDs and a bounded outcome code;
- invalidation and fresh-physical-pointer signals carrying bounded reason codes.

`enabled` is true only while the owning Rust session is active. Disabling clears adapter-local pending/consumed-request bookkeeping after completing any already-issued release cleanup. `inputBlocked` is driven only by Ferric's consolidated browser-owned prompt/internal-surface predicate; changing it to true increments the revision and invalidates the session. Do not expose a generic “send arbitrary mouse event” API to IPC, JavaScript, or unrelated QML. The exact internal Qt receiver and event-delivery call remain the output of Slice 0 and must be documented beside this interface before production code lands.

### Dispatch transaction

A request includes the session identifier, captured target, owning view identity, surface revision, selection revision, exact point, action kind, and a one-shot request ID.

Before dispatch, the GUI-thread adapter independently confirms that the originating view is still live, active, visible, unobstructed by browser-owned modal surfaces, and matches the captured identity/revision. Confirm finite in-bounds geometry and local interactive authority. Reject stale requests; never remap them to a replacement view.

The overlay may be visually present but must not become the input receiver. Resolve presentation/input ownership deterministically, not with an arbitrary timer. If the latest selection projection is not ready, hold only a bounded pending commit tied to that exact session/revision, and cancel it on invalidation.

For clicking, the qualified backend should perform the appropriate pointer positioning and button press/release sequence. Include correct button state, event coordinates, timestamps, and neutral modifiers for the default bindings. Test event order rather than assuming it.

The desired sequence is: hover emits one no-button move; click emits a no-button move at the point, then one press whose `button` and `buttons` contain the selected button, then one release whose `button` is selected and whose `buttons` is empty. Modifiers are empty. Scene and global positions are derived with Qt mapping APIs from the same view-local point; the OS cursor is not warped. Slice 0 may refine this sequence only when recorded Qt/WebEngine evidence requires it, and the final sequence must be written into the native-input evidence before Slice 2.

Maintain release/cleanup responsibility once a press has been issued. Handle reentrancy or destruction without redirecting release to a different window/document. No held-button state may survive to await the user's next grid key in v0.1.

Consume each request at most once. A dispatch acknowledgement means the event was delivered or rejected; it does not prove that the intended page operation succeeded. Do not automatically retry after a timeout or absent visible page response: the first click may already have acted.

The adapter accepts only the action strings `hover`, `left`, `right`, and `middle`; Rust uses a closed enum and QML performs no string construction beyond projecting that enum. A malformed action, non-finite coordinate, point outside `[0,width) × [0,height)`, mismatched serial/revision, inactive window, hidden/inactive view, browser-owned prompt, attached DevTools surface, or destroyed target is a terminal rejection.

At most one request may be pending per session. Rust owns the pending record. The adapter keeps a bounded record of consumed request IDs for the live session so a duplicate QML signal or acknowledgement cannot redispatch. Once it issues a press, cleanup of the matching release is adapter-owned even if the target is destroyed reentrantly; the release must never be redirected to a replacement target.

### Hover and race limitations

Some controls appear or change on hover. An explicit hover operation gives the user a chance to inspect them before clicking. For the click path, movement may itself cause page changes; there is no universal atomic “move, inspect, and click the same intended element” guarantee for raw coordinates.

Do not insert a long automatic hover delay and assume that makes clicking safer. Test representative controls and retain the explicit two-step hover workflow for cases that need it.

### Gate acceptance

Before proceeding, demonstrate on Ferric's actual supported Qt version and native desktop environment:

1. Correct left/right/middle button delivery at known points, including canvas coordinates.
2. Correct handling of a cross-origin frame under engine hit testing, without DOM-origin bypasses.
3. Pointer event ordering and observed trusted/activation behaviour in page event handlers.
4. Expected behaviour for file inputs, popup requests, and fullscreen under unchanged browser policies.
5. Correct coordinate mapping under page zoom, scale factors, view offsets, and browser-window fullscreen; clean rejection in page-requested content fullscreen.
6. No activation of browser chrome, another window/tab, or a prompt following a race.
7. No click duplication, missing-release state, key leakage, or inadvertent OS cursor warp.

Record unsupported cases honestly. A feature may need a narrower release scope, or the backend design may need revision. Do not silently substitute JavaScript activation or disable browser protections to make a test appear successful.

## 9. Rust and Qt architecture

### State machine

The v0.1 lifecycle is:

```text
Inactive
  → AwaitingSurface
  → Selecting
  → DispatchPending
      → Inactive                  (click acknowledged, or any dispatch rejected)
      → Selecting at root         (hover delivered and the same surface remains valid)

Any active state → Inactive       (cancel, loss of authority, invalidation)
```

Refine, Backspace, reset, and help are accepted only in `Selecting`. Commit is unavailable before valid surface geometry has arrived. All input except cancellation is consumed but ignored in `AwaitingSurface` and `DispatchPending`. Late replies for an old session/request are ignored and cannot change the newer session.

Only `Selecting` has a visible overlay. `AwaitingSurface` may exist for at most one Qt event-loop turn; one session-ID-bound queued callback cancels it with `surface-unavailable` if QML did not bind a valid active surface. There is no timer-based retry. `DispatchPending` has a Rust-owned two-second deadline checked through the existing runtime-work scheduler, not an overlay timer. Expiry only releases local session/UI state; it never retries the native action and is reported as `dispatch-uncertain` rather than “not clicked.”

### Required core vocabulary

Add `crates/ferric-browser-core/src/spatial_navigation.rs` with closed, Qt-independent types. Exact field visibility is an implementation detail, but the following semantics and names are fixed:

```text
LogicalPoint { x: f64, y: f64 }
LogicalRect { x: f64, y: f64, width: f64, height: f64 }
GridCell = One | Two | Three | Four | Five | Six | Seven | Eight | Nine
PointerButton = Left | Right | Middle
SpatialAction = Hover | Click(PointerButton)
SpatialLifecycle = AwaitingSurface | Selecting | DispatchPending
SpatialCancelReason = User | SurfaceUnavailable | TargetChanged | ViewChanged |
                      GeometryChanged | WindowInactive | PromptOwnedInput |
                      RendererUnavailable | PhysicalPointer | DispatchRejected |
                      DispatchUncertain
SpatialSessionId / SpatialRequestId = opaque, monotonically allocated IDs
SurfaceStamp { serial: u64, revision: u64 }
SpatialTarget { window: WindowId, target: Target }
SpatialOwner { target: SpatialTarget, surface: SurfaceStamp }
SpatialProjection { session_id, selection_revision, lifecycle, root_rect,
                    current_rect, crosshair, depth, help_visible }
SpatialDispatchRequest { request_id, session_id, owner, selection_revision,
                         point, action }
```

`LogicalRect::new` rejects non-finite coordinates, non-positive extents, and values whose derived edges are non-finite. A point is dispatchable only when `x >= 0`, `y >= 0`, `x < root.width`, and `y < root.height` in view-local coordinates. Geometry operations retain `f64` values without per-level integer rounding.

`SpatialSession` is an opaque state machine in the same module. `AwaitingSurface` owns a `SpatialTarget` but no geometry; after a valid surface observation it owns the `SpatialOwner`, root/current rectangle, at most eight parent rectangles, depth, selection revision, help flag, and optional single pending request. A `SpatialProjection` exists only after valid surface geometry is installed. The selection revision starts at one and uses checked increments; overflow fails closed by cancelling the session. Session methods return typed transitions; callers cannot directly mutate rectangles or pending IDs. `refine` increments the selection revision exactly once when it changes geometry. Back/reset/help similarly produce a new projection revision only when visible state changes. No selection-only method can construct a dispatch request.

The root rectangle is always `(0, 0, view.width, view.height)` in target-`WebEngineView` local logical coordinates. The overlay maps that rectangle into its own item coordinates for drawing; it does not feed mapped overlay coordinates back into Rust or the native adapter.

### Required ownership and boundary

Use this ownership model:

```text
ApplicationState / reducer
  owns Mode::Grid in the window mode stack and Target freshness
                 │
BrowserUiRust (one owning browser window)
  owns Option<SpatialSession>, ID allocation, provenance checks,
  dispatch-pending bookkeeping, and QML projection properties
                 │
QML runtime
  identifies the exact active WebEngineView, maps presentation geometry,
  renders the passive projection, and forwards bounded host observations
                 │
FerricPagePointerAdapter (GUI thread)
  owns QPointer identity/revision, last-moment validity checks,
  physical-pointer observation, native delivery, and acknowledgements
```

The session is deliberately not stored in durable `ApplicationState`: it contains an adapter-owned live view stamp and is never restorable. This follows the existing `HintSession`/`BrowserUiRust` pattern while keeping the browser mode itself reducer-owned. QML must not own a second rectangle history, pending request, or freshness boolean.

Entry and exit helpers must keep the two owners coherent:

- `begin_spatial_navigation` first validates local authority and captures the live `Target`, then pushes `Mode::Grid`, installs `Some(SpatialSession::AwaitingSurface)`, updates the binding resolver, and publishes mode/projection. Any failure unwinds to Normal with no session.
- `cancel_spatial_navigation(reason)` clears the session and projection, applies the reducer's `Event::Escape` when Grid is still active so both Normal- and Command-origin entry return to Normal, resets the resolver, and publishes one bounded status message. It is idempotent.
- generic `escape()` calls the Grid cancellation helper before normal cleanup;
- every path that changes `core_mode` away from Grid clears the session first;
- debug assertions and tests enforce `core_mode == Mode::Grid` if and only if a live spatial session exists after a public method returns.

### Repository integration points

**Core:** add the module above; add `Mode::Grid`; add spatial session/request IDs in `id.rs`; export only the required types from `lib.rs`. Do not reuse `HintSessionId`.

**Reducer:** continue to own only the mode stack and tab/document freshness. It does not receive Qt geometry or view pointers.

**Input/command:** register the exact command vocabulary from Section 5, add the default bindings, and extend every exhaustive mode-name projection (`binding_policy.rs`, binding help, status presentation, and relevant tests). Counts are unsupported for every Grid command.

**Browser UI Rust:** add `browser_ui_spatial_navigation.rs` for session coordination. The existing `BrowserApplication::capture_target` supplies the document target. Dispatch always uses the owner stored in the session; there is no “look up the current active tab later” shortcut.

**Qt adapter:** add `FerricPagePointerAdapter` as specified above. Do not broaden `FerricBrowserKeyRouter` into a generic pointer injector; extend it only for press/release ownership and auto-repeat metadata.

**QML:** add `FerricSpatialGridOverlay.qml`, anchored over `webViews`, and bind the pointer adapter to `window.activeWebView()`. QML calls explicit Rust invokables for surface-ready, invalidation, and acknowledgement facts. JSON is not required for the hot projection: prefer typed scalar properties or fixed parallel lists consistent with the existing CXX-Qt boundary.

Extend the existing focus/overlay arbitration so the new overlay does not fight command entry, prompts, or hint mode. A rendering-only overlay is different from an input-blocking browser modal: distinguish those roles explicitly.

No additional workspace crate is warranted solely for this feature unless implementation reveals a real reuse boundary.

### Projection and acknowledgement contract

Expose only the data QML renders: visibility, lifecycle name, session/revision IDs, current/root rectangle scalars, crosshair scalars, depth/path, cell labels, and help visibility. Every `u64` identity or revision that crosses JavaScript/QML is a canonical nonzero decimal string of at most 20 ASCII digits; never round-trip it through a JavaScript `Number`. Geometry remains finite `double` values. Rust formats the status/path text; QML must not reconstruct semantic state from it.

To avoid re-entering a CXX-Qt Rust invokable while it is publishing a request, QML copies the immutable request projection and schedules exactly one `Qt.callLater` dispatch. At the callback it first confirms that the currently projected session/request/revisions still equal the copy, then calls the native adapter once. A mismatch drops the callback. This is a one-shot reentrancy boundary, not a retry or timing-based freshness mechanism; the native adapter still performs all last-moment checks.

The QML-to-Rust acknowledgement contains `session_id`, `request_id`, `surface_serial`, `surface_revision`, and one of `delivered`, `rejected-stale`, `rejected-unavailable`, `rejected-invalid`, or `uncertain`. Unknown values fail closed. Rust accepts it only when every identity matches the single pending request. A delivered hover also requires the current adapter stamp to equal the session owner before returning to root selection.

## 10. Provenance, macros, privacy, and policy

The current command system already distinguishes command sources, but a source label is not itself an unforgeable grant of authority. Validate the actual ingress path and current local interaction/session, rather than accepting a caller-supplied string claiming to be a keyboard. [R3]

The v0.1 ingress matrix is:

| Ingress | `grid` entry | `grid-*` action |
|---|---:|---:|
| Accepted `FerricBrowserKeyRouter` binding in the focused owning window | allow | allow |
| Direct submission from that window's visible command bar | allow | not reachable while Grid owns the mode |
| Browser-owned clickable UI/action registry | deny; no action is published | deny |
| IPC or forwarded CLI command | deny | deny |
| Userscript | deny | deny |
| Switcher | deny | deny |
| Macro record/playback | deny and abort recording/playback cleanly | deny and abort |
| Generic repeat | deny | deny |

Enforce this at each ingress before calling the common executor, and recheck the focused window/current live session immediately before creating a native request. Do not add a boolean or string argument that lets a caller claim local authority.

For v0.1, reject grid entry during macro recording/playback and reject pointer-action commands from repeat, IPC, CLI automation, and userscripts. Keep ordinary local command-bar entry available. Do not record a coordinate action and silently drop it from a macro: later recorded commands could then operate on the wrong page state.

A coordinate path such as `826` is not a stable locator across viewport sizes, scrolling, navigation, or layout changes. Future replay would require an explicit, separately reviewed feature, not accidental inheritance from generic repeat behaviour.

Grid is not a permission system and must not override Ferric's existing rules for external protocols, popup handling, downloads, fullscreen, clipboard, or other protected features. It may cause the same request an ordinary user pointer action causes; the normal policy still adjudicates that request.

Cross-origin point input, once qualified, relies on normal engine hit testing. It is not permission to inspect or bypass a frame's DOM isolation.

Do not persist sessions or coordinate histories in session restore. Avoid logging page text, form values, screenshots, or precise action trails. Development diagnostics can record bounded technical information such as session IDs, invalidation reasons, and dispatch outcomes; keep private/transient browsing rules intact.

## 11. Performance and accessibility

The intended incremental cost is bounded geometry and a small browser-owned overlay. The raw mode requires no candidate collection, DOM scans, OCR, AI, screenshot capture, or periodic JavaScript polling.

Bound refinement history to eight entries and keep the visible overlay to the current cells, outline, crosshair, and compact status/legend. Inactive mode should have no dedicated polling timer. Redraw the overlay only when its state or owning presentation changes; the underlying page may of course still render independently.

Do not claim a measured battery or memory improvement. Measure added idle CPU, input-to-overlay latency, repaint activity, and allocations on the lower-spec target hardware before making performance claims.

Grid requires visual spatial judgement and is not, by itself, a substitute for semantic accessibility. Provide an accessible mode name, readable instructions, user-scalable contrast/labels, and a predictable cancellation path. Avoid repeatedly announcing all nine cells or stealing assistive-technology focus on each refinement. Preserve normal tab navigation and semantic hints.

## 12. Verification plan

### Pure core tests

Verify exact child-region relationships, finite geometry, containment of the crosshair, correct row-major mapping for every digit, and no unintended gaps caused by integer rounding. Cover unusual aspect ratios, tiny views, fractional sizes, invalid inputs, and the depth stop.

Backspace must restore the previous rectangle; reset must restore the original root. Digit actions must never emit pointer effects. A depth limit must never activate anything.

State-machine tests should cover invalidation in every active state, duplicate commit requests, stale replies, missing surfaces, destroyed/recreated views, and mode/session consistency. Property tests should generate action sequences and assert these invariants.

### Input and ownership tests

Verify that digits select cells rather than building counts; key prefixes are reset on entry; Enter/Backspace/Space/Escape never reach page content; releases do not leak after focus changes; auto-repeat does not click or refine repeatedly; independent repeated digits still work.

Test Num Lock, supported layouts, modifier normalisation, focus handover, pending IME/composition state, physical pointer takeover, and conflicts with user overrides. Confirm that a browser prompt appearing between selection and dispatch wins input ownership.

### Native fixture pages

Extend `crates/xtask/src/fixture_server.rs` with a bounded `/spatial-grid` route and a distinct-origin `/spatial-grid-frame` route. The main fixture includes static controls, a canvas that reports exact pointer coordinates, same-origin and cross-origin frames, open and closed shadow DOM, a hover menu, a text field, a file input, a video/fullscreen control, a popup button, and a nested scrolling panel. It records a bounded in-page event list containing only fixture element IDs, event type/order, local/client/screen coordinates, button/buttons/modifiers, `isTrusted`, and user-activation flags; it contains no general inspection endpoint.

Use live engine-level assertions: event coordinates and sequence, button/modifier state, `isTrusted`, and activation state inside handlers. Test normal policy outcomes, not just that an event callback ran.

Exercise resizing, sidebar/chrome changes, page zoom, device-scale changes, moving between displays, fullscreen transitions, renderer replacement, and two browser windows. Include content changes between aiming and committing and document the remaining raw-coordinate limitations.

The repository's existing offscreen smoke is explicitly not native input qualification. Keep native tests and their environment/results visible rather than inferring them from string assertions or a clean application launch. [R8]

### Usability evaluation

Compare hints, the grid, and the mouse on realistic tasks. Record completion time, errors, refinement depth, cancellations, and whether the user had to abandon the keyboard—not just key counts.

Include an ordinary link page where hints should remain preferable, a canvas/control case where hints do not expose the desired point, a thin timeline target, a hover menu, and a high-resolution viewport. Record number-row reach or layout complaints as evidence for a later preset proposal; do not add a second mapping during v0.1 qualification.

Success means a dependable way to complete previously awkward keyboard-only tasks. It does not require grid selection to beat direct element hints at ordinary links.

## 13. Delivery sequence

Each slice below should be one reviewable change with no opportunistic refactor. “Likely files” bounds discovery; if another file must change to keep an exhaustive match or generated contract correct, change it and record why in the slice evidence.

### Slice 0 — Native-input stop/go spike

**Goal:** determine the exact public-Qt route before committing to product architecture.

**Work:** build the smallest native-Wayland prototype in Ferric's real `WebEngineView` composition. Add the bounded `/spatial-grid` fixture described in Section 12 to `crates/xtask/src/fixture_server.rs`. Try event delivery only through public Qt APIs. Record the actual receiver object, coordinate transformations, event sequence, global-position treatment, cursor behaviour, and reentrancy/destruction handling. This spike may use temporary diagnostic code, but no diagnostic or generic injection endpoint remains enabled afterward.

**Required evidence:** create `docs/testing/spatial-grid-native-input.md` with the Ferric commit, dirty/clean state, OS/session, compositor, Qt/QtWebEngine/Chromium versions, scale factor, page zoom, commands used, and a capability matrix for hover/left/right/middle, canvas coordinates, cross-origin iframe hit testing, `PointerEvent.isTrusted`, transient user activation, popup/fullscreen/file-input policy, cursor warp, and press/release ordering. Distinguish automated assertions from manual observations.

**Exit:** all Section 8 gate items pass without a prohibited fallback, and the document names the exact event receiver/delivery API selected. If any required button cannot be made safe, either narrow this proposal explicitly and re-review the controls or stop. Do not proceed on the theory that later QML work will fix native routing.

**Checks:** `cargo build -p ferric-browser --locked`; the documented native-Wayland fixture run. Offscreen Qt tests are supplementary only.

### Slice 1 — Pure spatial model

**Goal:** land deterministic geometry and state transitions with no Qt or user-visible mode.

**Likely files:** `crates/ferric-browser-core/src/spatial_navigation.rs` (new), `id.rs`, `lib.rs`, and focused unit/property-style tests in the new module.

**Work:** implement the Section 9 core vocabulary, checked geometry construction, row-major subdivision, maximum depth/precision stop, exact Back/reset behaviour, projection revisions, one pending request, acknowledgement matching, and invalidation from every active state. Keep fields private enough that invalid states cannot be assembled outside the module.

**Exit:** tests cover all nine mappings; fractional/tiny/extreme valid rectangles; NaN/infinity/zero/negative rejection; eight-level history; refine-without-dispatch; duplicate commit; stale/wrong acknowledgement; hover reset; rejected/uncertain terminal outcomes; and arbitrary bounded action sequences preserving containment, finite coordinates, depth, and one-pending-request invariants.

**Checks:**

```sh
cargo fmt --all -- --check
cargo test -p ferric-browser-core spatial_navigation --locked --offline
cargo clippy -p ferric-browser-core --all-targets --locked --offline -- -D warnings
```

### Slice 2 — Narrow native page-pointer adapter

**Goal:** productionize only the qualified primitive, with no public Grid command.

**Likely files:** `browser_page_pointer_adapter.h/.cpp` (new), `crates/ferric-browser-engine-qt/build.rs`, a dedicated native test/fixture hook, and `src/tests/presentation_boundaries.rs` for structural assertions that remain useful.

**Work:** implement `FerricPagePointerAdapter`, guarded target/window ownership, monotonic surface stamps, bounded action parsing, finite/in-bounds validation, at-most-once dispatch, press/release cleanup, acknowledgements, and guarded physical-pointer observation. Carry forward the exact route proven in Slice 0; do not generalize it.

**Exit:** the adapter compiles into the QML module but has no remotely callable or user-visible route. Native evidence reconfirms the Slice 0 capability matrix through the production type. Destruction between press and release, duplicate request IDs, stale revisions, inactive windows, hidden views, and invalid points all fail safely.

**Checks:**

```sh
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked --offline
cargo build -p ferric-browser --locked
cargo xtask test engine
```

Repeat the native-Wayland cases recorded in Slice 0.

### Slice 3 — Rust session coordinator and lifecycle invalidation

**Goal:** connect the core session to one `BrowserUiRust` owner without exposing entry yet.

**Likely files:** `browser_ui_spatial_navigation.rs` (new), `lib.rs`, `browser_ui_controls.rs`, `browser_ui_tab_state.rs`, `browser_ui_navigation.rs`, focused Qt-crate tests, and only the QML callbacks needed to report host invalidation facts.

**Work:** add session/ID fields, typed projection properties, surface-ready/invalidate/acknowledge invokables, idempotent cancellation, and the two-second uncertain watchdog. Hook all already-available active-tab, navigation, renderer, view-close, zoom, fullscreen, prompt, and window-activity observations. Keep dispatch disabled unless a live adapter stamp exactly matches the stored owner. Mode/session coherence is added in Slice 5, when `Mode::Grid` first exists.

**Exit:** tests show that every known lifecycle callback cancels `AwaitingSurface`, `Selecting`, and `DispatchPending`; stale callbacks cannot cancel or revive a new session; acknowledgement mismatch is ignored; and cancellation never queues a later action. No public command can create a session yet.

**Checks:** focused engine tests, then `cargo test -p ferric-browser-engine-qt --locked --offline` and `cargo clippy -p ferric-browser-engine-qt --all-targets --locked --offline -- -D warnings`.

### Slice 4 — Passive overlay and exact view binding

**Goal:** render the Rust projection over the owning view without adding input ownership.

**Likely files:** `qml/components/FerricSpatialGridOverlay.qml` (new), `FerricBrowserRuntimeChrome.qml`, `FerricBrowserRuntimeRequests.qml`, `FerricBrowserRuntimeSurface.qml`, `presentation_boundaries.rs`, and engine/QML tests.

**Work:** instantiate the pointer adapter against `window.activeWebView()`, provide root geometry/stamps to Rust, map view-local projection coordinates into the overlay, and render the current 3×3 rectangle, outer boundary, labels, crosshair gap, status, and optional help. Cancel on mapping/visibility/geometry/prompt changes. The overlay must remain passive and inactive when no `Selecting` projection exists.

**Exit:** QML tests cover 3×3 placement, narrow-label fallback, scaling, visibility, and accessible status. Structural tests reject `focus`, `MouseArea`, `TapHandler`, independent subdivision math, timers, or pointer dispatch inside the overlay. A manual visual smoke confirms that the drawn crosshair and fixture-reported native coordinates agree at normal and non-1× scale.

**Checks:** `cargo test -p ferric-browser-engine-qt --locked --offline`, `cargo xtask check`, `cargo xtask test engine`, and the recorded visual smoke.

### Slice 5 — Mode, commands, binding ownership, and local provenance

**Goal:** expose the complete v0.1 workflow only after the safe backend and lifecycle path exist.

**Likely files:** core `model.rs`, `command.rs`, `completion.rs`, `input.rs`; Qt `binding_policy.rs`, `binding_presentation.rs`, `browser_ui_command_dispatch.rs`, `browser_ui_input.rs`, `browser_ui_ipc.rs`, `macro_policy.rs`, `browser_key_router.h/.cpp`, relevant `lib.rs`/QML mode matches, and focused command/input tests.

**Work:** add `Mode::Grid`, the exact commands/default bindings from Section 5, the `; g` entry, canonical Grid key tokens, auto-repeat suppression, accepted-release ownership, and mode/session atomicity. Route `grid` and `grid-*` through the existing full-command-executor classification to dedicated `BrowserUi` methods; the generic core dispatcher must not treat an unsupported Grid command as successfully executed. Wire those methods to the Rust session coordinator and pending native request projection. Explicitly reject entry/actions through IPC, CLI forwarding, userscripts, switcher/action discovery, macro record/playback, and repeat. Add Grid to binding configuration/help projections so user overrides still pass the shared registry; do not add a second QML keymap.

**Exit:** end-to-end native fixture tests cover refine/back/reset/help/cancel, left/right/middle/hover, no input during aiming, exact one-shot dispatch, hover-to-root, and post-click focus policy. Input tests cover held digits/Enter, releases after mode exit, unsupported modifiers, IME-like input, Num Lock variants, user binding conflicts, and pending Normal prefix/count reset. Source tests prove every non-local ingress fails without changing mode or producing a request.

**Checks:**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo xtask check
cargo xtask test engine
cargo xtask test wayland
```

### Slice 6 — Requirement records, user documentation, and release qualification

**Goal:** make support claims match measured behaviour.

**Likely files:** `docs/requirements.csv`, `docs/DEVELOPMENT_SPEC.md`, a new accepted ADR for the native spatial-input boundary, `docs/user/commands.md`, `docs/user/bindings.md`, `docs/user/configuration.md` only if binding examples require it, fixture files, and `docs/testing/spatial-grid-native-input.md`.

**Work:** allocate `SPATIAL-*` requirement IDs, map every acceptance invariant to code/tests/evidence, document raw-coordinate limitations and cancellation reasons, and add the final capability/environment matrix. Record unsupported environments or controls rather than generalising results from one setup. Run the usability cases from Section 12; this can generate follow-up work but does not silently expand v0.1.

**Exit:** the repository gate is green; native evidence covers the supported environment matrix; every `GRID-*` invariant has a test or named manual observation; docs do not promise pixel stability, semantic target identity, macro replay, drag, or desktop control.

### Post-v0.1 work — separate proposals

Arrow-key adjustment, numpad/letter presets, regional hint handoff, semantic assistance, double click, and dragging are not backlog items inside these slices. Each changes the interaction or safety contract and needs a separate accepted proposal. In particular, dragging requires button ownership, capture, paths, thresholds, cancellation/release, and focus-loss semantics; Escape cannot undo side effects already caused before release.

## 14. Acceptance invariants

| ID | Requirement | Primary slice |
|---|---|---:|
| GRID-001 | Refinement never sends page input or clicks automatically | 1, 5 |
| GRID-002 | A click addresses the displayed crosshair, not a guessed DOM target | 1, 2, 5 |
| GRID-003 | Every action remains bound to its original live window/tab/document/view | 2, 3 |
| GRID-004 | Known invalidation rejects pending actions instead of retargeting them | 2, 3 |
| GRID-005 | Grid key presses, repeats, and corresponding releases do not leak to the page | 5 |
| GRID-006 | The overlay does not steal or intercept its own dispatched pointer action | 2, 4 |
| GRID-007 | Each commit is delivered at most once; no automatic retry after uncertain outcomes | 1–3 |
| GRID-008 | Every issued button press has bounded cleanup responsibility; no user-paced held-button state in v0.1 | 0, 2 |
| GRID-009 | Native input capability is demonstrated, not inferred from DOM activation or offscreen smoke | 0, 2, 6 |
| GRID-010 | Engine security, browser policy, and cross-origin DOM isolation remain intact | 0, 2, 6 |
| GRID-011 | Grid sessions and coordinate histories are ephemeral and not silently replayable | 1, 3, 5 |
| GRID-012 | The UI makes no promise of stable element identity on a changing page | 4, 6 |
| GRID-013 | `Mode::Grid` and the live Rust session cannot outlive one another after a public transition | 3, 5 |
| GRID-014 | Non-local command sources cannot enter Grid or create/refine/commit a session | 5 |
| GRID-015 | A stale, malformed, duplicate, or late acknowledgement cannot dispatch or mutate a newer session | 1–3 |
| GRID-016 | Inactive Grid mode has no polling timer, live adapter request, or retained page-derived data | 3, 4 |

## 15. Evidence gates and escalation policy

There are no discretionary product decisions a slice author must invent. The remaining unknowns are empirical:

1. the exact public-Qt receiver and delivery API that reaches Qt WebEngine correctly;
2. whether all four v0.1 actions preserve expected event ordering, trust/activation, policy, coordinates, and cleanup;
3. which native event attributes safely distinguish fresh physical takeover from the adapter's own events; and
4. whether the passive overlay remains aligned across the supported scale/fullscreen/window matrix.

Slice 0 resolves items 1–3 or stops the implementation. Slices 2, 4, and 6 reconfirm them in production form and resolve item 4. If evidence contradicts a fixed contract, the implementing agent must stop that slice, append the observed facts to `docs/testing/spatial-grid-native-input.md`, and request design review. It must not broaden APIs, weaken validation, silently remove a promised control, add a DOM fallback, or mark the slice complete.

The release decision turns on native input correctness and real task completion. The general desirability of keyboard-addressable points is not a substitute for those results.

---

## Sources

Repository links are relative to this proposal and refer to the reviewed local worktree. External documentation was consulted on 26 September 2026; its rolling version is not necessarily Ferric's deployed Qt version.

[R1]: crates/ferric-browser-core/src/model.rs
[R2]: crates/ferric-browser-core/src/input.rs
[R3]: crates/ferric-browser-core/src/dispatcher.rs
[R4]: crates/ferric-browser-engine-qt/qml/components/FerricHintOverlay.qml
[R5]: crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js
[R6]: crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimePresentation.qml
[R7]: docs/architecture/ADR-0007-hint-geometry-and-activation.md
[R8]: docs/testing/m0-163-ui-automation-boundary.md
[R9]: docs/DEVELOPMENT_SPEC.md
[R10]: docs/user/bindings.md
[R11]: crates/ferric-browser-engine-qt/src/browser_key_router.cpp
[R12]: crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeChrome.qml
[E1]: https://support.microsoft.com/en-us/accessibility/windows/voice-access/use-the-mouse-with-voice
[E2]: https://github.com/rvaiya/warpd
[E3]: https://doc.qt.io/qt-6/highdpi.html
[E4]: https://doc.qt.io/qt-6/qmouseevent.html
[E5]: https://html.spec.whatwg.org/multipage/interaction.html#tracking-user-activation
[E6]: https://doc.qt.io/qt-6/qcoreapplication.html#sendEvent
[E7]: https://doc.qt.io/qt-6/qquickitem.html#mapToScene

Repository references: [model][R1], [input][R2], [dispatcher][R3], [hint overlay][R4], [page scripts][R5], [runtime presentation][R6], [hint ADR][R7], [native-testing boundary][R8], [development specification][R9], [default bindings][R10], [native key router][R11], [WebEngine view composition][R12].

External references: [Windows Voice Access][E1], [warpd][E2], [Qt high-DPI model][E3], [QMouseEvent][E4], [HTML user activation][E5], [Qt event delivery][E6], [Qt item mapping][E7].
