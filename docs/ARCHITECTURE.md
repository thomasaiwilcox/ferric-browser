# Ferric Browser architecture

Ferric separates policy from presentation. The core crate owns IDs, state,
commands, bindings, actions, navigation, and the reducer. Its application state
has no public mutable fields. The runtime crate owns that state and reduces
typed inputs. The application crate is the sole Qt-independent owner of the
runtime, effective configuration, and Rust-owned durable metadata work. The Qt
adapter receives application effects and executes only platform work.

```text
CLI / IPC / QML intent / engine fact / worker completion
                         │
                         ▼
       BrowserApplication typed domain methods
                         │
              ┌──────────┴──────────┐
              ▼                     ▼
       reducer and policy      configuration and storage orchestration
              │                     │
              └──────────┬──────────┘
                         ▼
        engine / storage / UI / diagnostic effects
                         │
                         ▼
                Qt and platform adapters
```

Qt callbacks translate facts and intents. They do not directly mutate core
state. Runtime commands enter through `dispatch_runtime`; configuration,
profile, context, and storage intents use their corresponding typed methods.
QML renders feature-specific model data and emits intent; WebEngine,
native dialogs, portals, Wayland integration, and effect execution stay in the
Qt adapter. Registry commands are reduced inside `BrowserApplication`;
the adapter observes typed effects and transports application-projected IPC
payloads. Browser-owned page scripts are resources with generated hashes.

The context-route, clean-link preview, and Site Doctor prompts cross the Qt/QML
boundary as typed scalar and list properties. Their JSON forms remain available
only for corresponding IPC response or diagnostic payloads, not as QML
application state.

The profile metadata worker owns its SQLite connection and exposes bounded,
typed completions to the application layer. `BrowserApplication` owns worker
lifetime and bounded retry admission for durable intents. The Qt adapter never
owns a storage sender, receiver, worker, or persistence retry queue; it only
submits typed application intents and renders their completion effects.

Profile activation transfers the profile lock, storage roots, and context
registry into `BrowserApplication`. Profile close and replacement use the same
bounded two-second drain: every accepted storage ticket ends in a completion,
submission failure, or explicit cancellation effect. Qt retains only immutable
profile/context projections needed to render platform UI.

The configuration crate owns the precedence and validation of profile,
persisted-runtime, command-line, and temporary override layers. The application
stores the resulting typed effective configuration; Qt only projects it into
presentation values and never reimplements configuration-layer policy. Reloads
commit atomically: immediate values become active together, while startup- and
restart-scoped values are reported as pending without partially mutating the
active configuration.

Architecture decisions live in `docs/architecture/`. New features must enter
through a typed registry or runtime input, mutate application state through the
reducer, and carry behavioral tests.
