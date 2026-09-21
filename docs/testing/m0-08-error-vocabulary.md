# Stable error vocabulary evidence

Date: 2026-09-18  
Status: in-progress

## DIAG-003

The local IPC response boundary canonicalizes legacy/internal codes such as
`E_INVALID_PARAMS`, `E_PROTOCOL_VERSION`, `E_PERMISSION`, `E_IN_FLIGHT`, and
`E_REJECTED` into the stable public vocabulary from the specification:
`E_INVALID_ARGUMENT`, `E_CONFIG`, `E_NOT_FOUND`, `E_NO_INSTANCE`,
`E_STALE_TARGET`, `E_DENIED`, `E_CONFIRMATION_REQUIRED`, `E_UNSUPPORTED`,
`E_BUSY`, `E_TIMEOUT`, `E_PROTOCOL`, `E_IO`, `E_STORAGE`, `E_ENGINE`, and
`E_CANCELLED`.

Every IPC failure receives an opaque `err-...` correlation ID and bounded
`preserved` and `next_action` details. The user-facing message includes the
same guidance without echoing request payloads. CLI failures now classify into
the same vocabulary while retaining existing process exit classes. When a
command includes `--format json`, the CLI emits a JSON error object on stderr
with the stable symbolic code, numeric status, human-readable message, opaque
`err-...` correlation ID, and bounded `preserved`/`next_action` guidance, so
automation does not need to parse the human form. Human-readable CLI errors
include the same guidance and correlation marker.
The Qt command/action boundary also maps missing objects, timeouts, queue
pressure, and permission refusals to `E_NOT_FOUND`, `E_TIMEOUT`, `E_BUSY`, and
`E_DENIED` before returning structured IPC failures.

Configuration, storage, protocol, and I/O failures are classified explicitly
before the general engine fallback, keeping native action responses aligned
with the same public vocabulary.

The CLI gives precedence to an explicit stable `E_*` code in an error payload
over prose heuristics. Unit coverage exercises every documented stable code,
including unsupported, I/O, engine, and cancellation failures.

## Verification

```text
cargo test -p ferric-browser-ipc -p ferric-browser --locked --offline
cargo xtask check
cargo fmt --all -- --check
target/debug/ferric-browser --software-rendering diagnostics --format json
```

Unit tests cover legacy-code canonicalization, guidance fields, correlation ID
shape, and CLI classification. The full workspace test gate passes.

## Remaining qualification

Native QML status strings still have a broader migration path to the same typed
error records. Action failures now reach the configured native JSONL sink with
their bounded operation/correlation identifier and category; non-action native
status updates remain intentionally outside that action ledger until their
operation ownership is explicit.
