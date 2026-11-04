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
latency increase exceeding both 5% and 1 ms fails qualification. No measurements
have yet been claimed for this migration.

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
reproduced by the Qt-only successive-window control. The final run reports one
96-byte allocation in each native variant and two in the Qt-only control.
The native churn run does not accumulate these allocations across recreations.
Export objects, subscriptions,
the context and owned fixture allocations remain checked. The unchanged
baseline chooser separately reproduced Qt GUI/Wayland backing-store leaks.

Native proof has passed; this permits dependent migration. It does not replace
the complete product, installed-runtime, Rust freshness and performance gates.

## Current source and completed build gates

The current product/test/qualification source is
`2a765e2143ea5bc8046a7fb1b82e2319b2a055b1`. Later evidence-only documentation
commits do not change the qualification input manifest. The protocol context,
generation-bound chooser, tagging and Rust freshness implementation are in
place. Current production and native targets, CI, packaging and SDK wrappers
have no KDE discovery or linkage. The historical implementation-pack validators
remain unchanged; the successor is `ferric.kde-free-wayland.v2`.

An isolated release rebuild completed in 14 minutes 44 seconds with KDE package
files and the historical local prefix unavailable, networking disabled, and
host files read-only outside the declared build/cache directories. Only test
fixture inputs changed during that build; compiled product inputs were
unchanged. The final source-bound revalidation passes with the same namespace
policy. Its inventory and exact command are in
`target/kde-free-evidence/qualified-final/fresh-build*`.

The actual package recipe staged 41 files under
`target/kde-free-install-qualified`, including the separately built helper and
protocol licenses/provenance. The installed helper check passes. The release
and installed browser have the same SHA-256:
`fdd8955dc8f5ec84e5316b4d57e9665887f883b9a96f3675a8c6002fea36bb4e`.
This staging does not replace the user's installation.

All four requested workspace checks pass on that exact source. The repository
gate includes 1,231 passing QML cases. Five native/control CTests, three portal
CTests and five ASan/UBSan CTests pass with fresh source-bound receipts in
`qualified-final`. The sanitizer command explicitly enables leak detection and
uses only the documented Qt surface suppression; the first unsuppressed failure
is retained. The typed native snapshot and private generation witness do not
change persistent data or the serializable compositor identity shape.

## Baseline comparison and test corrections

The quiet baseline comparison passes all nine remaining application/portal
entries: page search, chooser backend/privacy, portable windows, download
finalization/runtime guard/downloads, portal download history and portal browser.
The 108.8-second page-search pass distinguishes the earlier compilation-loaded
timeout from a migration regression.

Public-page-host tests have three distinct requirements. `qualify.py` executes
the complete discovered inventory as logical offscreen components, captured
commands on the owned Wayland compositor with its disposable clipboard, and
actual Hyprland input on the inherited session. The latter needs the existing
`FERRIC_TEST_HYPRLAND_RUNTIME_DIR` setting and passes in the baseline. The small
owned compositor does not implement arbitrary desktop activation policies;
running the whole suite there is not valid native-input qualification. Initial
live-desktop and headless-compositor setup failures remain recorded.

The frozen baseline reproducibly fails the existing
`siblingPolicyChangeRevokesTheSharedProfilesCapturedRequest` assertion after
policy invalidation. Its result remains nonzero. The v2 verifier permits a
public-host baseline comparison only with the frozen starting revision, the
same complete scoped harness, matching Qt library hashes, original executable
references and assertion sources identical in Git and the candidate. It checks
exact assertion messages/locations, complete test inventories, unchanged Qt
row counts and zero skips. New bridge, freshness, runtime and performance
failures cannot use this exception. Fourteen policy/verifier self-tests pass.

The initial 76-entry candidate application run passed 73 entries. Its failures
were the missing CLI executable setup, a synthetic offscreen rename-navigation
case also failing in the baseline, and the full aggregate's twenty-minute
timeout. The CLI now points directly to the staged product. Synthetic Tab
delivery waits for rendered delegate incubation; both manager-form rows pass
in the targeted eight-second check. Offscreen window assertions now expect no
Wayland tag submission. The full aggregate retains individual assertion/function
deadlines and has a two-hour outer budget so every function can run.

The final complete application pass is running. The remaining public-host,
everyday, explicit QML, Wayland/blocking, installed GTK runtime and paired
performance receipts are still pending. No complete product qualification is
claimed until these gates and the successor verifier finish.
