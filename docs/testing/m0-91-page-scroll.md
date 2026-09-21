# M0-91 — Page scrolling commands

The V1 page-scrolling commands are now registered and executable through the
native Qt adapter:

- `scroll up|down|left|right [--count N]` performs bounded incremental scrolling.
- `scroll-page up|down [--half] [--count N]` moves by a viewport-relative amount.
- `scroll-to top|bottom` moves the focused page container to an extremity.

Typed IPC accepts only the documented direction/edge enums, boolean `half`,
and counts from 1 through 9,999. The adapter validates the command again before
queuing a browser-owned JavaScript operation. The script selects the nearest
scrollable ancestor of the focused element, falling back to the document
scroller, and never receives arbitrary script text.

The default normal-mode bindings now cover `h/j/k/l` for left/down/up/right,
`gg` for top, and `G` for bottom. Primary and secondary windows execute the
same bounded payload through their owned WebEngine view.

## Verification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
