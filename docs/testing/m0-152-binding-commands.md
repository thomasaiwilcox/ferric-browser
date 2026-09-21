# M0-152 — Binding list and explanation commands

Completed the documented binding discovery command surface.

- The command registry exposes `binding-list [--mode MODE]` and
  `binding-explain KEYCHAIN [--mode MODE]` with typed arguments.
- CLI and Qt typed IPC preserve mode and keychain values without shell parsing
  and reject invalid modes or missing keychains.
- Local commands open the existing searchable binding help surface; IPC
  returns the existing structured binding map or resolution explanation.

Evidence:

- `cargo fmt --all`
- `cargo test -p browser-core --locked --offline`: 59 passed
- `cargo test -p rustbrowser --locked --offline`: 43 passed
- `cargo test -p browser-engine-qt --locked --offline binding_commands_use_typed_mode_and_keychain_arguments`: 1 passed
