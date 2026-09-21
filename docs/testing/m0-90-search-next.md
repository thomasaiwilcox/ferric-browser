# M0-90 — Retained in-page search traversal

The V1 `search-next` command and `browser.tab.search-next` action now traverse
the retained in-page search for the current live tab.

## Contract

- `search-next` advances the retained search in its current direction.
- `search-next --backward` traverses backward.
- `search-next --count N` repeats the traversal up to 100 matches.
- `action tab search-next [forward|backward] [N]` exposes the same operation
  through the stable action registry.
- Typed IPC accepts `direction` (`forward` or `backward`) and integer `count`
  fields; shell text is never reparsed by the typed path.
- No retained search is an explicit error; stale or non-live tab targets are
  rejected before an engine operation is queued.

The reducer retains the search query and direction and emits one typed
`SearchNext` event per requested match. The Qt adapter queues a bounded
`find-next` engine action, and both the primary and secondary window surfaces
execute it against their owned WebEngine view. The search overlay exposes
accessible previous/next controls, while normal-mode `n`/`N` remains the
keyboard traversal path.

## Verification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
