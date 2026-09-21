# Ferric Browser architecture

Ferric separates policy from presentation. The core crate owns IDs, state,
commands, bindings, actions, navigation, and the reducer. Its application state
has no public mutable fields. The runtime crate accepts typed inputs, invokes
the reducer for the extracted paths, and returns effects plus bounded snapshots.
The Qt adapter currently uses a documented compatibility seam while remaining
command and worker orchestration is migrated into runtime ownership.

```text
CLI / IPC / QML intent / engine fact / worker completion
                         │
                         ▼
                BrowserRuntime::handle
                         │
              ┌──────────┴──────────┐
              ▼                     ▼
       reducer and policy      storage orchestration
              │                     │
              └──────────┬──────────┘
                         ▼
        engine / storage / UI / diagnostic effects
                         │
                         ▼
                Qt and platform adapters
```

Qt callbacks translate facts and intents. They do not directly mutate core
state. QML renders presentation data and emits intent; WebEngine, native
dialogs, portals, Wayland integration, and effect execution stay in the Qt
adapter. Browser-owned page scripts are resources with generated hashes.

Architecture decisions live in `docs/architecture/`. New features must enter
through a typed registry or runtime input, mutate application state through the
reducer, and carry behavioral tests.
