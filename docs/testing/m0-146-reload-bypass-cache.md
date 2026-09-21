# M0-146 — Typed reload bypass-cache option

The shared `reload` command now supports the V1 option
`reload --bypass-cache`. The CLI and typed IPC surfaces expose the option as a
boolean `bypass_cache` field and reject non-boolean or unknown values before
constructing the parsed command.

The command dispatcher converts the validated option into the existing typed
`Event::Reload { bypass_cache }` event. The reducer preserves the flag in
`EngineEffect::Reload`, and the Qt adapter emits a bounded native action for
the live target. Primary and secondary WebEngine views call
`reloadAndBypassCache()` only when the flag is true; ordinary reload continues
to call `reload()`.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-core --locked --offline` — 57 passed
- `cargo test -p ferric-browser --locked --offline` — 40 passed
- `cargo test -p ferric-browser-engine-qt --locked --offline` — 111 passed
- `cargo xtask check --locked`
- Wayland startup smoke with `--temp-basedir` — expected timeout, no error,
  panic, failed, or assert diagnostics
