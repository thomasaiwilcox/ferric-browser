# M0-70 — Registry-generated action examples

Each built-in action now owns bounded example command text in the shared core
action registry. The same examples are exposed through `actions.query`, native
`action-list`, and the library presentation, so documentation and discovery do
not depend on parser-only special cases. Example text is static registry data;
it is never executed or reparsed as a command during discovery.

Covered examples include URL open/copy/clean/explain, link open/copy/clean-copy/
download, selection copy, foreground/background tab open, tab
back/forward/reload/stop, scrolling, tab traversal, tab clone/fullscreen, and
tab undo, validated live tab transfer, and normal/private window creation with
optional profile selection, plus bookmark and quickmark creation and named session save/load/delete.
URL and link open actions accept
the same bounded `--target` values as the typed IPC route, including
`tab-bg`; invalid targets fail before dispatch. The existing typed action
executor continues to validate the actual invocation separately.

The project gate also validates the registry schema before generating or
serving metadata: IDs must match their declared subject/verb, identities and
arguments must be unique, labels/descriptions and examples must be present,
every action must declare at least one invocation source, and every executor
command must resolve in the canonical command registry. Namespaced typed IPC
actions additionally reject undeclared argument names and missing required
arguments before translating into the broader command schema. The same gate
also rejects sensitivity drift between an action and its executor command;
effect class remains action-facing metadata because a sensitive navigation
action can legitimately use a sensitive executor.

The native adapter regression
`every_builtin_action_example_maps_through_typed_parser` parses and resolves
every example for every built-in action, rather than only the first example in
each definition. This keeps alternate forms such as background tabs, reload
bypass, optional counts, private windows, and typed stored-entry arguments
aligned with their registered executor.

Evidence:

* `cargo test -p browser-core -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
