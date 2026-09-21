# Cancellable process-group evidence

Date: 2026-09-16  
Status: in-progress

## Scope

Browser-owned external editors and userscripts now start in a dedicated Unix
process group. Cancellation and timeout terminate that group with `SIGTERM`,
wait for at most 500 ms while polling the tracked child, then send `SIGKILL`
and reap the child if it is still running. Scratch-file removal and suppression
of late completion use the existing editor cancellation path.

Explicit `spawn` operations retain their separate lifetime: they are detached
browser operations and are not killed merely because a tab or its originating
userscript closes. A process that deliberately calls `setsid` or otherwise
leaves its process group is outside this bounded guarantee.

## Verification

```text
cargo test -p ferric-browser-engine-qt --locked --offline
cargo clippy -p ferric-browser-engine-qt --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
```

Unit coverage verifies both tracked-child reaping and termination of a shell
fixture's descendant in the same process group. Live Wayland cancellation and
detached-daemon behavior remain qualification work; browser-owned editor stderr
is intentionally discarded while userscript stderr uses an explicit bounded
operations query.
