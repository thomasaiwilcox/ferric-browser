# Filter failure evidence

Date: 2026-09-16  
Status: in-progress

## FAIL-003 filter compilation

Cached host-policy lists now distinguish a malformed/undecodable list from a
missing cache entry. A compile failure is reported in the bounded policy
snapshot and the active browser rules are left unchanged. The native reload
surface reports the affected opaque list IDs and retains the last-known-good
blocked and exception host sets. A successful reload replaces both sets
atomically and reports that the policy was reloaded.

This keeps a broken update from silently disabling the previous blocker. The
maintained Brave `adblock` engine now compiles the full supported ABP network
rule subset, including URL/type/first-party options and exceptions. The
separate optional cosmetic path remains intentionally conservative: unsupported
cosmetic rules, scriptlets, and page-world behavior are ignored.

## Verification

```text
cargo test -p browser-engine-qt --locked --offline
cargo fmt --all -- --check
cargo xtask check
```

The policy unit tests cover a valid cached list, ABP type/exception matching,
the Rust↔Qt FFI provenance boundary, and an invalid-UTF-8 list that is
classified as a compile failure without producing active rules.
