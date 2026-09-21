# M0-135 open shadow-root hint activation

Hint collection walks open shadow roots and retains the exact interactive DOM
element behind each bounded candidate ID. Fresh selection, focus, and ordinary
control activation use that retained identity, so `elementFromPoint` returning
the shadow host or an overlapping interactive descendant cannot retarget the
hint.

Closed shadow roots remain inaccessible by design. The existing document,
frame-path, metadata, visibility, and geometry checks still run before
navigation, focus, or control activation.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
```
