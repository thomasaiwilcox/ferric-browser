# M0-61 — Bounded journey search and neighborhood expansion

The native journey manager now supports literal, bounded search over safe
titles, URLs, transition kinds, and source labels. Search is available from
the command surface as `journey --search TEXT` and through typed IPC.
The accessible native manager also exposes a `Current` filter backed by
`journey --current`, plus an `All` control to return to the profile-local
outline.

The outline's `Expand` action requests a one-hop neighborhood for an exact
retained node with `journey --expand NODE_UUID`. Durable storage performs
bounded SQL queries for the center's neighbors and for edges whose endpoints
are visible; private and ephemeral profiles use the bounded in-memory graph.
Search and expansion never load page previews or contact recorded URLs, and
wildcards in search input remain literal text. `--expand` is intentionally
exclusive with `--current` and `--search` so the view has one unambiguous
bounded scope. The native graph layout is covered by
`m0-62-journey-graph-layout.md`.

Evidence:

* `cargo test -p ferric-browser-storage -p ferric-browser-engine-qt -p ferric-browser --locked --offline`
* `cargo xtask check`
* `cargo build -p ferric-browser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir`

The native UI check is a bounded startup smoke; timeout exit is expected while
the GUI remains open.
