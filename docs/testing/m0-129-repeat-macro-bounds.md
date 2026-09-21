# M0-129 repeat and macro bounds

This slice closes the remaining boundedness gap in INPUT-006. The native
command executor now uses an explicit safe repeat/macro eligibility set that
covers navigation, scrolling, zoom, retained search traversal, and tab
movement/open commands. Clipboard/page typing, permission decisions, spawn,
downloads, site-data clearing, and other sensitive or confirmation-bearing
flows remain ineligible.

Macro recording is memory-only and stops at 1,000 eligible command
invocations. Replay tracks the total expanded command count across nested
macros and repeat commands, stopping at 1,000 with a browser-owned status
message. Nested macro playback remains limited to depth 8. The expansion
counter is reset for each top-level replay and all registers are cleared with
the browser runtime.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
```

The existing native Wayland smoke remains the integration check; no macro
contents are persisted or sent to page JavaScript.
