# M0-147 — Typed tab-close options

The V1 `tab-close` command now supports the documented forms
`tab-close`, `tab-close --id TAB_ID`, and `tab-close --count N`. The legacy
positional stable-ID form remains accepted for existing typed actions.

Targeted close preserves the existing live-tab validation and close lifecycle.
The default and counted forms close only live unpinned tabs, bounded to 100
targets; pinned tabs are skipped. CLI and typed IPC convert the optional ID and
count into the same validated command, and the core dispatcher emits typed
`Event::CloseTab` events for both targeted and bulk forms.

Qt uses a browser-owned pending action. Targeted close follows the existing
dialog, permission, file-picker, and view-closed cleanup path. Bulk close
reuses that path per tab, updates the native tab model after every close, and
reports how many tabs were actually closed.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-core --locked --offline` — 58 passed
- `cargo test -p ferric-browser --locked --offline` — 41 passed
- `cargo test -p ferric-browser-engine-qt --locked --offline` — 112 passed
- `cargo xtask check --locked`
- Wayland startup smoke with `--temp-basedir` — expected timeout, no error,
  panic, failed, or assert diagnostics
