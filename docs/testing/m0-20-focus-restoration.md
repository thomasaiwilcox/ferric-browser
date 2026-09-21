# M0-20 focus restoration evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

Browser-owned overlays now capture the active focus item before taking focus
and restore it after dismissal when the item is still valid. If navigation or
tab destruction invalidates that item, the active WebEngine view is used as a
safe fallback. The same bounded mechanism covers:

- command and search mode transitions;
- the universal switcher;
- page JavaScript and authentication dialogs;
- permission and screen-sharing prompts;
- renderer recovery surfaces;
- download destination dialogs; and
- upload/save file pickers in primary, secondary, and popup windows.

Escape remains a cancellation path for the interactive overlays, and modal
surfaces explicitly request focus when they become visible. Teardown paths
only restore focus when they owned a captured request, so closing an unrelated
view cannot consume another overlay's focus state.

## Verification

```text
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Full AT-SPI screen-reader traversal and live
portal dialog focus evidence remain manual follow-up work.
