# M2-02 history, bookmarks, and quickmarks evidence

Date: 2026-09-15  
Status: in-progress

## Implemented slice

Profile-local history is recorded only for safe committed URLs. Normal profiles
use the storage worker and bounded newest-first readers for history, bookmarks,
and quickmarks. Private and ephemeral profiles use a separate bounded
newest-first in-memory history store for the lifetime of the session; it never
opens SQLite, creates session files, or participates in durable worker writes.
Bookmark/quickmark mutations remain unavailable to transient profiles, while
private history can be listed, completed, reopened by its negative in-session
ID, and explicitly cleared by origin or age.

Settled same-document URL changes are recorded through the acknowledged history
worker path with a distinct transition, while callbacks belonging to a pending
full navigation are suppressed to avoid duplicate visits. These changes do not
create extra journey nodes.

The shared command registry and typed IPC/CLI paths now support:

- `bookmark-add [--title TEXT]`, `bookmark-delete ID`, `bookmark-list`
- `quickmark-add NAME [URL]`, `quickmark-delete NAME`, `quickmark-list`
- `history`, and confirmation-gated `history-clear [--since UNIX_SECONDS]
  [--origin EXACT_HTTP_ORIGIN] --confirm`

Recently closed normal-profile descriptors are also available through
`tab-undo`, which reuses the same safe reopen path as the switcher.

The native command surface opens a profile-local list manager for the list
commands. All displayed URLs use the same sanitized output boundary as IPC
queries. History, bookmark, and quickmark rows expose typed Open actions; the
bookmark and quickmark rows also expose two-step Delete actions that reuse the
same validated command boundary.

History writes prune committed visits older than the configured retention
cutoff in the same transaction as insertion; the storage fallback uses the
same profile-specific cutoff and defaults to 90 days when none is supplied.
Eligible command history
is transactionally capped at 1,000 newest entries.
History clearing now carries the validated timestamp and exact HTTP(S) origin
through typed IPC, CLI command forwarding, the storage worker, and the SQLite
transaction. Origin matching is boundary-aware, so `https://example.test`
cannot clear `https://example.test.evil`.

## Evidence

- `cargo test --workspace --locked --offline`
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
- `cargo build -p rustbrowser --locked --offline`
- Live owner smoke: quickmark and bookmark creation were forwarded to the
  running owner, `switcher.query --scope marks` returned the bookmark, and an
  unconfirmed `history-clear` was rejected.

## Remaining work

Completion now consumes the bounded profile-local history, bookmark, and
quickmark catalog. The native manager supports opening all three record types,
deleting bookmarks/quickmarks with explicit confirmation, and bounded paging
over the loaded catalog. `switcher.query` exposes bounded offset/limit pages
with a `has_more` watermark for manager-scale browsing. Remaining work is
performance qualification at the target history size and richer worker
ownership; bookmark titles and quickmark destinations are now editable through
the same typed, stale-target-safe storage worker path.
