# Clean-link update transport

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: LINK-005; transport portion of NET-002.
- Files: `crates/browser-config/src/lib.rs`,
  `crates/browser-core/src/command.rs`,
  `crates/browser-engine-qt/src/link_rule_updater.h`,
  `crates/browser-engine-qt/src/link_rule_updater.cpp`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: a normal profile can explicitly run
  `link-cleaning-update` when `[links.cleaning]` contains a paired HTTPS
  `update_source` and lowercase `update_sha256` pin. The fetch is bounded to
  1 MiB, uses Qt's no-less-safe redirect policy, requires HTTPS on the final
  response and the configured host, and aborts after 30 seconds. Rust then
  verifies the exact SHA-256, parses the manifest, runs its corpus, and uses
  the atomic private `accepted.bundle` acceptance boundary.

Defaults leave both fields unset. This transport slice had no automatic
clean-link scheduler; the later conditional-update and scheduler slices cover
that extension. There is no update attempt in private or memory-only profiles,
and failures retain the last accepted or compiled-reviewed rules.

## Verification

```text
cargo test -p browser-config --locked --offline
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
target/debug/rustbrowser diagnostics --format json
QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir
```
