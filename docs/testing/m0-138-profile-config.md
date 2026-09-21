# M0-138 profile configuration

Date: 2026-09-17  
Status: verified

## Scope

`profiles.toml` is now a bounded, data-only configuration layer. It supports
profile names, display labels, one optional default profile, and typed
configuration overrides. Unknown keys, structural configuration fields,
duplicate names, duplicate defaults, invalid labels, oversized values, and
invalid typed values are rejected before startup or reload can commit.

The selected default profile is applied after the authored `config.toml` layer
and before generated runtime, CLI, and temporary overrides. Its name and label
also identify the normal storage profile. A missing or non-default profile
definition falls back to the built-in `default` profile. Private, ephemeral,
and temporary profiles continue to use memory-only runtime overrides.

The Qt engine receives the validated profile layer separately from the authored
base configuration. Configuration reloads watch the sibling `profiles.toml`,
re-read it transactionally, and retain the last good effective configuration
when either the authored config or profile layer is invalid.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-config --locked --offline` — 30 tests
- `cargo test -p ferric-browser-engine-qt --locked --offline` — 104 tests
- `cargo test -p ferric-browser --locked --offline` — 33 tests

The full workspace gate and native startup smoke remain the release-level
checks for this slice.
