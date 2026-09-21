# M0-86 — Safe tab clone

## Scope

The `tab-clone` command is implemented as a same-profile safe URL-descriptor
operation.

## Implemented behavior

- `tab-clone` creates a new live tab in the current window and navigates it to
  the current tab's safe URL descriptor.
- Credentials, fragments, secret-bearing query parameters, unsupported URLs,
  and control characters are excluded by the existing safe history policy.
- The operation never copies POST bodies or form state and reports a pending
  browser navigation for the new tab.
- The native toolbar exposes the same command path for a browser-owned clone
  action; stale or busy targets fail before creating a tab.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```
