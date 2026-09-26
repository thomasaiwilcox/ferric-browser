# ADR-0013: View-bound spatial Grid navigation

## Status

Accepted for the v0.1 implementation boundary; native desktop qualification is
tracked separately in `docs/testing/spatial-grid-native-input.md`.

## Decision

Ferric implements keyboard-controlled spatial navigation as an ephemeral
`SpatialSession` owned by `BrowserUiRust`. The session captures the active
window/tab/document and a monotonic native surface stamp, then subdivides the
active `WebEngineView`'s logical local rectangle into a row-major 3×3 grid.
Rust owns the history, freshness checks, dispatch transaction, and lifecycle.
QML receives only a projection and displays a passive overlay. A registered
`FerricPagePointerAdapter` is the sole pointer-delivery boundary and accepts
only bounded hover/left/right/middle requests from the owning UI instance.

Grid commands are local-interactive commands. Keyboard bindings and the direct
visible command bar may enter or operate the mode; IPC, CLI, userscripts,
macros, repeat, and switcher/action discovery cannot inject coordinates or
commit a point. DOM activation, JavaScript-created events, DevTools, Qt private
APIs, and OS-wide injection are not fallback implementations.

## Consequences

- The feature is safe to cancel when a tab, document, view, window, geometry,
  prompt, renderer, or physical pointer invalidates the captured surface.
- Raw coordinates do not provide element identity or exhaustive protection from
  in-place animation, inner scrolling, or canvas redraws; documentation states
  that limitation and offers explicit hover as a two-step workflow.
- Native qualification must demonstrate event ordering, button behaviour,
  cross-origin frame hit testing, scale/zoom mapping, release cleanup, and no
  chrome/prompt race before production enablement.
