# Secret storage boundary

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: SEC-005.
- Files: `crates/ferric-browser-engine-qt/src/diagnostics.rs`.
- Observable result: diagnostics state that V1 has no built-in password vault,
  no configured external password-manager adapter, no persistent secret fields,
  and no credential-handler capability for userscripts.

HTTP/proxy authentication values remain transient engine-request state. The
browser-owned prompt clears its password field after submission or teardown;
credentials are not written to the Rust profile store, session snapshots,
command history, logs, or diagnostic exports. An external password-manager
integration remains a separately reviewed future adapter with exact-origin
checks.

## Verification

```text
cargo test -p ferric-browser-engine-qt diagnostics --locked --offline
cargo xtask check --locked
ferric-browser diagnostics --format json
```

The diagnostic object is a policy boundary, not a claim that an external
password manager is installed or that a credential store is available.
