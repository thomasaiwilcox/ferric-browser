# S27 integrated validation report

> Historical audit. The [completion report](completion-report.md) records later repairs and application coverage. The S21 specification contradiction is resolved; its current status is `blocked_environment`.

## Assessed tree

The integrated source tree is commit `7dbfb14af070c2494077ca6342ad009cefb41840` on `implementation/keyboard-interaction-v03`. After S26, S27 made one test-only cleanup in `tests/label_probes.rs` to satisfy strict Clippy. The opt-in probe reran successfully and its JSON result exactly matched the prior S25 output. No production implementation, configuration default, keybinding, or resource manifest changed during S27.

## Fresh gate results

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` passed on the assessed source tree. The workspace run included the CLI crate and reported 90, 16, 56, 140, 441 (one existing ignored), 29, 5, 85, and 17 passing tests in its nonempty test binaries; no failures occurred.

`QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask check` passed. It includes generated user-document freshness, QML lint, architecture and release-artifact checks, formatting, QML component/WebEngine tests, and workspace tests. The rerun reported 144/144 QML tests, checked the release artifact manifest's 38 inputs, and passed license policy for 11 locked dependencies. Its first run omitted the fixture's forced-GC flag and failed only `test_T044_scope_start_cancel_stress_and_forced_gc` (`gc_available=false`); that raw log is retained as `S27-xtask-check-no-gc.log`.

`QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask test engine` passed 441 engine tests (one existing ignored), 144 QML/WebEngine tests, and native CTest 1/1. The verbose native log shows 59/59 Qt offscreen cases passed, including overlapping/repeated key ownership, epoch invalidation, lost-release recovery, prompt routing, proxy focus, and exact native control activation. The separate bounded adapter smoke passed its 10-second Qt/QML lifetime.

`cargo xtask test wayland` passed nested compositor startup, then reported physical input qualification `not-run`: `wtype` could not access the compositor virtual-keyboard protocol. Startup is not physical keyboard or pointer evidence.

The pack integrity/evidence validator and its 12 structural/negative tests passed; it confirmed all 55 source items, 24 contracts, 26 unified invariants, 57 planned tests, and 28 slices are represented.

The first two strict-Clippy attempts are retained. They identified casts, a doc-code span, clone assignments, and an uninlined format string in the S25 probe; the final all-target strict-Clippy run is clean. The failure logs are diagnostic history, not current failures.

## Evidence that remains bounded

The fresh fixture and offscreen results establish the behaviors exercised by those fixtures. They do not establish physical Wayland routing, installed-release behavior, live-site compatibility, or universal support for editors, frames, accessibility providers, canvases, maps, or sliders. S23/S24/S25 research decisions, S20's environment block, and S21's specification block remain separate from the passing integrated test gates.

The latest performance evidence is S25 because there were no subsequent production changes. The 500-target callback p95 exceeded S17 by roughly 10–20% across zoom rows, focused repeats crossed the C17 relative threshold, and 500-marker overlay p95 was 212.60–224.29 ms against the 200 ms reference ambition. Rust-stage timings remained close to S17. Host load was elevated and no controlled low-load repeat was available, so these results remain unresolved rather than attributed to a runtime regression.
