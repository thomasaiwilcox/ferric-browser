# M0-128 blocklist source metadata

This slice completes the source-date part of NET-001/NET-002. Blocklist
updates already write bounded validator metadata beside each accepted cached
list; the Rust policy loader now reads that metadata without making it part of
the synchronous request callback.

For each successfully loaded list, `blocking-status` and `site.status` expose
only bounded metadata: cache/list ID, accepted byte count, whether an ETag was
recorded, and an optional validated `Last-Modified` value. Missing or malformed
metadata does not disable an otherwise valid last-known-good list. The raw
source URL, request body, credentials, and arbitrary response headers are not
exposed.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt network_policy --locked --offline
cargo xtask check --locked
```

The native startup smoke remains the final integration check. A cached list
with no validator metadata is still usable and reports an empty metadata
field, preserving offline first-use behavior.
