# M0-78 — Typed command actions

## Scope

Registered command switcher rows now use stable command-subject actions for
help and execution.

## Implemented behavior

- `browser.command.help` and `browser.command.execute` are registry-backed
  namespaced actions.
- The action carries a bounded stable command ID, never command text.
- Help resolves to the existing native help surface.
- Execute resolves the captured command definition and preserves command
  validation and mode checks. Argument-bearing invocations use the bounded
  typed-object path documented in M0-81; native switcher rows remain
  zero-argument because they do not capture command arguments.
- Switcher command rows use the same typed action boundary.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```

See `docs/testing/m0-81-action-command-arguments.md` for the argument-bearing
typed IPC extension and `docs/testing/m0-79-action-capability-availability.md`
for action capability metadata.
