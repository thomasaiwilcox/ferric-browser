# M0-27 untrusted-input boundary evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: SEC-001.
- Files: `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-storage/src/lib.rs`, `crates/ferric-browser-core/src/url.rs`, and
  `crates/ferric-browser-ipc/src/lib.rs`.
- Observable result: browser/page/IPC-derived data is bounded and validated
  before it enters reducer state, durable metadata, or native effects.

## Evidence

- Typed IPC text arguments are nonempty, control-free, and capped at 16 KiB;
  URL parsing has its independent bounded scheme/authority policy and rejects
  `rb://`, script, and other unsupported schemes.
- Page titles are sanitized for controls/bidi controls and capped at 4 KiB
  before reducer state, history, bookmarks, and userscript context use them.
- Hint candidates, selection results, page-dialog text, userscript results,
  and file paths retain their existing type, target, size, and control checks.
- Durable history/download URL checks reject control characters and values over
  16 KiB; history/bookmark/quickmark/download identifiers and metadata reject
  oversized or control-bearing text before SQLite writes.
- IPC requests remain typed data and are never reparsed as shell syntax; error
  responses additionally pass through the SEC-004 redaction boundary.

## Limitations

Long-duration fuzz campaigns, a live malicious-page/input fixture, and
exhaustive qualification of every Qt engine callback remain future work. Local configuration and
explicitly installed external scripts are trusted user inputs with separate
validation and OS-permission caveats described by the specification.

## Verification

```text
cargo test -p ferric-browser-core --locked
cargo test -p ferric-browser-engine-qt --locked
cargo test -p ferric-browser-storage --locked
cargo test -p ferric-browser-ipc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```
