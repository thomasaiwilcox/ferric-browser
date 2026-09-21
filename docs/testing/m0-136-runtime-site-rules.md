# M0-136 generated runtime site rules

Interactive configuration now supports the specification's generated
site-scoped layer:

```text
set --pattern https://docs.example/* input.entry_mode=insert
unset --pattern https://docs.example/* input.entry_mode
```

Rules are stored in the existing bounded `runtime-overrides.toml` document,
with atomic private writes for persistent changes and memory-only behavior for
temporary/private layers. Runtime rules use the validated site-setting key set,
receive generated highest precedence, and are applied to authored rules without
rewriting `config.toml`. Typed IPC and CLI forwarding preserve the pattern as
data; unsupported global/profile-only keys are rejected.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-config --locked --offline
cargo test -p browser-engine-qt --locked --offline
cargo test -p rustbrowser --locked --offline
cargo xtask check --locked
```
