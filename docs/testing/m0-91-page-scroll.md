# M0-91 — Page scrolling commands

The V1 page-scrolling commands are now registered and executable through the
native Qt adapter:

- `scroll up|down|left|right [--count N]` performs bounded incremental scrolling.
- `scroll-page up|down [--half] [--count N]` moves by a viewport-relative amount.
- `scroll-to top|bottom` moves the resolved page container vertically and preserves its horizontal offset.

Typed IPC accepts only the documented direction/edge enums, boolean `half`,
and counts from 1 through 9,999. The adapter validates the command again before
queuing a browser-owned JavaScript operation. With automatic targeting, the
script selects the nearest scrollable ancestor of the focused element on the
requested axis and falls back to the document scroller. `scroll-target select`
reuses `hint scrollables` to pin one visible element; `scroll-target document`
pins the document, `auto` restores focus-aware resolution, and `status` reports
the current policy. Explicit element identity remains in the isolated page
world through weak references and is revalidated before every scroll. Invalid
elements clear to automatic for the triggering operation; unsupported axes
remain no-ops. Commands never receive arbitrary script text or change DOM focus.

The default normal-mode bindings now cover `h/j/k/l` for left/down/up/right,
`gg` for top, and `G` for bottom. Primary and secondary windows execute the
same bounded payload through their owned WebEngine view. Policy changes and
status queries also dispatch to the owning primary or secondary view.

The expanded target-selection fixture and qualification matrix are recorded in
[`explicit-scroll-target.md`](explicit-scroll-target.md).

## Verification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```

For explicit target selection, the locked core/IPC/Qt and CLI test targets,
workspace verifier, warning-denied Clippy, generated docs, build, adapter smoke,
and formatting gates passed on 2026-09-26. The direct Chromium fixture matrix
is recorded in [`explicit-scroll-target.md`](explicit-scroll-target.md).
Native Wayland startup passed, but interactive qualification was not run
because the compositor lacks the virtual-keyboard protocol required for input
injection; this is not counted as a native interaction pass.
