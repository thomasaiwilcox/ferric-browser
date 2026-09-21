# M0-63 — Conservative journey redirect classification

The native load boundary now captures the URL from Qt's load request before
the first document commit. If the engine reports a subsequent top-level URL
change whose document URL differs (fragment-only changes are excluded), the
commit is tagged with the typed `redirect` journey transition. Explicit
transitions such as `hint`, `popup`, `session-restore`, and `reopen` retain
their higher-level classification. History traversal and failed loads clear
the pending redirect state, so stale callbacks cannot label a later visit.

This records only the safe committed URL/title/timestamp descriptors already
allowed by the journey boundary; no referrer header, redirect chain body,
form data, or page-script output is stored.

Evidence:

* `cargo test -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
