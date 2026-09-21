# M0-24 searchable help map evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

The browser now exposes a native, searchable keyboard-help surface. Its rows
come from the active validated binding resolver and the shared command
registry, so each applicable mode shows effective key sequences, command
description, source layer, count policy, and unbound commands. Prefix
ambiguities are surfaced as bounded conflict rows with the configured chord
timeout. The typed `bindings.query` IPC response carries the same command,
unbound, and conflict data for display-free clients.

The surface is accessible, read-only apart from its search field, takes focus
while visible, closes through Escape or a named action, and restores the
captured browser target. It does not inspect page state or use a privileged
WebChannel. Display-free clients can request the same effective map through
`bindings.query`, and `bindings.explain` resolves a supplied keychain with its
current mode, optional count, exact/prefix result, continuations, timeout,
conflicts, source layer, and reserved Escape constraint.

After the existing chord delay, incomplete valid keychains also appear in a
bounded non-modal continuation overlay. It is visual-only, leaves focus with
the page/browser surface, and disappears on resolution, rejection, or Escape.
When `discovery.learning_mode` is enabled, successful bindings add the
registered command name to the status feedback; the mode does not alter
dispatch, timing, privacy, or keystroke retention.

## Verification

```text
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Interactive search and assistive-technology
traversal remain manual follow-up work.
