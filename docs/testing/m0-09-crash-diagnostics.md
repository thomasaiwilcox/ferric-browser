# Process crash diagnostics evidence

Date: 2026-09-16  
Status: in-progress

## FAIL-005

The browser now preserves two profile-local lifecycle artifacts:

- `state/running` is a private, atomically replaced process marker. Its
  presence on the next start indicates that the previous browser process did
  not complete its shutdown path.
- `state/crash-events.json` is a private, atomically replaced JSON log capped
  at 32 lifecycle events. Events contain only a schema, event kind, process
  label, PID, timestamp, and whether an earlier marker was present.

The CLI `diagnostics --format json` and the live `diagnostics.get` IPC method
expose the same sanitized crash section. The section explicitly records that
core dumps are not collected or exported automatically, because Linux core
dumps can contain page and credential memory. It also reports debug-symbol
availability as unprobed unless release packaging supplies that evidence.

The design correlates the RustBrowser browser process through its marker and
structured events, but makes no claim that this prevents or fully attributes a
Qt helper-process or GPU-driver crash. Native crash attribution and release
package symbol publication remain deployment qualification work.

## Verification

```text
cargo test -p browser-storage -p browser-engine-qt -p rustbrowser
cargo fmt --all -- --check
cargo xtask check
target/debug/rustbrowser diagnostics --format json
```

The storage unit test verifies bounded retention, clean marker removal, and
that the serialized diagnostic snapshot does not contain the storage path,
URLs, or credential-like fields.
## FFI failure boundary

The workspace sets `panic = "abort"` in both development and release
profiles. This is required because CXX-Qt-generated Rust invokable entry
points are exposed as `noexcept` C++ functions. A Rust panic therefore cannot
unwind through Qt; the running marker remains in place and the next startup
reports the unclean process boundary through the existing crash diagnostics.
