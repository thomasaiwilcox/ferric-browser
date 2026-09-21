# Clean-link accepted-rule cache

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: LINK-005.
- Files: `crates/browser-engine-qt/src/link_cleaning_policy.rs` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: a validated downloaded rule candidate can be accepted
  atomically, becomes active for normal profiles, and cannot displace the last
  accepted snapshot when parsing, corpus, size, UTF-8, or checksum validation
  fails.

Candidates are stored as one private `cache/link-cleaning/accepted.bundle`
envelope containing the SHA-256 and manifest bytes. The manifest is parsed and
its complete corpus is executed before the envelope is atomically replaced.
Normal-profile clean-link callers load that accepted snapshot; private and
memory-only profiles have no storage root and remain on the compiled reviewed
bundle. A malformed or mismatched cache is rejected and the compiled bundle
remains active. IPC diagnostics identify the active revision and whether it is
compiled or accepted from cache without exposing corpus URLs or filesystem
paths.

The browser still has no configured remote source or automatic clean-link
refresh scheduler. A future transport must feed this acceptance boundary using
NET-002 HTTPS, conditional-request, timeout, size, and last-known-good rules.

## Verification

```text
cargo test -p browser-engine-qt link_cleaning_policy --locked --offline
cargo xtask check --locked
target/debug/rustbrowser diagnostics --format json
QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir
```
