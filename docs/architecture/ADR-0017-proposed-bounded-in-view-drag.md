# ADR-0017: Proposed bounded in-view pointer gesture

## Status

Proposed, pending native Wayland qualification. No production drag action or
user-facing UI is enabled by this decision record.

## Context

Ferric's native pointer adapter currently dispatches discrete pointer actions.
A held press/move/release gesture needs a bounded owner, deterministic endpoint
selection, a cancellation policy, and an explicit account of page effects.
Qt Quick owns hit testing for the existing public `QQuickWindow` event path;
the event is not sent directly to a guessed WebEngine child.

The S20 test-only fixture exercised that public Qt path on Qt 6.11.2 and
QtWebEngine 6.11.2. It observed trusted pointer and mouse events in an ordinary
pointer-driven widget, Pointer Events capture across off-element movement, a
canvas-like pointer surface, and a native range slider. It also exercised
Escape, input blocking, window deactivation, a 1-second watchdog, and original
view destruction while a button was held.

## Proposed decision

If the required native Wayland evidence later passes, Ferric may implement only
this explicitly selected in-view subset:

- one active gesture per owning browser window;
- left button, no modifiers, same live view and window;
- both start and destination points selected and revalidated before press;
- a straight path of 12 steps over 200 ms, with a 1,000 ms absolute watchdog;
- native adapter-owned clocking, held-button state, cancellation, and one
  terminal outcome;
- on cancellation, send at most one release to the still-live original owner;
  if that owner is gone, clear native bookkeeping and send no event to a
  replacement.

This subset is intended for pointer-driven widgets, explicit Pointer Events
capture, canvas-like controls, and native range sliders only to the extent each
category is qualified. It does not cover native HTML drag-and-drop, cross-window
or desktop/file drag, modifiers, edge scrolling, or a button held while the
user chooses a destination. It does not claim exact application-level success.

## Effects and cancellation

The offscreen fixture changed its range value from 10 to 94 while the pointer
was held. Escape then delivered one trusted pointer-up and mouse-up to the live
view, and the value remained 94. Cancellation therefore stops further native
input; it cannot undo events or state changes already accepted by a page. Ferric
must not retry an uncertain gesture or present cleanup as rollback.

The HTML draggable fixture emitted `dragstart`, `pointercancel`, and `dragend`,
but no `drop` and no local drop commit. Native HTML drag-and-drop remains outside
this proposal even though the same mouse path can enter its event machinery.

When the original `QQuickView` was destroyed, the test's `QPointer` owner
references cleared. The test-only transaction cleared its held state without a
release; a separately created replacement view observed zero DOM events. The
production adapter still needs a held-gesture implementation that owns these
checks and stays bound to the original surface. This fixture is feasibility
evidence, not that implementation.

## Qualification gate

S20's native Qt offscreen probe passed, but native Wayland physical pointer
input has not been qualified. A nested Weston startup smoke does not count as
pointer evidence. Until the existing Wayland harness can send real pointer
input and repeat the ownership/cleanup checks, S20 remains environment-blocked,
S21 stays gated, and this ADR remains proposed.

No live website or account was used. The observations are limited to the
checked Qt/WebEngine build and deterministic local fixture.
