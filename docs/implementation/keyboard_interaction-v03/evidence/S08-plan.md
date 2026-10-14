# S08 implementation outline and evidence map

## Integrated boundary

Publish `input.shortcut_policy` and `input.page_keys` through the existing
typed configuration and registry contract. Validate both global values and
site-rule values with the S07 logical-token policy, and apply configuration
changes through the existing atomic `ConfigurationLayers` transaction.

Resolve a synchronous Rust ownership decision from the active policy, current
mode, and the binding resolver's pending count/keychain state. Call it in the
shared QML key handler before Normal-mode command entry, search, start-page,
hint, caret, search-next, and ordinary binding special cases. The route remains
the original Qt event to its existing recipient.

Extend `FerricBrowserKeyRouter` gesture bookkeeping with page-owned identities.
The first QML decision marks the current physical gesture; repeats and release
remain page-owned even if policy, focus, or input epoch changes. Fresh presses
replace stale identities. Accepted browser-owned repeats retain their route as
well. This reuses the single application event filter and does not synthesize
or queue key events.

Extend `binding-explain` with the same effective ownership decision, setting
source, reservation, pending browser sequence, QML-special handling, and any
Normal binding prefix made unreachable by the exact page mapping. Primary and
secondary windows will pass their own router into the shared handler.

## Planned source changes

- Config schema, validation, setting registry, runtime site-rule validation,
  configuration tests, and generated documentation.
- Core binding resolver pending-count/keychain introspection and config's
  dependency on the pure S07 policy model.
- Qt BrowserUi policy resolution, live config/site updates, synchronous QML
  decision invokable, `binding-explain` provenance, and tests.
- Existing `browser_key_router.h/.cpp`, the shared QML runtime key handler,
  primary/secondary router wiring, and native router regressions.
- This plan, `evidence/S08.json`, `evidence/progress.json`, and S08 test logs.

## Tests and evidence

| Test | Assertion | Evidence class |
| --- | --- | --- |
| T021 | Reject invalid, duplicate, reserved, sequenced, and oversized keys; preserve token conventions, layer replacement/inheritance, and atomic validation. | Pure unit |
| T022 | Exact mapped `/`, `?`, `f`, and `Ctrl+f` go to the existing page recipient; reserved and recovery controls stay browser-owned; baseline remains with no mapping. | Native Qt offscreen; Wayland when available |
| T023 | Pending count/keychain wins; mismatch, timeout, and config reset never replay a prefix or suffix; fresh gesture uses the new policy. | Pure unit; native Qt offscreen |
| T024 | Press, repeat, overlap, release, lost-release replacement, delayed renderer copies, and epoch changes keep one physical gesture's owner. | Native Qt offscreen |
| T025 | Existing Escape, pass-through, Insert, Hint, Grid, and both-window dispatch remain intact. | QML component; native Qt offscreen |
| T027 | A policy/focus update affects new gestures and cannot retarget a held gesture. | Native Qt offscreen |
| T045 | Existing QML/WebEngine interaction suites remain unchanged and pass. | WebEngine fixture; native Qt offscreen |
| T055 | Registry, runtime values, site rules, IPC/help discovery, and generated docs agree. | Pure unit; repository check |

Record unavailable display-server evidence as `not_run` with its actual
environment reason. Do not claim the workspace test command exercises native
event routing unless the native target was explicitly run.
