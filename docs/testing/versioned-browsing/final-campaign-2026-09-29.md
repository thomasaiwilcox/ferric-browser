# Versioned Browsing final campaign — 2026-09-29

This records the final-campaign checks run after the continuous implementation
pass. A passing unit, fixture, or structural probe does not enable a production
capability. The backlog remains the status authority; no slice is promoted to
`verified` by this report alone.

## 2026-10-02 continuation — current implementation tree

The first `cargo xtask check --locked --offline` run exposed seven failures in
the versioning-panel QML fixture. The fixture used a hidden `Item` as its
window, so visibility-dependent assertions and mouse actions could not run;
the production panel was not changed. A visible `ApplicationWindow` with the
real archive-view URL property now hosts the fixture. Its focused suite passes
15/15. A rerun of the complete project check passes: 151 QML checks, zero
failures, five fixture-specific skips covered by dedicated native probes, plus
the workspace Rust suites (1,094 tests in nonempty suites). The first broad
rerun stopped only at two formatting differences in the newly added native
download runner; those were formatted before the passing rerun. The focused
application versioning capture suite passes 27/27. Workspace Clippy with
`-D warnings` passes; C++/Qt headers emit compiler warnings outside that Rust
lint result. Both cached and uncached offscreen adapter startup smokes pass.
Nested Wayland startup passes, while editable-page input qualification is
not-run because the compositor does not expose virtual-keyboard input.

For VB-022, `/usr/bin/python3 tools/tests/test_versioning_native_panel_probe.py`
passes four helper tests. A fresh disposable durable normal-profile Qt process
accepted a structural commit; its profile-local metadata database contained
one exact commit ID. After process termination and restart, `commit-list`
was accepted. The first accessibility attempt using `QT_ACCESSIBILITY=1` had
no AT-SPI application root for the target PID, so its row result was
inconclusive. A second restart using Qt's process-local
`QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` registered the exact process, and the
bounded probe observed that saved ID as a descendant of `Saved commits`.
This closes VB-022's native rendered-list gap only. It does not turn on
archive, cookie, WebRTC, live-retention or branch capability flags.

## Continuation results — consent, restore hosting and startup

The continuation fixed an executable consent-handler scope error: the inline
parent handler referred to an internal `dialog` ID from another component. The
production consent host now owns an explicit `consentDialog` ID and routes
accept/decline to the exact native operation. Its QML suite passed 7 checks;
restoring the old reference in a disposable copy failed 4 checks with
`ReferenceError: dialog is not defined`. A rejected native callback preserves
the disclosure; stash acceptance stays disabled.

The restore coordinator now loads by local URL on demand, with required
window/UI properties supplied before activation. Its suite passed 8 checks,
including actual first load, reuse, one-shot editable consent, selectable
display-only contenteditable text, timeout and stale callback cleanup. The full
QML directory passed **117 checks, 0 failures, 4 environment-gated skips** on Qt
6.11.2 with default GC settings (offscreen/software). These are component-level
results, not native committed-sidecar qualification.

The browser build passed. `cargo xtask test adapter` was strengthened to remove
diagnostic GC/interpreter overrides and run both default-cache and uncached QML
startup. The run passed its bounded 10-second default-cache case, then failed
the uncached case with SIGSEGV and empty stdout/stderr. Thus the native startup
blocker is **not resolved**; warm startup alone is not sufficient evidence.
Diagnostic local-QML copies showed that reducing the initial component tree
can avoid some crashes, but neither deferring the coordinator nor removing the
eager restore-window factory reliably eliminated them. No GC workaround or
alternate QML entry-point environment variable is shipped.

An additional 15-second run with an **empty, disposable disk-cache directory**
and otherwise normal settings survived without diagnostics. This is distinct
from forced runtime compilation: `QML_DISABLE_DISK_CACHE=1` disables reuse of
precompiled QML as well as disk caching, whereas a normal first launch can use
the embedded compilation units. Do not describe the failed stress case as
proof that every first launch fails. The runtime-compilation crash and its
origin remain unresolved. [Qt cache configuration](https://doc.qt.io/qt-6/qmldiskcache.html).

A disposable native Wayland normal profile successfully stored one structural
commit in Ferric's own metadata database (one row, 1283-byte manifest). Native
restore created a second tab-capable window, but exposed an IPC ownership bug:
the new primary surface installed the process-global request waker and polled
the shared queue. A query that previously returned source sequence 6 and its
live registry instead returned the child's sequence 3 and empty registry.
That result is **not** counted as a successful end-to-end restore. The restore
wrapper now opts out of instance request ownership; its application-worker
pump remains active. Native retest results are recorded below when available.
Only synthetic blank-page state and a disposable profile were used; no
Chromium database or real account was inspected.

The backend investigation now includes immutable Qt 6.11.2 source pins and
specific missing activity/cookie contracts in
[ADR-0016](../../architecture/ADR-0016-versioning-engine-capability-backend.md#pinned-source-inspection--2026-09-29).
No fork was built and no capability flag changed.

After the consent, restore-host and instance-request ownership changes,
`cargo xtask check --locked --offline` passed again with exit code 0, including
the 117-pass/4-skip QML directory, workspace Rust suites, bounded native probes,
release-manifest and license-policy checks. The adapter stress check is separate
and remains failed as described above; the workspace gate does not override it.

Subsequent ownership corrections restrict child close/startup failure to the
child window, skip profile-wide stale-checkpoint cleanup in restore children,
and retain dynamically created destinations in a bounded feature-owned window
controller until approved close. The controller participates in the parent's
shutdown/cancel protocol. Its focused QML suite passed 5 checks, including
explicit GC and capacity refusal. These additions followed the workspace run
above; their final retest must not be inferred from that earlier run.

The complete QML directory was rerun after those corrections: **122 passed,
0 failed, 4 environment-gated skips**, with default GC. The offscreen WebEngine
run emitted Vulkan/GPU and transient `blink.mojom.Widget` diagnostics; no QtTest
assertion failed. `cargo fmt --all -- --check`, `git diff --check` and the pack
checksum validation passed. `cargo clippy -p xtask --all-targets --locked
--offline -- -D warnings` passed for the changed smoke runner.

The focused adapter boundary regression
`structural_restore_uses_a_tab_capable_blank_primary_window_for_multi_page_plans`
passed (1 passed, 414 filtered) after the ownership/close/checkpoint changes.
Two earlier focused-test attempts were terminated (exit 143), not counted as
passes. A subsequent clean-profile native GET capture again produced a durable
replayable commit, but the destination did not remain mapped and the source
reported only “Opening”. Thus retention alone is not claimed to fix the native
restore failure. Startup profile/configuration rejection now reports a fixed,
content-free failure to the exact source UI before the child closes, to prevent
that misleading indefinite “Opening” state. Native integration remains open.

The final native failure-path retest confirmed the profile-instance branch:
the source reported “Restore window profile could not initialize; current work
is intact.” Its live registry entry, window/tab IDs, owner token and sequence
remained unchanged and IPC remained responsive after the failed restore.
Pinned Qt source rejects a second profile prototype for an already-used
persistent storage path. The remaining work is the same-profile, tab-capable
window adapter described in [VB-024](VB-024.md#native-profile-construction-constraint),
including exact profile-wide callback ownership and instance discovery—not
relaxing Qt's profile-path check. No end-to-end new-window restore success is
claimed from these failure-path tests.

A subsequent same-profile adapter implementation now avoids the second
prototype entirely, reuses the exact existing profile/interceptor, disables
duplicate profile signal receivers and routes child downloads by exact view or
retained request identity. Different-profile secondary owners remain refused.
The real Qt profile-host fixture passed 3 checks; window ownership/routing
passed 6; the combined QML directory passed **126, failed 0, skipped 4** with
default GC. Native full-window qualification remains distinct and is recorded
after the rebuilt application is exercised.

Native same-profile retest used a new disposable normal profile on Wayland,
default GC, and a synthetic loopback document. Placeholder restore kept a
second native window open. An explicit typed GET produced a replayable durable
commit; restoring it opened a third window and emitted another document request
at the fixture server. Source IPC queries retained the original owner token
and window/tab IDs. Confirmed close removed the placeholder child while the
source and GET child remained mapped. The 200-second bound then expired; clean
whole-instance shutdown is not established. Restore children still do not
appear in source discovery. No real account/profile data was used.

The first workspace rerun after the profile-host change passed 414 engine
tests and failed one obsolete source-layout assertion looking for the profile
storage-name expression in its former component. That assertion now checks
the new host's temporary/persistent namespace expression; results of its rerun
are recorded separately below.

A second fresh-profile lifecycle run used only a blank source and one restore
child. Source IPC `quit` followed by explicit confirmation closed both windows
and returned **exit code 0** before the 180-second timeout, with no QML
diagnostics. This qualifies that idle shutdown case, not active-download,
dirty-page, cancellation or multi-page behavior. Manual smoke on the latest
built binary again reached the normal 10-second bound without diagnostics;
forced runtime compilation (`QML_DISABLE_DISK_CACHE=1`, diagnostic GC/JIT
overrides removed) exited 139 with a core-dump notification.

After updating the moved-profile source assertion, the final
`cargo xtask check --locked --offline` rerun completed with **exit code 0**.
This includes the 126-pass/4-skip QML suite, workspace Rust tests, bounded native
probes and release/license checks. The separate forced-runtime-compilation
startup failure remains open; the workspace pass does not override it.

Discovery continuation adds bounded retained-child projection and exact-owner
token focus without inserting full restore surfaces into the legacy single-tab
ownership registry. The owner QML suite passed 8 checks. A full QML run with
inherited desktop theme failed the panel cancel-click assertion (127 passed,
1 failed, 4 skipped); the panel alone passed 14 checks and the full standard
environment rerun with `QT_QPA_PLATFORMTHEME=` passed **128, failed 0, skipped 4**.
No panel implementation change or timing workaround was made. These later QML
changes follow the workspace pass above; native integration is checked separately.

The rebuilt discovery implementation passed a fresh-profile native journey:
two distinct owner-token entries were published despite identical local window
IDs; exact-token focus activated the restore child; confirmed close removed
only that entry; reusing its token reported a stale target. Source IPC token
and sequence were unchanged and subsequent source quit returned exit code 0.
Tab-count changes also now trigger publication; their multi-page check follows.

The final rebuilt binary passed that multi-page check in another fresh normal
profile. Three blank source tabs produced a three-page durable manifest;
restoring it published a second owner with `tab_count: 3`, retaining the source
token, selection and sequence. Confirmed source quit closed both three-tab
windows and returned exit code 0 without QML diagnostics. The final QML rerun
passed **128, failed 0, skipped 4**; formatting/diff checks passed. No complete
workspace rerun after the discovery-only QML continuation is claimed.

## Results

- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` — passed.
- `cargo xtask check --locked --offline` — passed with exit code 0. The workspace
  Rust suites passed, the combined QML run passed 108 tests with 4 environment-
  gated skips and 0 failures, and the synthetic
  page-state, guarded archive-save, bounded cookie-observation/adapter, TLS
  fixture, release-manifest, and license-policy probes reported success.
- `cargo test -p ferric-browser-storage --locked --offline versioning::` — 48
  passed, 0 failed.
- `cargo test -p ferric-browser-application --locked --offline versioning::capture::tests`
  — 27 passed, 0 failed.
- The focused `tst_versioning_panel.qml` run — 14 passed, 0 failed. The
  `tst_versioning_restore_coordinator.qml` suite also passed in the combined
  run, including selectable plain-text editable-preview behavior. The panel
  regression follows keyboard focus through list → details → live-restore
  preview → confirmation. The nested-window helper still does not prove native
  keyboard activation or assistive-technology behavior.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- Pack validation reported 96 tasks, 9 phases, 275 dependency edges, with 34
  verified, 20 in progress, 6 not started, and 36 blocked. The resolver reports
  no dependency/capability-ready task.

## Native startup blocker

`cargo xtask test adapter` rebuilt the current `ferric-browser` and failed: the
offscreen process exited with SIGSEGV before emitting stdout/stderr. The fresh
systemd core from this campaign identifies PID 2963010, launched as
`target/debug/ferric-browser --temp-basedir`, with `SEGV_MAPERR` in Qt QML 6.11.2
at `QV4::MarkStack::drain`. Its GUI-thread stack proceeds through
`QQmlValueTypeWrapper::create`, `QQmlBinding::evaluate`, QML component
finalization, and `QQuickTextField::componentComplete` while the application
engine loads QML. Other sampled threads were primarily waiting. Available
memory was approximately 9 GiB and the kernel journal had no OOM-kill entry for
this run. This reproduces the prior startup failure; it does not prove whether
Qt or application/native object lifetime handling caused the invalid state.

Read-only GDB inspection of this core found `RAX == 0` at the faulting
`MarkStack::drain` instruction, which dereferenced address `0x10`; earlier
repeated cores faulted later while dispatching through a null heap-class vtable.
Both observations place the failure in QV4 heap marking, but neither identifies
which earlier operation corrupted or invalidated the heap entry. The core was
inspected in place through `coredumpctl`; no copied core was retained.

A later core from the same isolated startup failure (PID 2759671) was inspected
in GDB. The faulting instruction is the indirect call used by
`MarkStack::drain` to invoke `h->internalClass->vtable->markObjects`; the
managed heap entry and its internal-class pointer are readable, but the vtable
pointer is null. Qt's corresponding 6.11.2 source performs this dispatch in
`MarkStack::drain` ([source](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/qml/memory/qv4mm.cpp)).
This narrows the symptom to an invalid QV4 heap entry/class descriptor during
marking. It does not establish whether the corrupt state originates in Qt or
in application/native object lifetime handling, so the startup blocker remains
unresolved.

Diagnostic variants reproduced the crash with `QV4_FORCE_INTERPRETER=1`,
`QV4_MM_CROSS_VALIDATE_INCREMENTAL_GC=1`, and `QT_QUICK_BACKEND=software`.
Bounded 10-second smoke runs survived with `QV4_GC_TIMELIMIT=0` and with
`QV4_MM_AGGRESSIVE_GC=1`. These are diagnostic observations only: they narrow
the symptom toward Qt's incremental-GC path but do not establish the root cause
or justify shipping a global-GC workaround. No workaround was committed and
the normal startup smoke remains failed/unqualified. Qt documents its QML GC
configuration and the incremental-GC controls in the
[JavaScript engine tuning guide](https://doc.qt.io/qt-6/qtqml-javascript-finetuning.html).

## Capability and release gates

- Live-tab stash, hidden retained views, and clean branches remain blocked until
  the backend can prove a per-view WebRTC data channel/call is absent while a
  view is frozen. The current public Qt recommendation is not that proof.
- General cookie snapshot/seed/merge remains blocked: public Qt does not expose
  complete cookie partition identity and a complete source snapshot boundary.
  The bounded observation adapter remains diagnostic only.
- Page archive capture/view and current-window replacement remain disabled.
  Native consent/download lifecycle, actual offline form/redirect behavior,
  archive isolation, and stable startup still need qualification.
- VB-025 remains in progress because the current adapter startup smoke crashes
  before a native application window is available. QML-level keyboard tests
  are not counted as native-window or assistive-technology qualification.

The positive and negative claims above are intentionally kept separate. No
public-API gap is replaced with a private API, mirror, mock, or guessed
capability, and no release flag was enabled.

## Post-campaign implementation continuation — not yet verified

After the checks above, the native archive probe was extended to link and attach
the production archive request interceptor. Offscreen input produced no
Chromium navigation despite active Quick focus, and Weston headless lacks the
virtual-keyboard protocol. The probe now requires an active Wayland session,
opens a visible disposable window, and focuses it through Hyprland when
available. Both active-session keyboard injection and a QtTest click to the
active `QQuickWindow` produced no form-navigation callback. The production
archive view enables the public `focusOnNavigationEnabled` setting while
keeping scripts disabled. Archive viewing remains disabled.

## Final verification re-run after the archive-probe continuation

- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` —
  passed with exit code 0. Qt/GCC headers emitted the existing incomplete-SFINAE
  warnings during native compilation; no Rust lint error occurred.
- `cargo test --workspace --locked --offline` — passed with exit code 0 across
  workspace unit and doc tests.
- `cargo test -p ferric-browser-application --locked --offline versioning::`
  — 122 passed; `cargo test -p ferric-browser-storage --locked --offline
  versioning::` — 48 passed.
- `cargo xtask test archive` — failed at
  `QtTest click on active Wayland window activates archived form`: the native
  window and WebEngine item reported active focus, but no form-navigation
  callback occurred. Archive viewing remains disabled.
- `cargo xtask check --locked --offline` — the combined QML suite passed 108
  tests with 4 skips; native page-state sidecar admission passed. The command
  then stopped at the same archive form-navigation assertion, so later check
  stages did not execute in this run.
- `cargo xtask test adapter` — reproduced the existing offscreen adapter
  startup SIGSEGV (signal 11) before any application output. No workaround was
  applied.
- `CurrentImplementation/ferric-versioned-browsing/validate_pack.py
  --summary` — document/ledger integrity passed: 96 tasks, 9 phases, 275
  dependency edges; 34 verified, 20 in progress, 6 not started, 36 blocked;
  no production capability enabled and no eligible dependency/capability-ready
  not-started task.

## Focused post-campaign unblock investigation

A clean, isolated worktree at the proposal baseline `deea458` was built with
`cargo build -p ferric-browser --locked --offline`. Its offscreen GUI survived a
bounded 10-second `--temp-basedir` run. The current implementation still exited
with SIGSEGV (139) in the same smoke, even after a temporary ablation of the
restore-window factory in `Main.qml`; that source was restored. This confirms a
regression relative to the clean baseline but does not identify its source.
The sampled current core again faults at address `0x10` in
`QV4::MarkStack::drain` during QML object construction.
The restored build also crashed in native Wayland mode and in the temporary
safe-mode profile, so this is not confined to the offscreen platform or a
durable-profile startup path.

For the restored full-window build, `QV4_GC_TIMELIMIT=0` survived the same
bounded smoke and `QV4_GC_TIMELIMIT=0 cargo xtask test adapter` passed. A
15-second smoke with both non-incremental and aggressive GC also survived;
aggressive GC alone survived a bounded 10-second smoke.
`QML_DISABLE_DISK_CACHE=1`, `QV4_FORCE_INTERPRETER=1`,
`QV4_MM_CROSS_VALIDATE_INCREMENTAL_GC=1`, and an 8 MiB
`QV4_GC_MAX_STACK_SIZE` did not prevent the default-GC fault in diagnostic
runs. Qt documents that a zero GC time limit switches QML to non-incremental
collection ([Qt JavaScript engine configuration](https://doc.qt.io/qt-6/qtqml-javascript-finetuning.html)).
This is a diagnostic contrast, not a root-cause finding or a committed runtime
workaround. The default startup release gate remains failed.
With that environment setting, `cargo xtask check --locked --offline` completed
with exit code 0: the QML suite reported 108 passed and 4 environment-gated
skips; native page-state, archive, cookie, TLS, release-manifest and license
probes passed their scoped assertions; and all workspace unit/doc tests passed.
The cookie probe still reported incomplete public partition/SameSite coverage.
This conditional run does not substitute for the failed default-GC campaign.

The native Wayland archive probe now proves that QtTest clicks reach a normal
local HTML button, link, and POST form, and a link inside a Qt-generated MHTML.
On this Qt WebEngine build, both POST and GET MHTML form clicks produce no Qt
navigation callback or loopback traffic. Chromium's MHTML policy applies
sandbox flags to archive documents that include form restrictions
([Chromium navigation policy](https://chromium.googlesource.com/chromium/src/+/e6de25ee4229a5154a458c97fb6d0da7d8845b0f/content/browser/renderer_host/navigation_policy_container_builder.h),
[form submission check](https://chromium.googlesource.com/chromium/src/+/d75446b2104cabc460e5c0899244856bb9e77386/third_party/blink/renderer/core/html/forms/html_form_element.cc)).
That provides a plausible renderer-level explanation, not evidence that
Ferric's view/profile form handlers ran. The probe therefore handles both
outcomes: it requires both Ferric barriers if a future Chromium emits the
form navigation, and otherwise requires no form request or network traffic.
The archived external link independently reaches Ferric's view rejection and
profile interception. The revised probe passes with a disposable, native
Wayland window. Qt's MHTML serializer retained the remote CSS/image references
in the fixture, but neither reached the profile interceptor; it removed the
unresolved remote iframe reference. This probe does not claim that CSS, image,
or frame requests reached Ferric's profile guard. Archive production
enablement remains false pending the remaining isolation and application
qualification.

## Backend feasibility review

[ADR-0016](../../architecture/ADR-0016-versioning-engine-capability-backend.md)
records the subsequent research on an alternative engine backend. Documented
CDP cookie methods have richer metadata but no stated consistent-snapshot or
conditional-apply guarantee, and Qt's remote-debugging endpoint has a separate
credential-exposure risk. Chromium's browser-side PeerConnection observer and
canonical-cookie interfaces provide possible native integration seams, not a
qualified Ferric capability. The proposed next step is a version-pinned,
disposable native-extension feasibility spike. No release gate or slice status
changes because of the review.

## 2026-10-01 current-build startup recheck

A fresh `cargo build -p ferric-browser --locked --offline` succeeded. Three
ordinary offscreen `--temp-basedir` startups survived a bounded 10 seconds,
but the original `cargo xtask test adapter` gate did **not** pass: its default
QML-cache variant survived, then its `QML_DISABLE_DISK_CACHE=1` variant exited
with SIGSEGV. The fresh core (PID 118027) again shows `SEGV_MAPERR` at address
`0x10` in `QV4::MarkStack::drain` while a QML value-type binding reacts to
`QQuickApplicationWindow::setFont` during root component finalization. The
kernel reported the segfault, not an OOM kill; approximately 11 GiB of memory
was available at inspection. The historical core is therefore no longer the
only evidence: the current implementation still fails the uncached gate.

The clean baseline executable survived the same uncached 10-second smoke.
Two temporary comparisons in the current checkout did not fix the crash:
binding `FontMetrics` directly to the configured font family/size instead of
the whole window font, and replacing the new profile Loader with the old direct
profile prototype. Both changes were reverted; the latter would also have
weakened restore-child profile ownership, so it is not a viable workaround.
No Qt GC override or application workaround was committed. This supports a
current-implementation startup regression but does not yet identify the
corrupting binding or native lifetime operation. Keep native GUI acceptance
open and rerun both adapter variants after a root-cause fix.

### Disposable QML entry comparison

A debug-only local-entry loader and a byte-identical copy of the QML source
tree were used to narrow the uncached crash; neither is part of the retained
application. With the same native binary and `QML_DISABLE_DISK_CACHE=1`, a
bounded 10-second offscreen run survived when `Main.qml` and its components
were loaded from local files, but the embedded `qrc` entry crashed. A local
`FerricBrowserRuntimeSurface` extending the embedded runtime chrome also
survived when its `BrowserScripts.js` import was local. Repointing just that
import to the embedded script URL reproduced the crash; using the embedded
Keyboard and Spellcheck scripts with local BrowserScripts survived. This is a
reproduction boundary, not proof that BrowserScripts' source or Qt's resource
loader is defective: the same script is imported by other embedded components.

Removing the unused BrowserScripts import from the production Surface and
rebuilding did **not** fix the uncached startup: it still exited with SIGSEGV
(139) after approximately 1.6 seconds. The import removal and debug-only
loader were reverted. The adapter gate remains failed; no GC environment
override or speculative resource-packaging change was retained.

### Unused shared-script imports and adapter recovery

A second comparison ruled out library-style script sharing as a sufficient
cause: removing `.pragma library` from `BrowserScripts.js`, rebuilding, and
running the uncached offscreen smoke still produced SIGSEGV (139). That change
was reverted. In the disposable local-QML tree, replacing only the Surface's
embedded `BrowserScripts.js` import with the larger embedded
`VersionedPageState.js` import survived the bounded 10-second uncached run.
Removing the Surface's restore-host child while retaining its embedded
BrowserScripts import still crashed. These comparisons implicate the repeated
BrowserScripts import path, but do not prove the Qt GC implementation's root
cause.

`FerricBrowserRuntimeChrome.qml`, `FerricBrowserRuntimeRequests.qml`, and
`FerricBrowserRuntimeSurface.qml` imported `BrowserScripts.js` without ever
referencing that namespace. Removing all three unused imports together and
rebuilding changed the observed result: three independent 10-second
`QT_QPA_PLATFORM=offscreen QML_DISABLE_DISK_CACHE=1` temporary-profile
startups survived, and `cargo xtask test adapter` passed both its default-cache
and uncached 10-second variants without a GC override. The earlier single
Surface-import removal had not helped. The debug-only local-entry loader was
removed after the comparisons. A final `cargo xtask test adapter` rerun on
that exact source state also passed both default-cache and uncached 10-second
variants. This repairs the observed adapter startup gate on the current build,
not a release-wide GUI certificate or proof of the Qt GC root cause.

`cargo xtask check --locked --offline` then completed with exit code 0 in the
ordinary environment, with no `QV4_*` or QML disk-cache override inherited.
The QML suite, native sidecar/archive/cookie/TLS probes, generated-doc and
release checks, formatting, and workspace unit/doc tests all completed. The
public cookie fixture still reported SameSite/Partitioned identity as
unqualified; this successful check does not enable a production capability.

## 2026-10-02 compact native keyboard continuation

The separate two-view `cargo xtask test versioning-profile-download` fixture
passed again. A 627 × 694 native Ferric window then exposed a VB-025
restore-review overflow: its confirmation button had been laid out outside
the dialog. A compact QML regression reproduced the failure, and constraining
the review column to its parent width plus explicit focus transitions repaired
it. The focused panel suite passed 17/17 on the repaired source.

The rebuilt native application used a disposable normal profile and an exact
one-page structural commit whose navigation was an `unsafe-navigation`
placeholder. With process-local Qt accessibility enabled, AT-SPI located the
target controls; compositor-directed keyboard events were sent only to that
Ferric window. Return opened details from the saved-commit list, Return opened
review, Tab moved focus from the safe Cancel default to the now-visible
confirmation, and Return opened a second same-process window while the source
stayed mapped. The confirmation bounds were inside the 627 × 694 native
dialog, and a later `commit-list` result refocused the list. This is scoped
structural keyboard/new-window evidence, not multi-page safe-GET, sidecar,
active-download/dirty-page, current-window replacement, cookie, WebRTC or
release qualification. `cargo xtask check --locked --offline` subsequently
passed with exit code 0 on these final QML changes: 153 QML passes, 5
environment-gated skips, zero QML failures, and completed workspace Rust
suites. Native archive input remained unqualified because its Wayland focus
probe could not acquire the source control; the public cookie probe continued
to report incomplete SameSite/Partitioned identity. Neither gate was enabled.

## 2026-10-02 native two-page safe-GET continuation

A disposable normal-profile two-page commit retained both loopback URLs as
replayable. Native restore kept the source mapped and its IPC owner available,
opened a child with two reducer tabs and two native views, and requested only
the selected `/popup-target` document. The first accessibility pass found a
one-tab child strip despite the two-tab model. A Qt `ListView` layout flush
after reconciliation and source-model bindings for count-dependent tab-strip
presentation repaired that native defect: a rebuilt child exposed two page
tabs, with the selected Popup target tab. The focused chrome QML suite passed
29/29. The background tab still presents `about:blank`; safe deferred URL
activation is not qualified. Sidecar, Ferric multi-window download callback,
dirty-page/download shutdown, and current-window replacement remain open.
The complete `cargo xtask check --locked --offline` then exited 0 with 154
QML passes and 5 gated skips; archive-input and public cookie probes remained
unqualified.

## 2026-10-02 deferred safe-page native continuation

The application now retains inactive replayable restore URLs in a bounded,
owner- and document-bound live map and starts one ordinary navigation when
the exact lazy tab is activated. A capacity check refuses the restore before
tab allocation if its deferred URLs cannot be retained. QML exposes keyboard
and accessible Press activation and labels unopened tabs “Restored page —
select to load” without displaying the URL. Application one-shot/staleness
and capacity regressions passed; focused chrome QML passed 30/30.

The rebuilt disposable two-page restore child exposed two accessible tabs,
including the neutral unloaded label. Its selected page alone requested
`/popup-target` on restore. Pressing the unloaded first tab through AT-SPI
caused exactly one `/final` request and changed its label to “Redirect final”;
pressing it again produced no request. The source stayed mapped. The complete
`cargo xtask check --locked --offline` passed on the capacity-guarded source
(155 QML passes, 5 gated skips, workspace Rust suites). Native committed sidecars,
Ferric multi-window download forwarding, active-download/dirty-page shutdown,
viewless downloads and current-window replacement remain unqualified.

## 2026-10-02 disposable Wayland archive-input continuation

The native archive probe's active-desktop skip was traced to unavailable
window focus: Hyprland showed its Quick window mapped, but another app
remained active even after a current-syntax exact-window focus request. The
same compiled probe passed inside disposable Weston headless with a fake
input seat, Pixman rendering and kiosk shell. `cargo xtask test archive` now
automatically uses that isolated Wayland environment when available and
tears it down afterward; a focused run passed. The probe covers native
page-control and link input, correlated MHTML save/cancel, hostile archive
navigation and zero-network assertions. This is not Ferric's integrated
capture or committed-sidecar journey, and it does not open the production
archive release gate. See [VB-045](VB-045.md).
The complete `cargo xtask check --locked --offline` then passed with the
archive probe executed rather than skipped (155 QML passes, 5 gated skips,
workspace Rust suites). Its disposable Weston process, socket and lock file
were gone after the run.

## 2026-10-02 shared-profile download routing continuation

The native two-view Qt fixture now includes Ferric's production restore-window
owner and profile download-routing functions. With two real views on one
off-the-record profile, the source and restored host each received exactly
their own request and finish ownership. An ambiguous claim over the second
view and an ID-only forged terminal request were refused. The focused
`cargo xtask test versioning-profile-download` passed. A disposable Ferric
source and restored child were also observed loading the same loopback page,
but desktop input did not activate the child link for a full UI download
journey. The native fixture does not qualify Ferric's durable ledger,
attachment handling, viewless Qt getter, or active-download shutdown.
The complete `cargo xtask check --locked --offline` passed with the expanded
fixture (155 QML passes, 5 gated skips, workspace Rust suites).

## 2026-10-02 restored-child shutdown cancellation continuation

A saved editor-page commit was restored into a same-process child against the
loopback fixture. Before repair, choosing “Keep browser open” on the child's
page-state warning left the source's process-wide shutdown gate active until
its timeout prompt. The child owner now reaches the root host's shutdown abort
only while a process-wide shutdown is active. Focused owner QML passed 15/15.
After rebuilding, refusing the child's page-state warning kept both windows
mapped with no timeout prompt after 12 seconds; a second `:quit` produced a
fresh root confirmation, confirming the prior gate had ended. Active-download
shutdown, same-document mutation, committed sidecars, viewless download
behavior and current-window replacement remain unqualified.
The complete `cargo xtask check --locked --offline` passed afterward (156 QML
passes, 5 gated skips, native synthetic probes and workspace Rust suites).

## 2026-10-02 streamed download continuation

The loopback fixture gained a bounded 8 MiB slow attachment. The native
shared-profile two-view test now navigates the restored view to it, accepts
the request, observes partial bytes while unfinished, cancels it, and proves
the same object is delivered to `downloadFinished` with cancelled state and
restored-host routing. Its focused test passed. A disposable Ferric entry
window also opened the slow attachment with an established TCP transfer;
native `:quit` presented the active-download warning. “Keep browser open”
left the process mapped, and a second quit opened a fresh confirmation.
Restored-child active-download shutdown, the Ferric durable ledger, viewless
callbacks and cancel-and-quit are not qualified by these checks.
The complete `cargo xtask check --locked --offline` passed afterward,
including the expanded native download probe and workspace Rust suites.

## 2026-10-02 restored-child download and timeout continuation

A one-use loopback arm caused the next restored editor-page GET to navigate
to a slow synthetic attachment. Against the saved editor commit, native
whole-browser quit reached the restored child's active-download prompt while
the transfer socket was established. “Keep browser open” preserved both
windows without a later timeout and allowed a fresh quit confirmation.
In a second run, “Cancel active work and quit” advanced to the child's
page-state review. The source timed out while that review was open; before
repair, child approval left the source on the force prompt until “Continue
waiting” was pressed. The entry owner now rechecks shutdown when an owned
child is removed. Focused owner QML passed 16/16; after rebuilding, approving
the child's page warning caused normal browser exit without force/continue
interaction. The synthetic profile's download row was `cancelled` with zero
bytes recorded. Accepted partial-byte accounting, the broader Ferric ledger,
viewless callbacks, sidecars and current-window replacement remain open.
The complete `cargo xtask check --locked --offline` passed afterward (157 QML
passes, 5 gated skips, native synthetic probes and workspace Rust suites).

## 2026-10-02 accepted restored-child download ledger continuation

The restored-child slow attachment was auto-accepted only in a disposable
profile and isolated destination. A native run showed that the durable row
remained at zero until completion because ordinary download handling did not
subscribe to received-byte changes. After adding bounded progress updates,
the rebuilt browser recorded 5,079,040 bytes while the transfer was still
in progress. A cancellation repeat exposed a second defect: the Qt request
guard rejected `cancel()` after `accept()`, so the row said cancelled while
the full 8 MiB file completed. The guard now permits one valid post-accept
cancel and shutdown records the observed terminal state. The next run stopped
the transfer and left no completed file, but Qt's terminal byte counter reset
to zero. Storage now retains each offered download's byte high-water mark;
the focused storage regression passed for individual and batched updates,
including reset on a new offer with a reused engine ID.

The final rebuilt native run observed an in-progress row with 851,968 bytes.
Whole-browser quit reached the restored child's active-download prompt;
cancel-and-quit produced a `cancelled` durable row with 6,389,760 bytes, no
completion timestamp, and no completed or staging attachment. Approving the
subsequent page-state warning closed the browser with exit 0. This qualifies
the accepted partial-transfer ledger/cancellation path only; viewless
callbacks, sidecars, broader multi-window cases and replacement remain open.
The complete `cargo xtask check --locked --offline` passed afterward,
including the workspace Rust suites and native synthetic probes.

The next disposable committed-sidecar attempt returned `E_ENGINE` for
`commit --pages --editable`; no sidecar was committed. The command requires
`ArchiveCapture`, while the documented production gate remains disabled
pending capture-orchestration qualification. This is not negative evidence
about restored sidecar behavior, and the gate was not bypassed.

## 2026-10-02 VB-026 corrupt-reference recovery preflight

The corrupt-reference quarantine path now rejects an oversized or malformed
digest set before creating a batch or moving an earlier valid object. A
missing shard is skipped as an absent object; unsafe existing shards are still
refused. Focused recovery tests passed 5/5, including invalid-after-valid,
over-limit, and missing-shard cases. This narrows one deterministic recovery
failure but does not finish the broader crash/fault campaign.
The complete `cargo xtask check --locked --offline` passed after the change;
strict storage-crate clippy and workspace formatting checks passed too.

## 2026-10-02 archive cancellation terminal continuation

The disposable Weston native archive probe passed after covering immediate
and started page-save cancellation. On Qt 6.11.2, both reached
`DownloadCancelled` without `downloadFinished` within bounded 15-second
waits; the immediate case reported `isFinished` without ever becoming
`DownloadCompleted`. Ferric now retires only its exact correlated
cancelled/interrupted capture from `stateChanged`, with the existing
90-second deadline as fallback. The production cleanup helper passed a
31/31 focused QML suite covering stale, unrelated, duplicate and
already-finished requests, path/ID quarantine and one-time operation cancel.
The complete `cargo xtask check --locked --offline` passed with this code,
native probes and workspace Rust suites. This does not qualify Ferric's
integrated save/fault journey or enable the production archive-capture gate.

An additional application/storage regression removed the exact staged file
after a real capture consent and reservation. Object installation failed;
cancelling the exact operation released the reservation and left no visible
commit in the disposable profile. The focused test passed. Disk-full and
actual Qt view loss remain unqualified.
The complete `cargo xtask check --locked --offline` passed with the regression
included, along with the native probes and workspace Rust suites.

The VB-044/045 duplicate/mismatched capture abort path now quarantines its
exact stage path and both request IDs before cancelling the new and original
Qt saves. Deadline and operation-retired cleanup clear the active capture
before Qt cancellation as well. A focused QML regression passed 32/32,
including synchronous-observation and throwing-cancel cases; the complete
`cargo xtask check --locked --offline` passed afterward. This is not a live
Ferric duplicate-save or deadline-race qualification, so the archive gate
remains disabled.

## 2026-10-02 delayed archive publication callback

An application/storage regression now holds a successful durable publication
callback beyond both the operation deadline and bounded callback window. The
SQLite commit is readable before and after an abort of the remaining
reservation. The application reports `OutcomeUnknown` and rejects a later
attempt to mark the operation completed, even after it receives the held
success callback. The focused test and strict affected-crate Clippy passed.
This does not simulate an OS crash or prove native capture orchestration.
The complete `cargo xtask check --locked --offline` passed with the regression
included, alongside the native probes and workspace Rust suites.

## 2026-10-02 media restore source-change refusal

VB-055 now recomputes the exact allowed media-source fingerprint immediately
before a user-approved seek. A native QML case changes `currentSrc` between
candidate collection and the seek and confirms that no seek or playback call
occurs. The focused Qt 6.11.2 offscreen suite passed 27 checks with four
expected loopback-fixture skips; `cargo xtask test versioning-geometry` passed
native sidecar admission. Integrated committed-sidecar restore and the
unkeyed fingerprint privacy decision remain unqualified; no release gate was
enabled.

## 2026-10-02 archive-view exact-permit continuation

The focused application publication/view test passed, including exact
materialization, cleanup, stale native-owner refusal, and corruption/missing-
object errors. The focused storage materialization test passed, including
removal of the exact temporary view directory on both failures. The native
`cargo xtask test archive` probe passed in disposable Weston after adding a
switch/revocation sequence: a previously rendered archive cannot reload after
authorization moves to another file, and the newly allowed file cannot reload
after its permit is cleared. No synthetic loopback traffic occurred. This is
profile-interceptor evidence, not a production archive-component lifecycle
or full hostile-resource qualification; the release gate remains off.

The native probe subsequently loaded the actual on-demand production archive
component with its own off-the-record profile. Its exact MHTML load and
successful blank acknowledgement passed while the temporary file still
existed; a later ordinary source-profile load also succeeded, with no
loopback traffic. `cargo xtask test archive` passed in disposable Weston.
Timeout/retry and the production chrome's storage-cleanup callback were not
exercised, so this is not an archive release qualification.

The native probe then loaded hostile MHTML in the actual production viewer.
Its clicked external link was rejected by that view, with zero loopback
requests. Immediate blanking after the rejected navigation timed out once;
the file remained retained and the component's explicit retry acknowledged
blank. Two subsequent disposable-Weston runs passed, with the reported
`hostile_link_rejected=1;hostile_blank_retry=1;network_hits=0` witness in the
last run. Workspace formatting passed. Strict xtask Clippy encountered the
pre-existing large-stack-array lint in `fixture_server.rs`; rerunning with
only that lint allowed passed. The archive release gate remains off.

The hostile MHTML fixture now retains visible download and unregistered-
protocol links. Native QtTest clicks in the actual production component
incremented the view's rejection count for both. Three disposable-Weston
runs passed, with the last bounded result reporting
`download_requests=0;download_rejected=1;protocol_rejected=1;network_hits=0`.
The probe requires those refusals. Chromium did not emit a download request
for this saved-page link, so the QML cancellation callback and the integrated
blank-to-storage-cleanup handoff remain unqualified.

An embedded `data:` download control now completes through Qt on the ordinary
source page at its exact native-click coordinate. Qt's saved MHTML retains the
link, but two disposable-Weston archive runs report no view callback, download
request, file or loopback traffic for the same click. A subsequent known link
is rejected by the production view, proving that input is still live. The
result is consistent with MHTML-sandbox suppression, not proof of Ferric's
download-cancellation callback. The archive gate remains disabled.

The existing production archive overlay was extracted into
`FerricVersioningArchiveHost` and retained the same runtime chrome bridge.
The native Weston probe loaded a valid MHTML through that actual host and
observed the successful blank-to-cleanup callback exactly once, followed by
one runtime schedule and viewer deactivation. Its file remained present for
the application worker, with zero loopback requests. The focused source-
boundary test, all 415 engine Rust tests, and 159 QML checks passed (5
environment-gated skips). This fixture bridge is not a full Ferric window
plus durable storage journey. Workspace formatting and strict engine-crate
Clippy passed. The offline release gate remains disabled.
