# ADR-0005: Engine-derived origin normalization

- Status: accepted for the V1 security boundary
- Date: 2026-09-18
- Scope: URL canonicalization, permission keys, site policy, and origin
  matching

## Decision

Qt supplies the active document/request URL facts, while Rust owns a strict
canonicalization boundary for supported HTTP(S) origins. Scheme and host
casing, trailing DNS dots, IDN labels, IPv6 authorities, and default ports
are normalized deterministically. Credentials, fragments, malformed input,
unsupported schemes, ambiguous wildcard patterns, and unsafe text are
rejected or preserved according to the calling contract.

Permission and site-policy keys use the same normalized exact-origin form.
The network interceptor consumes immutable bounded policy snapshots and never
calls storage or browser UI while deciding a request.

## Consequences and evidence

Different Qt/Rust representations cannot silently grant a rule to another
site. URL display may redact or visually mark unsafe text, but display
redaction is never used as a policy key.

- `crates/browser-core/src/url.rs`
- `crates/browser-engine-qt/src/lib.rs`
- `crates/browser-engine-qt/src/request_interceptor.cpp`
- `crates/browser-storage/src/permissions.rs`
- `docs/testing/m0-33-tls.md`
- `docs/testing/m0-127-network-request-context.md`

## Reconsideration trigger

Revisit only after a differential test demonstrates a required Qt/Chromium
origin form that this boundary cannot represent conservatively.
