# M0-68 — Journey redirect-chain metadata

Engine URL observations now retain a bounded count of distinct non-fragment
redirect hops for each pending navigation. Repeated callbacks for the same
URL do not inflate the count, fragment-only changes do not count as redirects,
and history traversal never becomes a redirect node. A normal redirect is
reported as `redirect`; a multi-hop chain is reported as
`redirect-chain:N`. Explicit hint, popup, session-restore, and reopen
transitions retain their type while carrying the bounded redirect metadata in
their safe source field.

The native journey outline and graph node accessibility descriptions expose
that source metadata, while durable export preserves the same sanitized field.

Evidence:

* `cargo test -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
