# KDE removal migration evidence

Starting revision: `dc9bf4f047121bc399b0a87ceb451d58e0c8fd74` (clean).
Migration branch: `migration/kde-free-wayland`. This report records work in
progress; it is not a product qualification receipt.

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
latency increase exceeding both 5% and 1 ms fails qualification. Only diagnostic pilot receipts exist; no official performance qualification
has been claimed for this migration.

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
package remains incomplete. No merge or replacement of the user installation
has occurred.

The owned protocol context, generation-bound chooser, tagging and Rust native
witnesses are implemented. Production and native targets, CI, packaging and SDK
wrappers no longer discover, fetch or link KDE components. Historical
implementation-pack validators and accepted KF6 receipts remain unchanged. The
current qualification successor is `ferric.kde-free-wayland.v2`; no final v2
qualification receipt has been issued.

The explicitly unqualified [progress manifest](kde-removal-2026-11-04/progress.json)
records exact source, artifact and receipt hashes. Complete local evidence is in
`target/kde-free-evidence/qualified-1257101`. Earlier failed attempts, setup
failures and receipts under other input revisions are retained separately.

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

## Remaining qualification blocker

The inherited desktop is securely locked. Read-only Omarchy lock-service IPC
reports `locked=true`, `sessionLocked=true` and `secure=true`; logind's separate
`LockedHint` is false and did not reveal this condition. The service's direct
protocol state explains why accepted compositor focus requests cannot activate
new test windows. The unchanged frozen baseline now fails the same native-input
activation assertion as the candidate. Earlier pointer-only explanations were
incomplete; no desktop focus automation was established.

The current public-page-host report contains the known baseline credential
policy failure and an additional native-input activation failure. The latter
cannot use the baseline exception. The current everyday report passes 15 of 17
entries, including T100; T101 and T102 fail their owned-focus preconditions.
Their operation assertions are not bypassed. The final seven-row live product
run also remains pending. These gates require an unlocked session.

The frozen baseline's reproducible credential failure is
`siblingPolicyChangeRevokesTheSharedProfilesCapturedRequest`. Its command keeps
its actual nonzero status. The v2 verifier permits only this public-host baseline
comparison, with the original revision, matching SDK hashes, original binaries,
identical assertion sources, complete scoped inventories and exact failure
fingerprints. Native input, new bridge/freshness work, runtime and performance
cannot be exempted. Candidate public-host qualification is therefore still
blocked by the additional native-input failure.

A nested session using the exact same Hyprland binary was attempted with an
owned temporary configuration, isolated D-Bus, disabled session/environment
management and physical DRM session acquisition. It did not provide a usable nested output/keyboard
and is not qualification evidence. Its owned processes were stopped. No user
desktop packages or compositor configuration were changed, and no unlock was
attempted.

The six-workload performance pilot completes for both frozen products, including
twenty exposed, rendered windows, actual creation/recreation and idle. Restored
background pages normally stay unmapped; the explicit native fixture maps only
committed, registered owners for the twenty-window workload. Its bounded startup
work stops before idle sampling. The pilot is diagnostic and cannot qualify
performance. The required five alternating process pairs, ten warmups and 100
latency samples have not run. No performance-regression or final qualification
claim is made.

The user has been asked to unlock the desktop and choose whether to allow a
temporary, process-owned idle inhibitor for the several-hour measurement run.
A transparent, input-free, timer-free inhibitor fixture is prepared but has not
been launched. It would release its Wayland resource when its process exits and
would not change desktop configuration. Until those remaining receipts and the
successor verifier pass, this branch remains unqualified.
