# M0-42 context authentication guard

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: CTX-005.
- Files: `crates/browser-engine-qt/src/lib.rs`.
- Observable result: an IPC `open` routed to a context validates the target
  window, context existence, and profile affinity before changing window
  membership. A rejected cross-profile route therefore cannot leave a durable
  context assignment behind.

This guard applies only before an explicit navigation request is created. It
does not move an existing popup, redirect, submitted form, WebAuthn request,
permission request, or other authentication-chain tab between contexts.

## Verification

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked
cargo xtask check
```

Route matching from `contexts.toml` is now implemented through the shared
bounded pattern matcher and is covered by configuration tests and the
pre-navigation Qt route path. Remaining work is interactive multi-window
enter/focus behavior and full OAuth/authentication-chain qualification with
disposable accounts.
