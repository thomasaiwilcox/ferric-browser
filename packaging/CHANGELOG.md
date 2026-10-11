# Changelog

## Unreleased

- `i` now enters Transient Insert: ordinary Enter reaches the page before
  returning to Normal. `I` enters Locked Insert for extended editing.
  Shift+Enter and other modified Enter combinations keep editing; Escape
  exits either variant. Focus and page/tab lifecycle changes invalidate stale
  editing callbacks. Fresh physical presses cannot inherit an earlier lost
  Enter release, including modified Enter and IME confirmation.
  Enter completion also survives focus moving to browser chrome or another
  window, without overwriting a newer editing session.
- CLI `mode-enter insert-locked`, binding queries without `--mode`, and tab
  queries without `--window` now
  produce valid IPC payloads. Window queries report the same canonical mode
  names as the keyboard and command references.
  Locked Insert is also accepted by configuration/runtime binding validation,
  typed binding commands and the existing binding-help projection.

The current development line contains the typed command/action registry,
bounded local IPC, durable session and profile storage, context routing,
diagnostics, and the system-Qt Arch packaging path. See the development
specification and testing records for qualification status and known limits.
