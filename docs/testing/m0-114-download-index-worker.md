# M0-114: download index worker

Normal-profile download index creation now runs on the profile SQLite worker.
The offer path queues the metadata row without waiting on SQLite in the Qt
callback; the poller reports the durable commit or failure while the existing
engine/file behavior continues. Late results are handled by storage polling or
shutdown, and private profiles continue to avoid durable index writes.

Evidence:

- `cargo test -p ferric-browser-storage worker_creates_download_index_before_acknowledging --locked --offline`
- `cargo check -p ferric-browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  (expected timeout, no lingering process)
