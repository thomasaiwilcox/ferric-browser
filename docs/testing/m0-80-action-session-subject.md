# M0-80 — Typed session action subject

Date: 2026-09-16  
Status: in-progress

## Scope

Named sessions are now represented in the shared action registry as the
`browser.session.load` action. The action carries a bounded session name and an
optional typed `append` flag, and is discoverable through native action-list and
typed `actions.query` metadata.

## Implemented behavior

- `action session load NAME` and `action session load --append NAME` resolve to
  the existing `session-load` command path without reparsing command text.
- Typed IPC accepts `{ "name": "NAME", "append": true }` and produces the
  same bounded command arguments.
- The action executor rechecks normal-profile durable storage, resolves the
  named session through the existing safe preview path, and returns a
  structured pending session-preview result with
  `requires_confirmation: true`; loading does not begin until the browser-owned
  preview is explicitly accepted.
- Switcher session rows (`load-preview` and `load`) invoke the same stable
  action ID through browser-owned UI activation.
- Private and ephemeral profiles remain unable to load durable sessions, and
  stale or unavailable names fail without changing the live tab set.

## Verification

- `cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked`
- `cargo fmt --all -- --check`

The focused tests cover registry resolution, command argument typing, action
mapping, and switcher routing. Replace/append confirmation and full
multi-window session restoration remain in the existing session qualification
scope.
