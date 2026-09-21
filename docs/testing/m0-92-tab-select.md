# M0-92 — Tab selection by index or stable ID

The tab-selection command is now shared across command, typed IPC, action, and
tab-strip UI paths:

- `tab-select N` selects the live tab at displayed one-based index `N`.
- `tab-select TAB_ID` selects a live tab by its stable runtime ID.
- `action tab select SELECTOR` and `browser.tab.select` use the same typed
  command and reducer event.
- The QML tab strip resolves its stable ID and invokes the registered action;
  it does not directly mutate core tab state.

Index zero, unknown IDs, stale tabs, missing selectors, and extra arguments
are rejected. Selection remains scoped to the current window and follows the
ordered live tab list maintained by the core reducer.

## Verification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
