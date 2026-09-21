# M0-67 — Background-link opener relationships

Link opens that create a background tab now capture the source tab's current
journey node before the new tab is created. Both direct typed background opens
and confirmed clean-link background opens attach that exact parent to the new
navigation target. Same-tab navigation and blank-tab creation remain
unlinked, and a missing/stale source node does not fabricate a relationship.

Evidence:

* `cargo test -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
