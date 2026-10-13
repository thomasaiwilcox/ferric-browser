# Keyboard interaction review repairs

The three integration findings from the local-versus-remote review are repaired
in source commit `5410c038409a835d271a08b42e96ea6dd434d6f9`.
The repairs retain the existing architecture, default interactions and capability
gates. No dependencies, configuration schema or external protocol were changed.

## Corrected behavior

Native pointer dispatch rechecks the captured receiver, active surface, input
ownership and request immediately before the first press. Move-time invalidation
sends no button gesture. After a press is attempted, cleanup is limited to the
unchanged receiver and interrupted delivery reports uncertainty. Requests are not
retried. Ten component regressions cover blocking, disabling, replacement, resize
and deactivation during move and press. All ten failed before the repair and pass
afterward. The shared adapter serves Pointer Hint and Grid.

Selector scripts consume the canonical nested Rust policy through one accessor.
Inheritance, extension, replacement, empty replacement, implicit scrollables and
proxy exclusions use the same representation. Malformed/flat entries fail closed.
The QML fixtures now match production serialization, and real BrowserUi startup
cases verify discovery with accepted configuration. Matching a selector grants
no new semantic or native action authority.

Configuration publication distinguishes runtime updates from reload. Reload
activates live, navigation and reload settings; startup/restart changes remain
pending. Insertion and removal restore the intended selector policy at the proper
boundary. Rule lists resolve their wildcard catalog metadata at the list root.
Preparation and publication remain separate, with revision checks and persistence
before the state swap. Effective policy is resolved from the prepared active
configuration, and the validated selector fingerprint is published alongside it
before notifications. Invalid CSS, stale callbacks and unavailable parser
timeouts preserve the previous configuration.

## Verification

The fresh repository check passed 889 Rust and 161 QML cases. Strict workspace
Clippy, formatting, architecture, generated documentation, artifacts and license
checks passed. The pack validator regression suite passed all 14 cases.
The [validation ledger](review-repairs-validation.json) records exact commands,
source revision and logs. The packaged application passed 56 functional cases,
separate DPR 2 cases and 1,000 Hint lifetime cycles. The stress slot is intentionally
skipped in the functional run and executed separately. QObject counts returned
from 3368 to 3368; retained elements, observers, windows, scopes and tracking state
were empty. Native components passed 69 cases, and the real CLI/IPC probe passed.
The new packaged selector/reload cases also passed their focused run.

The application suite's total timeout is 300 seconds to accommodate its expanded
coverage. The product preflight lifetime remains 250 ms. UI validation ran after
compilation completed. Existing teardown context warnings remain in the raw logs;
no user-visible teardown failure was established by these tests.

## Qualification limits

These repairs close the three reproduced findings. Physical Wayland delivery,
authenticated live-site journeys, platform IME/choosers and human/screen-reader
qualification remain outstanding. Slice statuses remain conservative. Double-click
remains disabled and drag remains blocked. The previously measured DOM-to-native
observation interval remains an admitted limitation; observed Qt invalidation is
now guarded, but DOM targeting is not atomic.

Earlier performance samples remain historical evidence. This repair introduces
no new performance qualification claim. Nothing was installed, published or pushed,
and unrelated `aqtinstall.log` and root `tools/` were preserved.
