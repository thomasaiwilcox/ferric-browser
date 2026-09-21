# M4-04 Omarchy theme/provider evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirements: OMA-001 through OMA-003.
- Files: `crates/browser-config/src/lib.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: `theme.source = auto` prefers a valid active Omarchy
  `colors.toml` under the current state layout, falls back to the older
  configuration layout, and otherwise uses the builtin palette. A nonempty
  `theme.path` always wins and may name either a palette file or its directory.
  Validated palette colors are passed to the native chrome as bounded JSON and
  update on configuration reload and event-driven replacement/modify events
  from the active palette file and its parent directories.
- The serialized palette also includes `provider_layout`, `provider_version`,
  and `provider_commit`. The version is read from the bounded installed
  `/usr/share/omarchy/version` file; commit metadata is reported only when a
  trusted checkout exposes a direct 40-character `HEAD`, otherwise it remains
  empty rather than being guessed.

## Safety boundaries

The provider reads only TOML data and never sources shell files, runs theme
helpers, or executes downloaded content. Palette files are bounded to 256 KiB;
colors must be `#RRGGBB` or `#RRGGBBAA`, and missing supported tokens use
deterministic builtin fallbacks. Invalid auto-discovered providers are skipped
in favor of the builtin palette; invalid reloads preserve the last good palette
and do not reload pages.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p browser-config --locked --offline`
- `cargo test -p browser-engine-qt --locked --offline`
- `cargo build -p rustbrowser --locked`
- `cargo xtask check`
- `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The focused suites cover current-layout precedence, explicit-path precedence,
missing-token fallbacks, invalid-color rejection, and the shared Linux inotify
watcher used for atomic replacement. The active development session exposes a
valid current Omarchy `colors.toml`; visual contrast qualification and
launcher/package integration remain follow-up work.
