# M0-82 — Typed download control actions

## Scope

Download pause and resume are now represented in the shared action registry
and use the browser-owned typed action path from the native manager.

## Implemented behavior

- `browser.download.pause` and `browser.download.resume` carry a bounded,
  stable download ID and require durable download metadata.
- Typed command and action callers reuse the existing state-aware control
  boundary: pause is accepted only for in-progress downloads, and resume only
  for paused downloads. Unsupported engine state is reported as an explicit
  error.
- The native download manager maps its Pause/Resume control to the stable
  action ID, then drains the existing browser-owned engine request.
- Action discovery exposes both controls with examples and the durable-index
  capability requirement; private/ephemeral sessions remain unavailable.

## Qualification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
