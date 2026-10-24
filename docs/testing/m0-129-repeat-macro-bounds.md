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
cargo test -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
```

The existing native Wayland smoke remains the integration check; no macro
contents are persisted or sent to page JavaScript.

## Accepted commands and routed replay (2026-10-23)

Repeat and macro recording now commit after successful dispatch. Rejected
profile/context routes, malformed navigation, unavailable local commands and
canceled previews leave the last accepted action intact. A partially successful
chain retains its successful commands. Search, scrolling and zoom use the same
acceptance rule as Core commands.

Memory-only records retain command arguments, keyboard counts, destination,
selectors and source authority. Replay revalidates the route. A confirmed
clean-link records the cleaned URL; an accepted or dismissed context prompt
records the chosen outcome, so replay does not reopen the prompt or change a
background tab into current-tab navigation. Routed playback participates in the
same 1,000-command expansion budget as local commands.

This pass is complete and locally qualified. The release build passed in
13 min 01 s. All 1,078 Rust unit checks passed; one existing optional Qt test
remains ignored. Strict Qt adapter all-target release Clippy, formatting and
whitespace checks passed.

| Native check | Passed |
| --- | ---: |
| Repeat admission, macros, counts and window/profile routing | 23 |
| Open destinations, clean-link and context decisions | 26 |
| The same open destinations with owned Wayland input | 26 |
| Context reload and window ownership | 33 |
| Action feedback | 40 |
| Page search | 38 |
| Repeat suite with owned Wayland input | 23 |

All 209 native Qt checks passed, including 49 with owned Wayland input. Public
process checks passed 12 repeat steps, 10 existing destination steps and 19
warm/cold context scenarios. Each process fixture reports the same final CLI
hash and reaps its owned browsers. The 36 tracked build/test process IDs are no
longer live. The final audit verifies 683 frozen source/fixture files and exact
Rust, engine, native-runner and compositor artifacts.

The stale-preview regression confirms the existing navigation-retirement path
cancels confirmation after a newer navigation; the newer accepted command stays
repeatable. Context tests use the real Y/N dialog path and consistent authored
configuration before changing context, then verify dot and macro replay return
to the original context. No production Rust changed after the release build.

CI now selects `application_repeat` and `application_repeat_cli` once each among
37 browser-surface gates. Hosted CI has not been run. QML implementation did not
change in this pass; the native fixtures use the production embedded resources.

Authoritative evidence is `target/repeat-macro-final-facts.json`,
`target/repeat-macro-source-hashes.json`,
`target/repeat-macro-artifact-hashes.json` and
`target/repeat-macro-owned-jobs.json`. The broader browser usability goal is not
claimed complete by this scoped change.
