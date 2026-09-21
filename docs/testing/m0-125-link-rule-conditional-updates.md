# Clean-link conditional updates

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirements: LINK-005 and the conditional-request portion of NET-002.
- Files: `crates/browser-engine-qt/src/link_rule_updater.h`,
  `crates/browser-engine-qt/src/link_rule_updater.cpp`,
  `crates/browser-engine-qt/src/link_cleaning_policy.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: an explicit clean-link update reads private validator
  metadata from `cache/link-cleaning/accepted.meta`, sends bounded
  `If-None-Match` and `If-Modified-Since` headers when present, and accepts a
  same-host HTTPS `304 Not Modified` without replacing the active bundle.
  Successful `2xx` candidates pass response validators to Rust; metadata is
  written only after checksum, parse, and corpus validation succeeds.

Validator values are bounded and control-character rejected. Missing or
unusable metadata simply causes a full fetch. The accepted rule bundle remains
the atomic last-known-good boundary, and private profiles still perform no
update traffic. Automatic clean-link scheduling is covered by the follow-up
evidence in `m0-126-link-rule-scheduler.md`.

## Verification

```text
cargo test -p browser-engine-qt link_cleaning_policy --locked --offline
cargo xtask check --locked
target/debug/rustbrowser diagnostics --format json
QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir
```
