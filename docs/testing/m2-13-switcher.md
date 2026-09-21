# M2-13 universal switcher activation evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirements: SWITCH-002 search/ranking, SWITCH-003 stored-result actions and
  the activation portion of M2-13, plus SWITCH-006 accessibility and scale.
- Files: `crates/ferric-browser-engine-qt/src/lib.rs` and
  `crates/ferric-browser-engine-qt/qml/Main.qml`.
- Observable result: shared `ferric-browser-core::switcher` primitives tokenize plain
  text with bounded Unicode-aware lowercasing and assign deterministic match
  classes; every switcher row exposes an allowlisted action button.
  Live tabs and windows focus through the reducer; closed tabs reopen through
  a fresh safe navigation; history, bookmarks, and quickmarks revalidate their
  stored URL before navigation; contexts enter only within the active profile;
  named sessions always open the existing validated preview; downloads use the
  existing regular-file/symlink checks; and command rows can either open
  filtered help or execute the registered no-argument command through the
  shared command path. Each result carries an opaque owner token so a merged
  switcher can route activation to the exact live `BrowserUi` and Qt window.

## Safety boundaries

The QML surface forwards the result kind, opaque row ID, displayed action, and
the captured live-tab generation when present. Rust checks the generation and
action against a per-kind allowlist and re-resolves the row from current state
or storage before changing anything. The public
`switcher.activate` method accepts the same typed result kinds and actions,
including normal-profile tab/window focus, stored-entry open/delete, session
preview/load, download show/open, command help/execute, context entry, and
closed-tab reopen. Private or ephemeral rows are excluded from ordinary
queries. An explicitly opened switcher inside a transient profile is scoped to
that same in-memory profile plus nonsensitive command/action rows; external
private queries still require configuration and explicit opt-in. A stale row
fails with status feedback rather than falling back to the active tab. Session
loads
cannot bypass the replace/append preview, and stored URLs must still satisfy
the durable history safety policy and strict URL parser.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-engine-qt --locked`
- `cargo check -p ferric-browser-engine-qt --locked`
- `cargo xtask check`
- `cargo build --locked`
- `QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir`

The focused native crate suite includes an allowlist/default-action regression
test. A live owner smoke confirms switcher rows expose an opaque owner token.
The universal action scope now includes argument-free registry actions that
declare switcher support; activation revalidates the registry and preserves
`source=switcher` provenance. Subject-bearing actions remain on their explicit
typed surfaces until a live subject is captured.

The remaining qualification gap is compositor-specific workspace routing and
denial reporting, plus generation-mismatch fault injection; the Qt host
activation request and secondary-view focus path are now integrated.

The switcher exposes accessible scope chips for all, tabs, windows, contexts,
commands, actions, history, marks, sessions, downloads, and closed entries;
changing a chip reuses the same bounded plain-text query path. It also sizes
itself from the available window and chrome font metrics, and exposes a named accessible list with selected list items,
descriptions, result count, and Home/End/Page Up/Page Down keyboard navigation.
The row cap follows the validated `switcher.max_results` setting (bounded to
10–1000, with a 100-item fallback) so refresh and assistive-technology
traversal stay bounded. Multi-window sources are now queried one per GUI event-loop batch and
publish partial results; query, scope, close, and replacement-generation
guards prevent an older batch from replacing newer input. A revisioned Rust
result cache also avoids rebuilding unchanged source snapshots. Normal profile
durable rows are now copied into a revisioned query index on the
`ferric-browser-switcher-index` worker thread; the Qt path rejects stale profile
or storage revisions and falls back to the immutable storage snapshot until a
matching index is ready. The focused regression
`switcher_library_index_is_profile_scoped_and_query_ready` covers the index
shape. Query paging now uses bounded top-K selection for `offset + limit`
before deterministic ordering; `switcher_page_selection_preserves_order_and_total`
covers the total-count and page-order contract. Screen-reader event,
cancellation stress, and large-scale performance qualification remain.
