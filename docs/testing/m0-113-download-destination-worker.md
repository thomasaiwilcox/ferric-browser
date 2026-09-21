# M0-113: download destination worker

Normal-profile download destination updates now execute on the profile SQLite
worker. Destination selection queues the durable update without waiting on
SQLite in the Qt callback; the poller reports commit or failure while the
engine lifecycle update proceeds. Late acknowledgements are consumed by
storage polling or shutdown, and worker failures are surfaced without a
GUI-thread store fallback; private profiles do not write durable metadata.

Evidence:

- `cargo test -p browser-storage worker_commits_download_destination_before_acknowledging --locked --offline`
- `cargo check -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir`
  (expected timeout, no lingering process)
