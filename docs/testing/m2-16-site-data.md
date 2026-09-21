# M2-16 site-data clearing evidence

Date: 2026-09-16  
Status: in-progress

## Scope contract

`site-data-clear ORIGIN` accepts only the active exact HTTP(S) origin. Without
`--confirm`, it returns a preview. With confirmation, it runs a browser-world
operation for the active page and reports category results; it cannot target a
different tab or origin.

The operation keeps categories distinct:

- `local_storage`, Cache Storage, and service-worker registrations use their
  page-origin APIs and report `cleared` or `unavailable`.
- Cookies are explicitly `page-visible-only`; HttpOnly and other cookie-store
  entries are not represented as cleared.
- HTTP cache is `profile-wide-only` and is never selected by a site-origin
  request.
- Whole-profile reset, permissions, history, downloads, and other origins are
  excluded.

The active isolated browser-world result is polled until asynchronous cache and
service-worker operations settle. Navigation or view loss causes a stale or
failed completion instead of widening the target. No private engine files are
deleted to emulate per-origin support.

## Evidence

- The typed command registry and IPC adapter accept `origin` plus an explicit
  boolean confirmation and reject unknown fields.
- The native Site Ledger exposes the supported active-origin action only for a
  non-private normalized origin and requires a review checkbox.
- The QML action reports the exact supported categories and deliberately omits
  a profile-wide cache-clear control.
- `cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked` and the full
  `cargo xtask check` gate cover the command/parser and generated Qt surface.

## Limitations

The public pinned Qt Quick API does not expose a per-origin cookie-store or
HTTP-cache deletion API. Consequently, cookie clearing is limited to the
page-visible `document.cookie` subset, and HTTP cache remains untouched. A
separately confirmed whole-profile reset is not offered until its lifecycle and
two-origin isolation tests exist.
