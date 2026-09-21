# M0-65 — Popup journey target binding

Approved popup windows now receive a bounded one-shot token from the same
browser-owned UI object that accepted the opener request. The token is bound
to the exact core popup target created by the reducer, so popup load-start,
document URL changes, commits, completion, failures, and teardown can update
that target without consulting the active primary tab. Popup commits preserve
the typed `popup` transition and the opener node parent; redirect observation
continues to classify a later document URL change conservatively.

Tokens contain no page data, are matched against the validated requested URL,
are capped at 256 pending entries, and are released when the popup view is
destroyed. An unclaimed or stale token produces no navigation effect.

Evidence:

* `cargo test -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
