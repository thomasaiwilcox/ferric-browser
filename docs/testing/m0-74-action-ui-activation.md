# M0-74 — Subject-aware UI action activation

## Scope

Browser-owned WebEngine context menus now activate the shared typed action
registry for link and selection subjects. The menu remains native and keeps
its existing engine-owned edit actions and focus lifecycle.

## Implemented behavior

- Link open, copy, and download items carry the corresponding stable action
  IDs and execute through the typed action boundary.
- Copy selection and search selection use browser-owned, one-shot selection
  extraction with the captured `BrowserUi` target.
- Primary, secondary, and approved popup views retain their owning UI and
  view when an action is selected; navigation and synthetic download requests
  are drained by that owner.
- Values passed from the context-menu request are still safe URL values, and
  the typed action path rejects unsupported UI action IDs.
- Undo, redo, cut, paste, delete, select-all, spelling replacement, and
  inspect remain direct Qt/WebEngine actions because they are engine editing
  operations rather than shared browser subjects.

## Qualification

Automated evidence:

```text
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```

The test suite covers the native context-menu action IDs and the typed action
executor. Live primary/secondary/popup context-menu activation remains.
