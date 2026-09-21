# M0-172: asynchronous profile command reads

The command-driven `profile-list` path now queues the profile-registry read on
the bounded profile-list worker and reports the result when the Qt poller
receives it. `profile-delete` no longer performs a synchronous registry lookup
before handing the action to QML; the existing worker-backed deletion preview
is the confirmation boundary. The legacy qinvokable getters expose only the
already-published Qt snapshots and do not open the registry.

Evidence:

- `cargo test -p ferric-browser-engine-qt profile_ --locked --offline`
- `cargo xtask check --locked --offline`
- `cargo fmt --all -- --check`
