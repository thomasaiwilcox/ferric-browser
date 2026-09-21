# M0-25 settings manager evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

The browser now exposes a native Settings manager for the validated runtime
configuration fields that already feed the Qt chrome and WebEngine views. The
manager provides typed boolean, enum, text, and numeric editors; filtering by
key/description; reset actions; explicit persistent versus temporary mode; and
scope/apply-time labels. Persistent edits call the existing atomic
`runtime-overrides.toml` transaction and never rewrite authored configuration.
Temporary edits stay memory-only, including in private profiles. Invalid keys,
values, bounds, and enum choices are rejected by the same Rust validator used
by `:set`/`:unset`, with the last valid live configuration retained.

The UI is browser-owned and accessible, takes focus while visible, closes
through Escape or a named action, and restores the captured browser target.
Site-scoped editing is deliberately labeled as command-only in this slice;
the generic manager does not pretend a global override is a per-site rule.

## Verification

```text
cargo xtask check
cargo build --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Qt’s existing WebEngine deprecation and
temporary-profile storage notices remain; the startup check produced no QML
errors, binding loops, or type errors.
