# Clean-link rule bundle audit

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirements: LINK-002 and LINK-005.
- Files: `packaging/link-cleaning-rules.toml`,
  `packaging/link-cleaning-rules.sha256`, and
  `crates/ferric-browser-engine-qt/src/link_cleaning_policy.rs`.
- Observable result: clean-link callers use a checksummed, versioned reviewed
  bundle rather than an untracked code-only rule set; every shipped revision
  carries corpus cases and a human-readable changelog.

The compiled bundle is parsed and validated before use. It is bounded to 512
rules and 64 corpus cases, rejects duplicate IDs and parameters, and verifies
the SHA-256 sidecar before producing `CleaningRules`. Its corpus covers both
expected removals and high-value non-removals: unknown signature/state values,
encoded parameter names, and unsupported schemes remain unchanged.

Diagnostics expose the active revision, source, manifest checksum, rule/corpus
counts, passed corpus count, changelog, and the explicit update status without
including corpus URLs. The update field is currently `not-configured` and
automatic refresh is false. A future downloaded-rule transport must use the
existing NET-002 HTTPS limits, conditional requests, timeout, atomic acceptance,
and last-known-good retention. The acceptance boundary is now exercised by
`docs/testing/m0-123-link-rule-accepted-cache.md`; this slice still does not
enable remote clean-link rule traffic.

## Verification

```text
cargo test -p ferric-browser-engine-qt link_cleaning_policy --locked --offline
cargo xtask check --locked
target/debug/ferric-browser diagnostics --format json
```
