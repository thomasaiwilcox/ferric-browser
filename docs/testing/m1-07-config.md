# M1-07 implementation evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: CONFIG-001, CONFIG-002, CONFIG-003, CONFIG-005, CONFIG-006, and
  the include portion of CONFIG-004.
- Files: `crates/ferric-browser-config/Cargo.toml`,
  `crates/ferric-browser-config/src/lib.rs`, `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, workspace `Cargo.toml`/`Cargo.lock`,
  and `docs/requirements.csv`.
- Observable result: a Qt-independent TOML configuration model exposes typed
  starter sections and defaults. Unknown fields, unsupported enum values,
  unsafe search templates, out-of-range numeric settings, and globally allowed
  screen capture are rejected. Root configuration values override recursively
  merged local includes. Includes are bounded to depth 8 and 2 MiB, resolve
  relative to their parent, reject cycles/repeated imports and environment
  expansion, and retain source paths. `ConfigStore` increments its revision
  only after a complete candidate loads and keeps the previous valid candidate
  when a reload fails.
- Profile definitions are loaded from a sibling `profiles.toml`; at most one
  default profile is selected and its typed overrides are applied before
  generated runtime layers. Invalid profile files reject startup/reload
  without changing the last-good configuration.
- Live reload validates a sibling `contexts.toml` before committing a
  candidate, matching the existing `config-check` validation boundary; a
  malformed contexts document therefore cannot replace the last-good config.
- Site rules are bounded to 256 validated HTTP(S) glob entries, resolve by
  priority then source order, and expose effective site-scoped JavaScript,
  image loading, autoplay gesture, dark mode, and zoom settings to the Qt
  views. `config.get --url URL --explain` reports the winning rule and its
  contributors.
- Site-rule capability is centralized in `setting_supports_site_scope`; a
  URL-qualified `config.get` for a profile/global-only setting is rejected,
  and supported-scope metadata is included in the response.
- The shared `get KEY [--url URL] [--explain]` command now forwards typed
  key/URL/explanation fields through CLI, interactive, and IPC paths and uses
  the same effective-value resolver as `config.get`.
- `config.get --explain` includes the authored, profile, persistent runtime,
  CLI, temporary, and matching site-rule contributors present for the key,
  marks the winner, and reports lifecycle-bound pending values separately from
  the active value.
- The setting registry now declares every static leaf setting's type, default,
  supported scopes, apply time, prerequisite, and sensitivity. Dynamic
  `search_engines`, `action_targets`, and binding namespaces have bounded
  metadata entries, and `config.get` exposes the matching declaration.
- Runtime overrides are stored in a separate bounded
  `runtime-overrides.toml`. The store provides private atomic writes using a
  temporary file, syncing and rename; typed application rejects unknown keys
  and wrong types, and `--set KEY=VALUE` participates in startup precedence
  without rewriting the user config. Temporary profiles do not read
  persistent overrides.
- Interactive `set KEY=VALUE`, `set --temp KEY=VALUE`, `unset KEY`, and
  `unset --temp KEY` use the same typed layer. Persistent changes are saved
  only after the candidate validates; failed saves leave the in-memory
  configuration unchanged. Private profiles remain memory-only, and unset
  recomputes from the base layer so lower-layer values return.
- Interactive `bind [--mode MODE] KEYCHAIN COMMAND` and
  `unbind [--mode MODE] KEYCHAIN` use the same durable override document.
  Binding commands are parsed and registry-validated before commit, and
  unbind writes an explicit empty marker that masks lower-layer bindings.
- `config-export PATH` serializes the effective typed configuration as a
  reviewed TOML document, creates only a new destination with private mode,
  and refuses to overwrite an existing path.
- The GUI watches the authored root, resolved include sources, and their parent
  directories through a bounded poll/debounce loop. Linux uses inotify as a
  prompting signal and the same bounded metadata poll remains as a fallback,
  so atomic replacement and missing-then-created sources are observable even
  when native notification setup is unavailable. A valid candidate applies
  live and next-navigation settings immediately while retaining reload-,
  startup-, and restart-scoped values as explicit pending changes; it updates
  the effective configuration and base snapshot without reloading pages. An
  invalid or partial candidate leaves the previous live configuration intact
  and reports the rejection.
- Evidence: `cargo test -p ferric-browser-config --locked --offline` (36 tests),
  `cargo test -p ferric-browser-engine-qt --locked --offline` (238 tests),
  `cargo test -p ferric-browser --locked --offline` (84 tests),
  `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`,
  the workspace `cargo xtask check`, and
  `cargo xtask test config` (disposable nested-Wayland atomic-reload smoke).

## Limitations

Richer profile/context management and broader installed-environment
qualification remain later slices. The profile manager
can open profile-aware secondary windows; each such window recomputes its
effective configuration from the selected profile layer.
Semantic theme tokens are covered by M1-09. Site patterns use the bounded CONFIG-006
scheme/host/port/path matcher, including apex-safe leading wildcards and
query/fragment-free path matching. No loaded setting executes a command or
performs network access.
