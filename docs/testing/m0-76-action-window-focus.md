# M0-76 — Typed window focus

## Scope

Window switcher rows and the stable `browser.window.focus` action now share
one typed executor.

## Implemented behavior

- `window focus WINDOW_ID` maps to `browser.window.focus`.
- Typed IPC accepts the bounded stable window ID and rejects stale targets.
- Focusing a window also activates its current tab and synchronizes the Qt
  tab surface.
- Switcher window activation routes through the same action boundary.

## Qualification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```

Live multi-window Wayland focus qualification remains.
