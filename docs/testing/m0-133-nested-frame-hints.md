# M0-133 nested-frame hint geometry

Hint collection now walks same-origin `iframe`/`frame` descendants up to eight
levels, assigning bounded paths such as `0.2.3` and translating descendant
rectangles into top-view coordinates. Fresh validation resolves the same frame
path before checking visibility and metadata, and input controls are focused in
their owning frame rather than assumed to belong to the top document.

Cross-origin and inaccessible frames are skipped by the browser's normal DOM
access rules. Closed shadow roots, PDF UI, and trusted gesture-only controls
remain explicit exclusions. Frame paths and candidate counts are validated in
Rust before they can enter a hint session.

Verification:

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
```
