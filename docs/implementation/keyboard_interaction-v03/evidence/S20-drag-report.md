# S20 bounded in-view drag feasibility report

## Result

The Qt offscreen experiment demonstrates a feasible public `QQuickWindow`
press/move/release path for a limited set of in-view pointer-driven controls.
It does not qualify a production gesture. Native Wayland pointer evidence is
unavailable for native input. The capability decision remains unresolved until
that evidence level is available; missing platform evidence is not a technical
no-go. The machine-readable gate state is recorded in
`evidence/S20-drag-decision.json` as `blocked_environment`; it deliberately
does not claim either production go or a technical no-go.

## Offscreen observations

The `native_insert` fixture sent a left-button press, 12 straight-line moves
over 200 ms, and release through the owning `QQuickWindow`. The ordinary
pointer widget received trusted pointer and mouse events at the selected
coordinates. A widget calling `setPointerCapture()` received captured moves
outside its bounds and released capture at the end. A canvas-like surface
received pointer moves. A native range slider changed from 10 to 94.

The HTML draggable source began a trusted `dragstart`, then WebEngine emitted
`pointercancel` and `dragend`; the proposed drop target received no `drop` and
the fixture's drop-commit counter stayed at zero. This category is excluded
from the bounded pointer-driven subset.

Escape, native input blocking, window deactivation, and the 1,000 ms test
watchdog each cleared held-button bookkeeping and sent exactly one cleanup
pointer-up/mouse-up to the still-live original view. During the Escape case, the
range value had already changed; cleanup did not restore it. No retry occurred.
Destroying the original `QQuickView` cleared the helper's `QPointer`s and sent
no cleanup event. A replacement view then observed zero DOM events.

These behaviors belong to a disposable test helper in
`crates/ferric-browser-engine-qt/tests/native-insert/tst_native_insert.cpp`;
production `FerricPagePointerAdapter` remains a discrete-action adapter. The
experiment did not authorize a UI or prove behavior on arbitrary websites.

## Test evidence

- `T047/native-qt-offscreen` and `T049/native-qt-offscreen`: passed in
  `evidence/logs/S20/native-offscreen-verbose.log`.
- `T047/native-wayland` and `T049/native-wayland`: `not_run` in
  `evidence/logs/S20/wayland.log`. The nested Weston startup smoke passed, but
  `wtype` could not access the compositor's virtual-keyboard protocol, so no
  native pointer input was sent.
- The qualified command
  `CARGO_NET_OFFLINE=true QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask test engine`
  passed the Rust engine tests, all 144 Qt Quick/WebEngine tests, and native
  `native_insert` CTest 1/1. Its output is in
  `evidence/logs/S20/engine-qualified.log`.
- Two earlier invocations failed before providing the S20 native evidence:
  omitting `--expose-gc` made existing T044 report `gc_available=false`, and
  globally adding `--disable-gpu` changed the existing diagnostics test's
  expected GPU status. These invocation-specific failures are retained in
  `engine-initial.log` and `engine-disable-gpu-flag-failure.log`; the passing
  command keeps `--disable-gpu` scoped to native CTest as the repository does.

## Handoff

The candidate bounds and limitations are in
`docs/architecture/ADR-0017-proposed-bounded-in-view-drag.md`. S20 remains
`blocked_environment` unless native Wayland input can exercise the required
checks. S21 must stay gated until S20 reaches an evidence-backed `go`; no
HTML drag-and-drop capability is implied.
