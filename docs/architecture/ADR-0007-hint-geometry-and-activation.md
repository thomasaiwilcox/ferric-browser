# ADR-0007: Validated native hint activation

- Status: accepted for the V1 interaction boundary
- Date: 2026-09-18
- Scope: page candidate collection, nested frames, overlays, and activation

## Decision

Page JavaScript may return only a bounded candidate description. Rust assigns
deterministic prefix-free labels and captures the tab, generation, document,
frame path, geometry, and metadata. Selection recollects and revalidates the
candidate before any navigation, focus, copy, download, external action, or
userscript invocation. Open shadow roots and bounded same-origin nested frames
are supported; cross-origin access remains denied.

QML renders a browser-owned non-focus-stealing overlay. The implementation
does not use Chromium remote debugging or an automation socket, and stale or
ambiguous geometry fails closed.

## Consequences and evidence

Hints remain useful for ordinary pages while preserving a clear trust boundary
between page-provided text/geometry and browser-owned effects. Zoom, device
scale, viewport movement, and native accessibility still require live
qualification.

- `crates/browser-core/src/hints.rs`
- `crates/browser-engine-qt/src/lib.rs`
- `crates/browser-engine-qt/qml/Main.qml`
- `docs/testing/m0-27-untrusted-inputs.md`
- `docs/testing/m0-163-ui-automation-boundary.md`

## Reconsideration trigger

Revisit only if a public Qt input API provides a stronger trusted activation
primitive; do not weaken candidate or generation validation to improve visual
success rates.
