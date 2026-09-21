# M0-69 — Typed action audit and error outcomes

Typed `action.execute` invocations now return structured failure details with
the action ID, operation ID, stable error code, failure category, and a bounded
retry hint. Categories distinguish invalid subject/parameters, stale targets,
confirmation requirements, missing capabilities, cancellation, denied source
or privacy policy, userscript failure, and executor failure.

The browser also retains at most 128 redacted action audit records in memory.
Records contain only action ID, operation ID, outcome, and category; arguments,
URLs, titles, page content, and credentials are never recorded. Diagnostics
expose the bounded audit surface with its privacy contract and add a
category-only recent-error summary that omits those identifiers. Failed action
operations are queryable through the existing operation-status surface.

Native/QML action activation and manifest-backed userscript actions now enter
the same bounded audit ledger. They retain the executor's operation ID when it
returns one, otherwise use a bounded UI-local correlation ID; failures use the
same redacted error-category mapping as IPC. The shared action resolver also
enforces each definition's allowed invocation source before capability checks.

Each ledger insertion also emits a bounded `action.audit` event through the
existing IPC event stream. Its payload is exactly the four ledger fields above,
so event subscribers receive live structured outcomes without action arguments,
URLs, titles, page content, or credentials. The same subscriber limits and gap
handling apply to these events.

Typed action responses additionally expose a bounded `command_context` object
with the IPC source, count, operation ID, captured window/tab/profile identity,
profile privacy, context metadata, requested route, and open target. Executor
failures preserve that envelope inside structured error details without adding
URLs, titles, page content, arguments, or credentials.

Evidence:

* `cargo test -p ferric-browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p ferric-browser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
