# M0-19 accessibility semantics evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

The Qt Quick chrome now publishes explicit accessibility metadata at the
native control boundaries most likely to be ambiguous to assistive technology:

- the tab strip is a `PageTabList`, each tab is a `PageTab`, and the active
  tab exposes selected state and a concise description;
- command completion is a named `List` with selected `ListItem` rows rather
  than duplicate child text announcements;
- address, command, and search fields expose editable-text roles;
- primary and secondary status bars expose the `StatusBar` role; and
- permission surfaces expose the `Dialog` role and a stable name.

Existing `Accessible.name` values remain on buttons, prompts, download
controls, capture controls, and management surfaces. State is also conveyed
textually where it affects security or privacy, so color is not the only
signal.

## Verification

```text
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after remaining
alive for the full interval; absence of QML load errors verifies instantiation.
Actual AT-SPI/screen-reader event and focus-order qualification remains a
manual follow-up requirement.
