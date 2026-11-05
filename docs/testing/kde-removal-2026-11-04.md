# KDE removal migration evidence

Starting revision: `dc9bf4f047121bc399b0a87ceb451d58e0c8fd74` (clean).
Migration branch: `migration/kde-free-wayland`. Qualification completed on
2026-11-05 against Qt 6.11.2. The [v2 receipt](kde-removal-2026-11-04/qualification.json)
and [verifier result](kde-removal-2026-11-04/verification-result.json) record the
qualified product, including the explicitly preserved baseline credential-test
failure described below. All six work packages are complete.

## Frozen environment and harness

Host: Omarchy Linux, x86_64, existing Hyprland 0.56.2 session, commit
`efb50993780079460b0cbed1363e2166a2de1d9f`; no desktop packages or
compositor configuration changed. Rust: 1.98.0. Compiler: GCC 16.2.1.
SDK: `$HOME/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64`.
Baseline KWindowSystem: 6.28.0, locally built against that SDK under
`../builds/kwindowsystem-qt6112-F01b/prefix`. Test compositor: Qt Wayland
Compositor 6.11.2 under `../builds/qtwayland-qt6112-F01b/build`.
The system Qt 6.12.0 and KF6 6.31.0 are not the qualified baseline.

Baseline portal configuration uses `tools/performance/with_qt611.sh`,
`-DCMAKE_BUILD_TYPE=RelWithDebInfo` and an explicit
`-DQt6WaylandCompositor_DIR` pointing to that build's
`lib/cmake/Qt6WaylandCompositor`. Build parallelism: two. Runtime additionally
includes the matching prefix's `lib/qt6/plugins` in `QT_PLUGIN_PATH`.
The wrapper alone omits both discoveries: the initial configure failure and
initial missing-plugin chooser failures are setup receipts, not migration
regressions. Their logs are retained alongside the corrected run.

Corrected baseline: all three standalone portal CTest entries pass, with zero
failures or skips. This exercises the unchanged production controller and
chooser against owned D-Bus/Wayland fixtures, including native parent identity,
cancellation, late replies, shared exports, required/auto fallback and service
loss. Logs: `target/kde-removal-baseline/portal-runtime-qualified.log`.
All four workspace checks pass from a `git archive` of the starting revision under
`target/kde-removal-baseline/source`, with logs and command exit receipts in
`target/kde-removal-baseline/workspace-*`. The formatting, Clippy, workspace
tests and repository gate took 3.1, 938.7, 2428.2 and 1928.0 seconds respectively.
The repository gate includes 1,231 QML cases, with no failures or skips.

Baseline T100–T102 live identity/grouping/focus fixtures pass on the same
Hyprland build. The first T101 attempt failed its focus precondition; the
unchanged repeat passed. Both receipts remain in
`target/kde-removal-baseline/hyprland-baseline-T*.log`.

The installed release browser at source `5dfc36eca091a0957f226c8a4f57f6a0e68ffac4`
has identical production build inputs to the starting revision (the intervening
changes affect tests, harnesses and documentation). The complete input/hash
comparison is `target/kde-removal-baseline/product/source-equivalence.json`.
Its SHA-256 is
`cc5a6e2b7cb942e628d6889fc14ddc64aab49180229686e0a9cd3bcfe2932d8b`.
An exact copy provides the frozen optimized baseline without changing the user
installation. The additional native application baseline uses the starting
source's debug engine archive and that release CLI; its separate report records
this configuration and any failures. A broad application run has already
recorded a page-search timeout; it is not a candidate regression.

Application-level downloads, private/popup owners, keyboard operation, live
Hyprland grouping/focus and unsupported-compositor behaviour remain distinct
baseline and final qualification requirements. Standalone portal success does
not establish these cases. Existing historical evidence is preserved, including
the 2026-10-26 T100–T102 identity receipts referenced by
`ADR-everyday-window-identity.md` and the 2026-11-03 performance report; neither
is evidence bound to this migration's starting or final source.

Performance must use the existing harness conventions: five alternating
independent process pairs, ten warmups, 100 latency samples, one/twenty-window
startup, recreation and idle measurements. No compilation during measurements.
Baseline-only idle repetitions must establish variability first. An ordinary
latency increase exceeding both 5% and 1 ms fails qualification. The final full
comparison passes these unchanged limits; its controls, variability investigation
and exact results are recorded below. Earlier pilots and interrupted attempts
remain historical evidence.

## Dependency inventory

| Scope | Starting dependency and migration requirement |
| --- | --- |
| Production | `build.rs` discovers/links KF6WindowSystem; `native_file_chooser.cpp` exports with KWaylandExtras; `page_window_identity.cpp` tags with KWaylandExtras. Replace only after native proof. |
| Native suites | Application, portal, QML runner and public-page-host CMake targets discover/link KF6; everyday standalone compositor identity fixture directly calls KDE tagging. Migrate all to the shared owned bridge. |
| Packaging | `PKGBUILD`, `dependencies.toml`, `LICENSES.md` and README require/document KWindowSystem. Update current requirements and include protocol license provenance. |
| CI | `.github/ci/build-window-system.py`, its source manifest and `ci.yml` fetch/build ECM, plasma-wayland-protocols and KWindowSystem and set KDE prefixes/plugin paths. Remove active dependency paths after replacement. |
| Developer wrapper | `tools/performance/with_qt611.sh` requires a KDE prefix. Remove after migration; retain Qt 6.11.2 and declare scanner/client/private-header discovery. |
| Historical implementation pack | `docs/implementation/everyday-v1/tools/{validate_pack.py,a57.py,preflight.py,test_tools.py,test_a57.py}` and accepted control/evidence files encode historical KF6 ABI/link/runtime assertions. Preserve them under their original schemas. Add and route current qualification through an explicit successor. |
| Historical reports | Existing ADR evidence and `docs/testing/performance-2026-11-03` describe the then-qualified KDE implementation. Preserve attribution and receipts; identify them as historical where current instructions change. |

Asynchronous identity consumers found during inventory: `page_group_runtime.rs`
(admissions, pending jobs and retained leases), `page_window_focus.rs` (logical
focus leases), native chooser parent export, and diagnostics. Diagnostics must
keep their public shape. Group operations must corroborate all participants'
native freshness, including non-source participants, before dispatch. Logical
focus ownership must survive native recreation without granting old queued
operations new native authority.

## Preservation and proof gates

Logical tags, page-group IDs, prepared state, persistent data and public
configuration/CLI must remain unchanged. Native recreation must not hide a
logical window. Native eligibility requires a current generation and a submitted
tag on a supported backend. Rust authority invalidation is synchronous.

Native proof must exercise actual Qt hide/show role recreation and
QWindow destroy/create surface recreation, request-before-first-commit ordering,
shared consumers, delayed replies, cancellation, missing globals, shutdown and
churn. Tests may not use the untagged-window override as evidence of tagging.
Sanitizers and installed/runtime dependency inspection are separate final gates.
No final KDE-free claim is valid until all six work packages have passed.

## Native bridge proof

The owned compositor records actual Qt-owned surface/toplevel resources, tags
and commits. All four protocol variants pass: both globals present, either
absent, and both absent. The full variant verifies tagging before the first
surface commit, logical tag stability with fresh native generations through
actual hide/show role recreation and destroy/show surface recreation, shared
exports, individual cancellation, held/late completions, twenty alternating
recreation cycles with zero retained fixture resources, and shutdown with live
windows and pending/queued export work. These tests use no untagged override.

`target/kde-free-evidence/native-final.{log,xml}` records all eight standalone
native/portal CTests passing. `sanitizers-final.{log,xml}` records all five
native/control CTests passing under AddressSanitizer and UndefinedBehaviorSanitizer,
including graceful compositor teardown and leak detection. A single narrow
suppression matches Qt 6.11.2's `QWaylandDisplay::createSurface` allocation,
reproduced by the Qt-only successive-window control. Runs reproduce the same
96-byte Qt allocation in bridge variants and one or two in the Qt-only control.
The native churn run does not accumulate these allocations across recreations.
Export objects, subscriptions,
the context and owned fixture allocations remain checked. The unchanged
baseline chooser separately reproduced Qt GUI/Wayland backing-store leaks.

Native proof has passed; this permits dependent migration. It does not replace
the complete product, installed-runtime, Rust freshness and performance gates.

## Current implementation and bound source

The current product, test and qualification input revision is
`1257101f6eead8e369d2ced87fd773ad6850da5b`. Documentation-only commits do not
change the qualification input manifest. The six ordered work packages have
implementation commits on `migration/kde-free-wayland`; the final qualification
package is complete. The qualified build has been installed per user for testing;
the migration remains unmerged.

The owned protocol context, generation-bound chooser, tagging and Rust native
witnesses are implemented. Production and native targets, CI, packaging and SDK
wrappers no longer discover, fetch or link KDE components. Historical
implementation-pack validators and accepted KF6 receipts remain unchanged. The
current qualification successor is `ferric.kde-free-wayland.v2`; its final
source-bound receipt passes the unmodified verifier.

The [evidence index](kde-removal-2026-11-04/progress.json) records exact source,
artifact, accepted run and retained earlier receipt hashes. The v2 receipt and
its verifier remain the qualification authority. Complete local evidence is in
`target/kde-free-evidence/qualified-1257101`. Earlier failed attempts, setup
failures and receipts under other input revisions are retained separately.

| Work package | Review anchor |
| --- | --- |
| 1. Baseline and architecture decision | `1d815a4`; frozen controls and ADR-0021 |
| 2. Protocol generation and native bridge proof | `b381c11`; actual Qt lifecycle, protocol-ordering and sanitizer receipts |
| 3. Portal export replacement | `88cf915`; generation-bound shared exports and production chooser receipts |
| 4. Tagging and Rust freshness | `008728d`; actual source/peer recreation, grouping and focus receipts |
| 5. Dependency removal and qualification migration | `86412d2`; KDE-free build/runtime closure, preserved historical schemas and current v2 enforcement |
| 6. Complete qualification and policy | Final input revision `1257101`; the verified v2 receipt, full performance data and final evidence index linked above |

## Completed verification

An initial clean isolated release rebuild took 14 minutes 44 seconds. Its mount
namespace made installed KDE package files and the historical local prefix
unavailable, disabled networking, and kept host files read-only outside declared
build/cache directories. Production inputs have not changed since that build.
The final source-bound revalidation uses the same namespace policy and passes.
The actual package recipe stages 41 files, including the separately isolated
helper build, protocol notices and provenance. The installed helper check passes.
The current staging directory is `target/kde-free-install-1257101`.

The release and installed browser have SHA-256
`fdd8955dc8f5ec84e5316b4d57e9665887f883b9a96f3675a8c6002fea36bb4e`.
Static inspection finds no KDE or unresolved shared libraries, no KDE symbols,
and one definition of each required linked protocol interface. The installed
browser passes actual window creation, role/surface recreation, a real GTK
portal chooser, cancellation and clean shutdown. Window-mode and file-dialog
process maps use the qualified Qt libraries and contain no KDE components. The
GTK backend is independently inspected; these are installed-product receipts.

| Gate | Current-source result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | 1,304 passes; two existing ignored tests |
| `cargo xtask check` | Pass, including 1,231 QML cases |
| Standalone native/control CTests | Five pass, zero skips |
| Production portal CTests | Three pass, zero skips |
| ASan/UBSan native/control CTests | Five pass, leak detection enabled with the narrow Qt-only suppression |
| Application inventory | All 78 entries have actual successful results on unchanged source, including the complete reruns described below |
| Explicit QML run | 1,231 passes, zero skips |
| Wayland entry point | Pass; its nested Weston input probe explicitly reports not run because virtual keyboard support is absent |
| Blocking entry point | Pass, including actual blocked/exception behaviour |
| Qualification-tool unit tests | 15 passes |
| Public-page-host scoped inventory | All 21 entries exercised; only the exact preserved baseline credential failure remains; real native keyboard input passes |
| Everyday inventory | All 17 complete entries have actual successful unchanged-source results, retaining the two original locked-desktop failures |
| Final live Hyprland product rows | All seven pass, including identity, grouping, focus and private windows |
| Full performance comparison | All six workloads pass unchanged limits, including the resolved one-window idle controls |
| Final v2 verification | Pass; explicitly lists the unchanged baseline credential failure |

The Wayland smoke result is not proof of native keyboard input. The owned
protocol fixture separately proves real Qt surface/role access, tags before the
initial commit, shared exports, cancellation, late completion, recreation,
missing globals, shutdown and churn. These tagging tests use no untagged override.

The application report preserves the original 78-entry run and its failures.
Two live entries initially failed the initial seat-probe activation precondition;
complete unchanged-source reruns pass all seven focus-freshness and nine
identity/grouping rows. A controlled initial pointer position is recorded in
those commands; the harness makes no focus requests during the test bodies.
Together with the four owned source/peer recreation rows and five grouping
fence rows, all 25 native-only rows have actual Wayland coverage.

The initial full offscreen aggregate reports 895 passes, one five-second
navigation-recovery deadline failure and 40 scoped skips. The failing row passes
on its targeted retry. The subsequent complete serial aggregate rerun reports
896 passes, zero failures and the same 40 skips. Twenty-five skips are native
rows covered by the dedicated Wayland entries; the other 15 retain their existing
separate-scope requirements. Portable/offscreen success is not tagging proof.
The separate 1,000-cycle hint-lifetime entry initially observed 9,171 to 9,172 Qt
objects; its complete serial rerun reports 9,171 before and after, with all
JavaScript hint resources released. No assertion was relaxed and no source
changed between these runs.

`application-final-complete.json` and its fresh JUnit report derive coverage only
from actual successful complete CTest entries. The recipe checks the same source
before/after each run, the entire discovered inventory, all native row results,
and the original failure fingerprints against the untruncated transcript. All
original failed reports and logs remain intact; this is not a claim that the
first uninterrupted run had zero failures.

## Live qualification resumed on 2026-11-05

The user unlocked the inherited desktop and requested another qualification
attempt. The original, unmodified native keyboard-input entry now passes.
T101 and T102 both pass their complete live operation assertions, including
zero unintended focus events. All seven final product rows pass: native identity,
native grouping, grouping focus, grouping order, focus lease, reentrant focus
and private-window behaviour. Tagging tests use no untagged override and the
activation harness makes no test-body focus requests.

The complete 21-entry scoped public-page-host run now contains only the frozen
baseline's reproducible credential failure,
`siblingPolicyChangeRevokesTheSharedProfilesCapturedRequest`. Native keyboard
input passes. The failing command retains its actual nonzero status. The v2
verifier permits only this exact baseline comparison, with the original revision,
matching SDK hashes, original binaries, identical assertion sources, complete
scoped inventories and exact failure fingerprints. Native input, new bridge,
freshness, runtime and performance gates cannot use this exception.

The 17-entry everyday report retains the original 15 successful entries and
replaces only the two locked-desktop precondition failures with their actual
successful complete reruns on unchanged source. Both original failed receipts
remain intact. `everyday-attempts-resumed.json` records the recipe and provenance;
`live-resumed-complete.json` records all four resumed live runs. Preflight verifies
all 16 current-source functional, workspace, build and installation receipts.

The earlier blocker was the secure desktop lock: direct lock-service IPC
reported its actual locked state even though logind's `LockedHint` was false.
The frozen baseline reproduced the same activation failure while locked.
No desktop focus automation was established. The attempted owned nested session
did not provide usable input and contributes no qualification evidence. No
user desktop configuration or packages were changed.

## Performance qualification interrupted

The six-workload pilot is diagnostic only. The required full run started on
2026-11-05 against the frozen baseline and staged installed candidate. It uses
five alternating independent process pairs, ten warmups and 100 latency samples,
with baseline-only idle repetitions collected before candidate measurements.
No compilation runs concurrently. Restored background pages normally stay
unmapped; the native fixture maps only committed registered owners to exercise
twenty exposed, rendered windows. Its bounded startup work stops before idle.

The user explicitly authorized temporary idle inhibition. One process-owned,
transparent, input-free, timer-free pixel held a Wayland idle inhibitor during
testing; actual protocol creation and the owned layer were verified. It changed
no desktop configuration. This external
test resource uses the desktop's Quickshell; both measured products continue to
use the qualified Qt 6.11.2 SDK. Its metadata and source are retained under
`target/kde-free-idle-inhibitor`.

At 05:38:23 UTC the desktop became securely locked again despite the active
protocol inhibitor. The frozen baseline's shutdown confirmation could no longer
complete; the measurement driver exited with status 1 after its 40-second
shutdown deadline. Three baseline-only idle repetitions were written before the
failure; that attempt produced no candidate measurement or completed comparison.
The lock trigger was not established at that point. Omarchy's own idle service
reported its stay-awake state already enabled, so this evidence did not establish an ordinary idle
timeout as the cause. No lock state or desktop setting was changed by testing.

The failed run, raw files and environment samples are retained under
`target/kde-free-performance-1257101` and the current evidence directory.
`performance-lock-interruption.json` records the lock state, actual failed
command and temporary resource provenance. The inhibitor process was stopped;
no active owned layer remained. Hyprland temporarily reported its retired layer
snapshot with PID -1 while locked; final cleanup confirms it is gone. This attempt
remains unqualified and is preserved separately from the successful run below.

## Autonomous run on mains power

The later journal inspection found `Lid closed.` from logind at exactly
05:38:23 UTC, coincident with the lock request. The active Hyprland lid binding
invokes `omarchy-system-lid-close`, which locks directly when the lid closes
without an external monitor, independently of idle inhibition. The existing
battery lid policy also suspends. Ordinary Omarchy idle locking was already
disabled. `lid-lock-cause.json` records the binding, command and checksums. The
user requested autonomous completion and kept the lid open. No machine
configuration was changed.

A process-owned `systemd-inhibit` block for `sleep:idle:handle-lid-switch`
protected the benchmark and released automatically when it exited. It guards
system sleep/idle policy; the direct compositor lid-lock binding still requires
the lid to stay open. Its actual presence in logind was verified. The laptop was
initially discharging; after
mains power connected, sampling restarted entirely so accepted baseline and
candidate measurements use the same power conditions. Both earlier partial
attempts and their nonzero exit statuses remain preserved.

The full run uses `target/kde-free-performance-mains-1257101` and
`performance-mains.json` in the current evidence directory. It completed in
25,050 seconds (6 hours 57 minutes). Setup, inhibitor provenance, compiler
inactivity, power and lock-state observations are retained alongside it. The
session stayed unlocked on mains throughout. Baseline-only idle variability was
collected before candidate measurements.

## Final performance and qualification

The [complete performance receipt](kde-removal-2026-11-04/performance.json)
retains the exact products, probe, source manifest and hashed raw process
receipts. Each latency workload has five alternating independent baseline and
candidate blocks, ten excluded warmups and 100 measured samples per block.
Startup samples launch independent fresh browser processes: 2,200 launches
including warmups across the two startup workloads. The summary is the median
of the five per-block p95 values. Creation and recreation exercise actual product
windows; recreation alternates actual Qt role and surface transitions. Readiness
requires native exposure and a fresh rendered frame. No compilation ran
concurrently, and no untagged or missing-protocol override was inherited.

| Latency workload | Baseline p95, ms | Candidate p95, ms | Change | Limit |
| --- | ---: | ---: | ---: | --- |
| startup-1 | 1275.597 | 1254.615 | -1.64% | Pass |
| startup-20 | 13416.875 | 13210.600 | -1.54% | Pass |
| creation | 564.381 | 573.662 | +1.64% | Pass |
| recreation | 96.167 | 96.649 | +0.50% | Pass |

The initial one-window 20-second idle comparison was inconclusive against its
baseline-only range collected almost seven hours earlier. The paired frozen
baseline also exceeded that early range; the candidate was lower than the paired
baseline for both CPU and wakeups. The original verifier correctly rejected the
comparison, and its failed receipt remains intact. No limit or assertion was
changed.

A targeted reassessment retained all four passing latency workloads and the
passing original twenty-window idle workload. Five new baseline-only one-window
controls immediately preceded five alternating independent process pairs, using
60-second intervals for both products. This improves CPU counter resolution from
0.05 to approximately 0.0167 percentage points and reduces short-interval
variability. The new CPU medians represent the same counter-level usage; wakeups
are lower for the candidate. Both metrics fall within the fresh baseline range.
The twenty-window workload retains its original 20-second controls and pairs.
The script, real command, raw records and original rejection are retained under
`performance-idle-resolution.json` and
`target/kde-free-performance-idle-resolution-1257101`. This reassessment took
992 seconds. The unchanged verifier recomputes every accepted metric from its
hashed raw receipts and enforces the original range and chronology checks.

CPU below is total browser process-tree CPU as a percentage of one core; wakeups
are GUI event-dispatcher awake events per second. The controls column is the
observed range of five baseline-only repetitions, not a relaxed limit.

| Idle workload and metric | Paired baseline median | Candidate median | Baseline-only range | Limit |
| --- | ---: | ---: | ---: | --- |
| idle-1: tree_cpu_percent | 0.716609 | 0.716613 | 0.699925–0.716614 | Pass |
| idle-1: gui_wakeups_per_second | 10.430 | 10.364 | 10.214–10.647 | Pass |
| idle-20: tree_cpu_percent | 14.584 | 13.627 | 11.132–14.024 | Pass |
| idle-20: gui_wakeups_per_second | 87.889 | 85.837 | 64.411–94.865 | Pass |

All six workloads meet the unchanged successor limits. The final verifier passes
for product/qualification input revision
`1257101f6eead8e369d2ced87fd773ad6850da5b`, manifest SHA-256
`111f07ede765513f18812b323444c16428a734531542ddb2900c06d86ab6f9bd`, and installed
browser SHA-256
`fdd8955dc8f5ec84e5316b4d57e9665887f883b9a96f3675a8c6002fea36bb4e`.
Documentation-only delivery commits do not change those qualified inputs.

The exact pre-existing public-host credential failure remains explicitly listed
by the verifier. Earlier failed runs, full original transcripts and actual
unchanged-source complete entry reruns are preserved; qualification does not
claim that every first uninterrupted test attempt passed.

`cleanup-final.json` confirms automatic release of all temporary system
inhibitors, removal of the earlier pixel layer, and no remaining owned browser
or native fixture test processes. No desktop configuration or package was changed.
The qualified matching Qt 6.11.2 private native-window header and pinned protocol
inputs remain part of future SDK qualification under
[ADR-0021](../architecture/ADR-0021-owned-wayland-integration.md). All six work
packages are complete on the migration branch.

## Installed system test

On 2026-11-05 the qualified binary was installed in a versioned per-user release,
and the normal CLI/desktop launcher was switched to it using Qt 6.11.2 without
the previous KWindowSystem library/plugin prefix. The previous release and
changed installed resources have a rollback copy. The
[deployment summary](kde-removal-2026-11-04/deployment.json) records the exact
binary hash and the local installation receipt hash.

The normal launch mapped a native Wayland window with the existing GTK platform
theme. A public CLI query succeeded; all four required Qt library hashes matched
the qualified SDK, and runtime maps/shared-library inspection found no KDE
components. The relocated phone-auth helper admitted a disposable request with
network transports disabled, proving its installed suffix-data lookup. Host
packages, profile configuration and default-browser ownership were not changed.

The [frozen recipes](kde-removal-2026-11-04/recipes/README.md) preserve the exact
complete-entry report assembly, idle reassessment and final receipt construction
for review. Their manifest records original paths and checksums; current product,
build and qualification tooling remains unchanged.
