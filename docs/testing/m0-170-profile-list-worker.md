# M0-170: profile-list worker

The profile manager no longer opens the durable profile registry from a QML
callback. `ProfileListWorker` owns the registry read on the named
`rustbrowser-profile-reader` thread and returns one bounded tab-separated
snapshot through a capacity-one response queue. Qt exposes the snapshot and a
pending flag; the profile manager polls those values and never blocks the GUI
thread on registry I/O.

The command and interactive profile-manager paths now share the same worker
boundary. Profile mutation remains on its separate worker, while profile-open
uses the list response to resolve its durable target without a GUI-thread
registry read.

Evidence:

- `cargo test -p browser-engine-qt profile_list_worker_reads_registry_off_thread --locked --offline`
- `cargo xtask check --locked --offline`
- `cargo fmt --all -- --check`
