# M0-156 — Hint target completion

Implemented the remaining V1 link-hint targets named by the specification:

- `tab` opens a validated hinted link in a new foreground tab.
- `window` queues a validated hinted link for a new same-profile window.
- `clean-yank` applies the active link-cleaning rules before writing the
  selected link to the clipboard and reports whether cleaning changed it.
- Typed IPC and command parsing accept the targets with the documented rapid
  constraints; foreground tab/window navigation cannot be combined with
  `--rapid`, while clean-yank requires it.

The existing `tab-bg`, `yank`, `download`, `userscript`, external-target, and
ephemeral hint paths remain unchanged. Live cross-window tab reparenting is a
separate capability boundary and is still reported honestly as unavailable.

Evidence:

- `cargo fmt --all`
- `cargo test -p ferric-browser-engine-qt --lib ipc_hint_commands_use_a_bounded_kind_selector`
  — passed
- Full workspace gate and native smoke verification completed after the slice.
