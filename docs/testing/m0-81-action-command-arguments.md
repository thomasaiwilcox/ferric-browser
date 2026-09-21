# M0-81 — Argument-bearing command actions

## Scope

`browser.command.execute` now supports typed arguments for registered
commands through the IPC action boundary.

## Implemented behavior

- The action accepts a stable command action ID and an optional bounded JSON
  object in `arguments.arguments`.
- The object is serialized as one data argument and converted by
  `typed_ipc_command`, which applies each registered command's existing schema
  and validation rules.
- The nested command is executed through the existing IPC or interactive
  command path, preserving mode, privacy, target, capability, and executor
  checks. Its result is returned under `result` for IPC callers.
- Recursive `action`, `command-help`, and `command-execute` invocation is
  rejected, and malformed/non-object or oversized argument payloads fail
  before execution.
- Native switcher and command-line action forms remain zero-argument because
  those surfaces do not capture a typed argument object.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```
