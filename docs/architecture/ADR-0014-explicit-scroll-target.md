# ADR-0014: Document-local explicit scroll targets

- Status: accepted
- Date: 2026-09-27
- Scope: keyboard page scrolling, Hint selection, page-script state, and queued
  command routing

## Context

Modern pages commonly contain several independently scrollable regions. DOM
focus is a useful default signal, but it is not reliable authority for choosing
which region Ferric's keyboard scrolling commands should move. Users need an
explicit choice without changing page focus, retaining detached DOM trees, or
persisting selectors that may later identify a different element.

The choice also has to survive asynchronous Hint activation and command
dispatch while remaining local to the current document. A queued policy
command must not be redirected merely because a later command activates a
different tab before QML consumes it.

## Decision

1. Each live document has one of three scroll-target policies: `auto`,
   `document`, or `element`. `auto` keeps the existing focus-aware resolution;
   `document` explicitly owns the top-level document scroller; and `element`
   owns the exact region selected through the `scrollables` Hint family.
2. Policy state lives only in Qt WebEngine's isolated ApplicationWorld. It is
   not represented in Rust application state, IPC payloads, session snapshots,
   storage, or page-world JavaScript. Navigation, reload, renderer replacement,
   tab destruction, and session restoration therefore begin in `auto` by
   construction.
3. Element policy stores the selected element and its owner document through
   `WeakRef`. Ferric does not retain a selector, frame description, page text,
   offset, or other replacement identity. Engines without `WeakRef` reject
   element assignment and retain or restore `auto`; they do not fall back to a
   strong reference.
4. Selection does not call `focus()`, blur the current control, or synthesize
   wheel, key, pointer, or touch input. The Hint element map remains alive only
   until the asynchronous assignment callback has consumed the selected
   record, after which the map's strong references are cleared.
5. Before every operation, an element target is revalidated for exact document
   identity, same-origin frame reachability, composed visibility, hit testing,
   and scrollability on the requested axis. An invalid target is cleared and
   the same operation resolves once through `auto`. A valid target that does
   not support the requested axis remains selected and the operation is a
   no-op.
6. Generated policy, status, selection, and scroll scripts share one set of
   state and validation predicates. Unexpected DOM or security exceptions are
   caught at the script boundary and returned as bounded structured results.
7. Rust validates the closed `scroll-target` command grammar. Queued `auto`,
   `document`, and `status` actions carry the issuing stable `TabId`; QML
   resolves that ID to the owning view when consuming the action. The active
   tab at consumption time is not authority.

## Consequences

- Explicit ownership is independent of DOM focus while automatic scrolling
  remains compatible with focus-aware behavior.
- Detached, hidden, adopted, replaced-frame, or no-longer-scrollable targets
  cannot be reinterpreted as a different element.
- Same-document DOM updates may retain a valid selected element, but every
  operation still revalidates it. New document worlds reset the policy without
  native cleanup or persistence logic.
- Cross-origin frame contents remain outside the selection and validation
  boundary.
- Scroll scripts currently embed the shared helper runtime in each generated
  expression. Qualification found no responsiveness regression, but this
  remains a measurable high-frequency cost rather than a permanent constraint.

## Evidence

- `crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeServices.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`
- `crates/ferric-browser-engine-qt/src/browser_ui_browsing_commands.rs`
- `crates/ferric-browser-engine-qt/src/tests/command_protocol.rs`
- `docs/testing/explicit-scroll-target.md`

## Reconsideration trigger

Revisit this decision if ApplicationWorld loses the required document-local or
weak-reference behavior, if cross-origin target selection becomes a product
requirement, or if measured scrolling latency justifies installing a versioned
helper runtime once per document instead of embedding it in every expression.
