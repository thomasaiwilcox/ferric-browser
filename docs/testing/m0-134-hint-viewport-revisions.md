# M0-134 hint viewport validation

Hint selection recomputes the retained element's current rectangle and walks
its live same-origin `frameElement` chain to rebuild top-view coordinates.
Rust validates the fresh finite geometry and rejects a selected target whose
center moved by more than the captured element dimensions (with a 16 CSS-pixel
minimum tolerance).

This detects meaningful scroll, resize, frame-offset, and layout changes for
the selected element without invalidating every label because an unrelated
part of a dynamic page changed. Stable element identity also prevents stale
coordinates from resolving to a different overlapping control. Cross-origin
frames remain inaccessible and are excluded.

Verification:

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
```
