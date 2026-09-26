# Spatial grid native-input qualification

This document records the production boundary for Grid mode. The browser
owns the session and `FerricPagePointerAdapter` owns the GUI-thread dispatch.
The adapter accepts only `hover`, `left`, `right`, and `middle`, validates the
captured surface stamp and view-local point, and sends a no-button move before
the optional press/release pair. It never calls page JavaScript and never
warps the OS cursor.

## Route under test

```text
BrowserUiRust spatial projection
  -> FerricPagePointerAdapter (exact active WebEngineView QQuickItem)
  -> application-synthesized QMouseEvent move/press/release via public QCoreApplication::sendEvent
  -> Qt Quick/WebEngine hit testing
```

The adapter labels these events with Qt's
`MouseEventSynthesizedByApplication` source. This is accurate source metadata;
it is not evidence that Chromium reports `PointerEvent.isTrusted` or grants
transient user activation. Those outcomes remain part of the native matrix
below.

The adapter's request and acknowledgement are one-shot and carry the session,
request, serial, revision, point, and bounded outcome. Geometry, target,
window, prompt, and pointer-takeover invalidations are terminal.

## Qualification checklist

Run the fixture server with `/spatial-grid` and capture a real desktop session
on each supported Qt/platform combination. Record the date, Qt version, window
scale, page zoom, and result for every row.

| Case | Required observation | Result |
|---|---|---|
| left button | one move, one press, one release; page report updates | pending desktop run |
| right button | context-menu path remains browser/page policy | pending desktop run |
| middle button | page receives middle button without forced tab policy | pending desktop run |
| canvas | coordinate arrives at the intended canvas point | pending desktop run |
| distinct-origin frame | hit testing reaches the frame without DOM helpers | pending desktop run |
| zoom and scale | view-local logical point remains aligned | pending desktop run |
| fullscreen/prompt race | stale requests reject; no chrome activation | pending desktop run |
| key ownership | auto-repeat is consumed; matching release never leaks | pending desktop run |
| cleanup | no duplicate click and no held-button state | pending desktop run |

Until this table is filled with real native evidence, Grid mode remains an
implementation feature behind the existing local build and must not be called
production-qualified. The fixture is intentionally bounded and does not
assert that an acknowledgement proves the page operation succeeded.

Automated evidence currently available: `cargo xtask test adapter` passes the
bounded offscreen Qt/QML adapter smoke, and `cargo xtask test wayland` passes
the nested native Wayland startup smoke. The latter reports interactive input
qualification as not-run when the compositor does not expose its virtual
keyboard protocol; that result does not fill any row in the matrix above.

## 2026-09-26 implementation audit

The post-implementation audit re-ran the workspace, QML, Clippy, and offscreen
adapter gates. It also added regression coverage for typed label projection,
session-bound surface callbacks, exact terminal acknowledgements, complete
key/pointer gesture ownership, stale target destruction, bounded request
deduplication, macro denial, and fail-closed surface-stamp exhaustion. These
automated checks harden the implementation but do not replace the pending
native desktop observations in the qualification table.
