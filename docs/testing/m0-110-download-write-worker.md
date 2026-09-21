# M0-110: download lifecycle write worker

Normal-profile download progress and terminal lifecycle updates no longer
write SQLite synchronously from the Qt callback path. Qt queues bounded
`DownloadUpdate` records; the profile worker owns the SQLite connection and
commits each batch atomically with an acknowledgement. Snapshot consumers are
marked dirty until the acknowledgement is observed, failed batches remain
retryable, and shutdown drains the queue within the same bounded durability
window as history and permission writes.

The existing offer and destination-selection records remain synchronous because
the engine must receive a safe destination result before it starts writing the
file. Subsequent state/byte updates use the worker path. If the worker cannot
accept a batch, the bounded queue reports backpressure; worker failures retain
updates for retry and the profile worker is recreated from the profile path.

Evidence:

- `cargo test -p ferric-browser-storage download --locked --offline` (6 tests,
  including worker commit and rollback coverage)
- `cargo check -p ferric-browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  (expected timeout, no lingering process)
