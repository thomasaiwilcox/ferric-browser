# M1-12 clean-link core evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: LINK-001 through LINK-004 (core portion).
- Files: `crates/ferric-browser-core/src/link_cleaning.rs`,
  `crates/ferric-browser-core/src/lib.rs`, `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, and `crates/ferric-browser/src/main.rs`.
- Observable result: `clean_link` is an explicit caller-controlled operation
  over a captured URL. Reviewed rules have stable IDs, source and revision
  metadata, bounded host/parameter data, and duplicate validation. The builtin
  revision removes only literal `utm_*`, `gclid`, and `fbclid` query names from
  HTTP(S). It preserves component order, percent encoding, fragments, and all
  non-matching values. Results expose original and cleaned strings, applied
  rules, removed/retained parameter names, and an explanation. Unsupported
  schemes, malformed authority shapes, encoded parameter names, and no-op
  cases retain the original URL.
- Evidence: `cargo test -p ferric-browser-core --locked` (42 tests), including four
  clean-link tests; the engine exposes `url-clean`, `url-explain`, and
  `open --clean-link` through interactive, typed command/action IPC, and the
  CLI. Changed opens create a target-aware pending navigation, keep the live
  tab unchanged until native confirmation, and then route the cleaned URL
  through the normal engine action path. The native command surface opens a
  sanitized preview overlay. URL/link copy and clean-copy are exposed through
  `yank`, typed action IPC, and the native QML clipboard bridge; copied values
  use the safe URL form and transient chrome offers re-copy. `cargo clippy --workspace --all-targets
  --locked --offline -- -D warnings` remains green.

## Limitations

Hint clean-yank and native context-menu clean-copy now reuse the reviewed
action path; inspect integration, full URL canonicalization, and downloaded
rule updates remain. The reviewed corpus, manifest checksum, and
active audit/changelog surface are covered by
`docs/testing/m0-122-link-rule-audit.md`. The
command preview masks credentials and recognized secret query values at its
IPC/native output boundary. The core never cleans ordinary navigation
implicitly and performs no network access.
