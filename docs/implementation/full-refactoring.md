# Application boundary refactoring

Delivery: one coordinated branch, `refactor/application-boundaries`. Protocol 3
replaces protocol 2; persisted profile, configuration and session formats remain
unchanged. The accepted conversation plan is the scope of this work.

## Checkpoints

- [x] Baseline and behavioral contract inventory
- [x] Shared command orchestration and typed errors
- [x] Application-owned bounded operation ledger and protocol 3
- [x] Typed, identity-preserving deferred native requests
- [x] Application-owned asynchronous configuration and userscript catalogs
- [x] Feature-owned Qt state and composed QML window runtime
- [x] Storage, CLI, tooling, snapshot and transitional-code cleanup
- [x] Release-linked full application and finalization regression qualification
- [x] Remaining native release matrix (passing evidence across recorded runs)

## Baseline

The six Qt-independent crate suites and formatting passed during the review.
The Qt library test build was interrupted during an I/O-stalled debug link; it
is not a passing baseline. Use a separate build directory with debug information
disabled for development checks on this machine; keep release settings intact.

## Required contracts

- Core mutation remains reducer-only; native/page inputs cannot assert authority.
- Accepted operations have exactly one terminal result; active work is never
  evicted to make room for retained history.
- Deferred document work retains its tab, generation and document identity.
- Settings persist before publication; post-rename sync uncertainty is committed.
- Profile retirement cannot publish into a replacement owner.
- Private and ephemeral metadata remains memory-only.
- Both window shells execute the same native request contracts.
- Existing database/session fixtures reopen without a data reset.

## Validation log

Implementation checks and remaining limitations are recorded here as each
checkpoint is integrated. A checkbox is completed only after integration and
its relevant checks pass.

### Integrated changes

- The command registry identifies common executors. UI, bindings, actions and
  IPC use one native command execution path for the shared command families;
  domain failures no longer depend on IPC error definitions.
- Runtime dispatch emits revisions and typed event-time observations. Full
  snapshots are explicit; IPC event serialization belongs to the adapter.
- The application owns a bounded operation catalog with admission before side
  effects, irreversible terminal transitions, separate output budgets and an
  exactly-once completion drain. Protocol 3 rejects version 2 handshakes and
  exposes bounded cursor pages. Alias expansion retains provider operation IDs.
- Native document requests own their arguments and exact target identity. Both
  window shells share their executor. Reload, history, search, clipboard edits,
  scrolling, zoom, printing, saving, source view, developer tools, JavaScript and
  Site Doctor use this contract. Live tab transfers retain stable source IDs.
- Configuration persistence runs on an owned worker before publication. Shared
  state-root revisions and reservations protect writes across named profiles and
  windows, including a writer whose observer retires. Temporary and CLI overrides
  remain local. Typed configuration is authoritative; JSON values are adapter
  presentation projections.
- Userscript parsing, discovery, installation, source loading, immutable catalogs
  and mutation workers belong to the application. Source/metadata limits are
  enforced while reading; root-scoped mutation reservations and profile leases
  outlive a retired observer. Navigation/action lookup uses the cached catalog.
- The Qt bridge declarations are isolated from ordinary Rust changes. Feature
  state groups own transient navigation, process, configuration I/O, library,
  interaction and diagnostic data. The primary QML window composes explicit-host
  controllers, replacing six inherited runtime layers.
- Storage separates transport, types, validation and execution. Nested payloads
  are checked before queue admission. CLI transport quotes through the browser
  command grammar. xtask separates architecture, documentation, native/desktop
  tests and packaging. Source guards check boundaries rather than file counts.
- Core invariant validation now builds membership once, avoiding a quadratic
  tab/window membership scan while retaining duplicate/orphan checks.
- Native handoffs remain running until their receipt arrives. Selection capture
  and its child process retain one operation ID; late receipts cannot revive a
  terminal operation. Public operation failures retain their typed category.
- Userscript watches retain the last validated catalog during refresh, observe
  changes even during a pending configuration reload, and reconcile after the
  bootstrap-to-watch handoff. Private sessions resolve the installed script root
  independently of a custom configuration file location without opening durable
  profile storage.
- Library mutation feedback has explicit typed string inputs. Configuration
  binding tests wait for the durable publication receipt before sending a key.
- Reconciliation of unchanged settings and publication of unrelated bindings
  preserve hint collectors. Only changes to hints, input policy or site rules
  revoke their configuration authority. Background inventory errors publish in
  Settings without overwriting the accepted userscript mutation's receipt.
- Startup configuration is accepted only before profile activation. Secondary
  windows no longer feed live primary-window JSON back into their startup API;
  their own watchers and application-owned persistent layer handle updates.
- Fuzz targets depend only on the Qt-independent crates. The userscript target
  uses the application parser; the hints fixture includes current ownership and
  presentation fields.

### Dispatch review corrections

- Restored registry mode validation for generic `spawn` dispatch before any
  process or userscript side effect. Native action entry points retain their
  deliberate eligibility rules, including selection actions from Insert mode.
- Restored the successful early return for `scroll-target select`; its chain
  suffix is discarded while interactive selection owns input. Other
  scroll-target policies continue the chain and invalid arguments reject it.
- Added native regressions for all ten modes and five scroll-target cases, and
  explicit Insert-mode coverage for native external selection actions. Before
  the fix, the new cases reproduced eight unauthorized-mode launches and the
  selection-chain failure; the six permitted/control cases passed. The corrected
  release action-feedback gate passes all 40 cases without failures or skips.
- Ignore the installer log and Python caches so generated clutter is excluded
  from future commits without deleting local evidence.

### Qualification

- Domain checks: application 121 unit + 5 integration; configuration 66; core
  156; IPC 34; runtime 7; storage 103; xtask 20 — all pass.
- Final `cargo xtask check` passes, including architecture/authority guards,
  QML lint, generated documentation, release inputs, license policy, formatting,
  native profile-lifetime/download-source tests and locked workspace tests.
- Strict workspace Clippy passes with all targets and warnings denied.
- All fuzz binaries compile against the locked offline dependency graph. The
  follow-up also executes all 12 targets for 1,000 runs each without crashes or
  timeouts. This is uninstrumented corpus/mutation smoke, not a coverage-guided
  or sanitizer campaign.
- QML interaction suite: 631 pass, no failures or skips.
- The final Rust workspace passes: Qt 493, CLI 111, and the domain checks above,
  for 1,116 passing tests and two intentional opt-in ignores.
- The captured native request regression passes. Window-routing, Switcher
  privacy and selection identity/cancellation/replacement IPC probes pass.
- All 130 native application methods were exercised in two isolated groups.
  Every initially failed application method passes in the final focused replay
  (24 cases, no failures or skips), including additional window-settings,
  startup-input and held-confirmation regressions. Focused popup, copy-feedback
  and registered-action regressions also pass. Context reload passes 33 cases.
- The initial download replay has 63 passes, one redirect-count failure during
  compilation and seven expected finalization skips. The follow-up serial full
  download gate passes 64 cases plus those seven skips within its standard
  timeout. Runtime compatibility and isolated native notifications pass.
  The finalization heartbeat harness was corrected after release diagnosis;
  ten consecutive runs now pass, and an injected GUI stall correctly fails.
- Serial release measurements improve median core/runtime reload dispatch by
  1.9× / 6.6× / 19.8× at 100 / 1,000 / 5,000 tabs. These are dispatch measurements
  on one host, not page-load or UI performance claims.

### Qualification limits

The original combined native CTest watchdogs expired during concurrent native
compilation. A follow-up serial run without builds passes the standard download
gate (64 passes, seven expected skips; 146.76 seconds) and context-reload gate
(33 passes; 60.06 seconds). The full application gate still exceeds 480 seconds:
it reports 198 passes, nine expected skips and no assertion failures before the
timeout. All 130 methods have now been exercised: the two serial remainder
groups pass 117 and 68 cases, with one expected opt-in skip and no assertion
failures. Across the interrupted run and these groups there are 379 distinct
passing cases (including setup/cleanup) and ten distinct environment/opt-in
skips. These native binaries link the unoptimized development library; the
official application runner builds a release library. Release-linked testing
was subsequently performed for the complete application and finalization gates,
as described below.

The follow-up passes both 1,000-cycle lifetime tests and private Wayland clipboard
transfers with compositor-delivered input. With the dedicated notification,
lifetime and clipboard environments included, all 389 distinct application case
names pass, including setup/cleanup; none of the ten initially skipped cases
remain unexercised. This does not make the timed-out monolithic CTest gate green.

A fresh standard release build and native relink now qualify the complete
application gate: 379 passes, zero failures and ten expected skips in 974.57
seconds. The aggregate CTest budget is corrected from 480 to 1,200 seconds;
individual assertion deadlines are unchanged. The subsequent dispatch-review
pass qualifies the other 23 registered CTest gates against the corrected release
library, including 40 action-feedback cases, both 1,000-cycle lifetime tests,
and all portal gates. IPC/window-routing and private Wayland clipboard probes
also pass. The earlier full-application and finalization evidence is retained;
those two gates were not repeated after the localized dispatch fixes.

Release diagnosis reproduced the finalization failure and showed that both
animation-clock coupling and the fixed 120 ms sampling window made the heartbeat
fragile. The fixture now requires five native GUI-thread callbacks within the
existing one-second command-response budget, checks actual elapsed time, and
verifies the file-sync barrier remains held. Ten consecutive runs pass all nine
cases. A temporary 1.1-second GUI stall correctly failed the assertion and was
removed before final validation. No production download code changed.

The remaining-matrix run initially failed one Switcher fixture after rapid tab
creation: tab-count publication preceded native activation and its subsequent
setting command did not complete submission. After adding an owned-view/frame/
load readiness check, three complete Switcher runs pass all 11 cases. The initial
failed invocation and successful replays are recorded in the report; this does
not claim a single fresh all-green invocation of all 25 CTest gates.

Physical keyboard input, hardware rendering, real desktop notification delivery
and coverage-guided/sanitizer fuzzing remain outside this qualification.

See [the qualification report](../testing/full-refactoring-2026-10-19.md) for
commands, native replay details and raw benchmark samples.

### Deliberate boundaries

- Window/feature presentation intents remain adapter-local and separate from
  captured document requests. The window facade is retained for secondary and
  popup shells; controllers own its behavior through explicit host references.
  The primary window and several controllers remain large, with substantial
  forwarding APIs. Composition improves dependencies but does not complete UI
  simplification; further decomposition should follow concrete ownership needs.
- Native facilities retain private diagnostic strings at their boundary; domain
  and public IPC failures carry typed categories. Durable storage retains its
  explicit drain/flush lifecycle; owned read/configuration/script workers retire
  without blocking on unfinished filesystem work.
- Historical implementation evidence keeps its original source paths. Current
  requirement references and architecture contracts use the new ownership map.

See [ADR-0020](../architecture/ADR-0020-application-boundaries.md) for the final
ownership and compatibility contracts.
