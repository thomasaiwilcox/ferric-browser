# M0-62 — Bounded native journey graph layout

The journey manager's relationship mode now includes a native graph layout.
It renders safe journey nodes and typed edges from the existing bounded graph
descriptor, while retaining the accessible relationship list beneath it.
The layout uses deterministic grid placement, a 120-node visual cap, a
scrollable surface, and text labels for transition kinds so color is not the
only relationship cue. It never loads page previews or contacts recorded
URLs.

Evidence:

* `cargo xtask check`
* `cargo build -p ferric-browser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir`

The Wayland check is a bounded startup smoke; timeout exit is expected while
the GUI remains open. Full interactive screenshot and assistive-technology
qualification remain environment-specific.
