# M0-87 — Tab movement uses the shared action path

The visible tab-strip left/right controls now invoke the registered
`browser.tab.move` action with the stable tab ID and bounded direction value.
The native action adapter decodes that small UI payload into the same typed
`tab-move` command used by IPC and command-mode execution. Stable-target,
live-tab, pinned-group, and boundary validation therefore remain centralized in
the native executor.

This slice covers movement within the owning window. Cross-window live
reparenting remains a separate capability because each current Qt top-level
window owns a distinct `BrowserUi` and `WebEngineView`; a reload-based fallback
must not be reported as live detach.

Evidence:

- `cargo test -p browser-engine-qt --locked --offline`
- `cargo fmt --all -- --check`
