# M0-24 logging and diagnostics evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: SEC-004.
- Files: `crates/ferric-browser-ipc/src/lib.rs`,
  `crates/ferric-browser-engine-qt/src/diagnostics.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, and
  `crates/ferric-browser-storage/src/lifecycle.rs`.
- Observable result: serialized IPC failures and the diagnostics preview do
  not expose common credential, form, or URL-secret data.

## Evidence

- `Response::failure` recursively redacts object fields containing password,
  PIN, authentication, cookie, bearer, token, secret, form, body, or header
  data. It also redacts bearer values, `key=value` secret components, and URL
  userinfo, query strings, and fragments in free-form error messages/details.
- The diagnostics snapshot declares page console logs and full navigation
  traces excluded, keeps redaction mandatory even when debugging is discussed,
  and marks its content as a preview requiring an explicit user action. There
  is no automatic upload path.
- Native diagnostics uses a user-visible “Copy preview” action. It is not a
  background export and private browsing data remains excluded from the
  snapshot/site report contracts. It also offers an explicit “Save preview”
  flow after the same visible snapshot, using a local file chooser, refusing
  overwrite, and writing through a private atomic file boundary.
- IPC failures retain a bounded process-memory ring of the opaque correlation
  ID and stable public error code. Diagnostics exposes that ring without
  retaining messages, URLs, request payloads, or other error details.
- The profile-local crash ledger is bounded to 32 structured lifecycle events,
  stores only event/process/timestamp state, and diagnostics omit storage paths
  and core dumps.
- `--log-level error|warn|info|debug` enables the bounded structured stderr
  sink for CLI events. GUI action audit events additionally use the configured
  `logging.level`, `logging.max_file_mib`, and `logging.retained_files` values
  to write redacted JSONL under the profile state `logs/` directory. The file
  is private, rotates before an existing oversized file is reused, and is
  released during profile teardown. Log creation refuses a symlinked active
  file on Unix, and event fields never include command arguments, URLs,
  profile names, or page data.

## Limitations

Dedicated disposable debug-profile qualification remains future work.
Correlation records are intentionally process-local and are cleared when the
browser exits; the file sink contains only bounded action identifiers and
outcome categories and is never uploaded automatically.

## Verification

```text
cargo test -p ferric-browser-ipc --locked
cargo test -p ferric-browser-engine-qt --locked
cargo test -p ferric-browser-storage --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```
