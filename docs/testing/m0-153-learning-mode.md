# M0-153 — Per-window learning mode

Completed the documented learning-mode command surface.

- The command registry exposes `learning-mode [on|off|toggle]` with a typed
  state argument.
- Qt owns the live learning flag per browser window, defaulting it off and
  resetting it when transient profile resources are released.
- Binding feedback reads the per-window flag; enabling it does not change
  dispatch, key timing, privacy, or page input handling.
- CLI and Qt typed IPC preserve the state value and reject invalid states.

Evidence:

- `cargo fmt --all`
- `cargo test -p browser-core --locked --offline`: 59 passed
- `cargo test -p rustbrowser --locked --offline cli_binding_commands_use_typed_mode_and_keychain_fields`: passed
- `cargo test -p browser-engine-qt --locked --offline binding_commands_use_typed_mode_and_keychain_arguments`: passed
