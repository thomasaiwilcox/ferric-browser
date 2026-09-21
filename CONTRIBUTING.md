# Contributing

Ferric Browser is a Cargo-first Rust/Qt project. Before proposing a change, read
the [development specification](docs/DEVELOPMENT_SPEC.md) and identify the
requirement and evidence record it advances.

## Local checks

Run the relevant focused tests, then the full gate:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo xtask check --locked --offline
```

Changes touching the Qt adapter should also run `cargo xtask test adapter`.
Wayland changes should run `cargo xtask test wayland` when Weston and a D-Bus
session are available; otherwise record the unavailable environment honestly.
Do not require a personal Google account or other external account in normal
CI.

## Design and ownership

The Qt-independent crates own typed state, commands, actions, validation, and
invariants. The runtime owns application orchestration, while the Qt adapter
owns WebEngine and QML integration. New commands,
actions, settings, bindings, contexts, compatibility quirks, and cleaning
rules must extend their existing typed registry and evidence record instead of
adding a parallel hand-written menu or parser.

Application state is read-only outside `ferric-browser-core`; every mutation
must be expressed as a reducer event. New adapter features emit typed runtime
inputs and consume typed effects or presentation snapshots. Do not add policy
to the legacy engine or QML composition roots while they are being split.

FFI entry points must not unwind across Qt, block the UI thread, expose page
content as native authority, or bypass target/privacy validation. Add focused
tests for invalid input, stale targets, boundedness, and failure behavior.

## Pull requests

Describe observable behavior, affected requirements, privacy/security impact,
tests run, and any unavailable native qualification. Include screenshots only
when they materially explain a UI change. Keep commits scoped and do not add
profiles, cookies, credentials, generated targets, or unrelated machine state.

## Wayland bug reproduction

Use the disposable fixture and nested-Wayland commands where available. Include
compositor, GPU/driver, portal, PipeWire, Qt, and QtWebEngine versions plus a
sanitized diagnostics report. Replace URLs, paths, titles, account names, and
tokens with stable placeholders.

## Security

Do not disclose vulnerabilities in public issues or pull requests. Follow
[SECURITY.md](SECURITY.md); a private reporting route must be configured before
public V1.
