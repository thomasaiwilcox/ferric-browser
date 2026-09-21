# M0-131 macro register gestures

The native normal-mode input path now implements the V1 `q` and `@` register
gestures. `q` waits for one ASCII letter or digit, starts recording into that
register, and toggles the same register off; `@` waits for the register and
replays it. The gesture is consumed by the browser, never typed into the page,
and rejects a second active recording rather than replacing the first one.

The register prefix uses the existing 1,000 ms chord timing, publishes the
same non-modal continuation overlay as other incomplete keychains, and is
cleared by Escape, timeout, invalid register input, document invalidation, or
runtime shutdown. Command-line and typed IPC macro commands continue to use
the same executor and bounds.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
```

Live Wayland keyboard qualification remains a follow-up for the broader input
matrix.
