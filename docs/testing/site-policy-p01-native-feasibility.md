# P01 native feasibility — 11 November 2026

Status: Complete after two independent exit approvals on 11 November 2026.
P00 passed exit review before this work began. P02 revalidation is now active;
preserved partial P03 and later implementation remain held.

## Initial adapter probe

`FerricBrowserKeyRouter::submitFeasibilityPageKey` is a temporary C++-only
submission primitive for the narrow `p→k` experiment. It is not invokable from
QML and has no user configuration or renderer transport. The fixture selects
the mapping; the production router submits the destination to WebEngine's
current Quick focus receiver after checking the active window and view. The
probe accepts Linux XCB, Wayland and offscreen backends only.

The virtual destination declares Qt key K, text `k`, XKB scan identity 45,
keysym `0x6b`, no modifiers and standard location. It never copies the source
scan identity. A key-event subclass preserves internal-output provenance
through `clone()`. The original synchronous submission is admitted only to its
selected receiver; returned clones are consumed before browser source routing.
The fixture waits for an actual returned clone rather than an arbitrary delay.

This follows the qualified Qt 6.11.2
[native-event copy path](https://raw.githubusercontent.com/qt/qtwebengine/v6.11.2/src/core/native_web_keyboard_event_qt.cpp)
and [Quick unhandled-key return](https://raw.githubusercontent.com/qt/qtwebengine/v6.11.2/src/webenginequick/api/qquickwebengineview.cpp).
Its [key conversion](https://raw.githubusercontent.com/qt/qtwebengine/v6.11.2/src/core/web_event_factory.cpp)
uses native scan identity on XCB/Wayland, with a US logical fallback when that
identity is absent. Offscreen does not exercise the XKB scan conversion.

Initial focused runs each passed nine QtTest cases, with zero failures/skips:
three metadata controls, two constructed source identities, two unchanged
passthrough controls, initialization and cleanup. Both mapped source cases
delivered exactly one `k` down/up pair, `code=KeyK`, legacy code 75, location
zero, no modifiers, no original `p` and no destination readmission. The XCB
negative control deliberately supplied source scan 33 with logical K and
observed `key=k, code=KeyP`; the explicit destination used scan 45 and observed
`KeyK`. Zero metadata produced `KeyK` through Qt's fallback.

Both backends observed `isTrusted=true` and transient activation in these
constructed-input traces. These observations establish neither physical input
nor media/fullscreen permission outcomes. Media and fullscreen require separate
tests. The input source remains constructed Qt events even on XCB.

Evidence paths are relative to `target/site-policy-feasibility`:

| Record | Scope |
| --- | --- |
| `p01-native-probe-build-1/build-record.json` | Initial qualified SDK build, commands, source hashes and binary identity. |
| `p01-native-probe-offscreen-1/run-record.json`, `test.log`, `results.xml` | Nine focused offscreen cases; 2.901 s QtTest time. |
| `p01-native-probe-xcb-1/run-record.json`, `test.log`, `results.xml` | Nine focused XCB cases; 3.200 s QtTest time; constructed sources. |
| `p01-native-probe-build-2/build-record.json` | Subsequent explicit Linux/backend guard; current source build passed. |
| `p01-native-regression-offscreen-1/run-record.json`, `detail.log`, `ctest.xml` | Guarded build: 101 native-insert cases and 24 native lifetime cases passed, zero failures/skips; both CTests passed in 23.54 s. |
| `p01-native-probe-xcb-2/run-record.json`, `test.log`, `results.xml` | Guarded build: all nine focused XCB cases passed again; 3.052 s QtTest time. The record labels its post-run UTC timestamp explicitly. |

The current guarded-build runs verified unchanged source and binary hashes
before and after execution. These are functional adapter/ownership checks,
not P00 performance measurements or completed native platform qualification.

## Adapter-owned gesture candidate

The next candidate moves first-press admission, repeats and terminal release
into the production router. Source records retain physical identity, original
key/text/modifiers, session epoch, receiver/document generation, destination
ownership, cancellation and one-shot completion. The separate output ledger
arbitrates the probe's normalized K channel. A suppressed source never takes
over when the winner releases, while two unchanged native sources remain
unsuppressed. The original event and metadata are retained for native/raw
delivery.

The focused offscreen run passed 26 QtTest cases, zero failures/skips, in
7.392 s (`p01-native-ledger-offscreen-1/run-record.json`). Its producer is
`p01-native-ledger-build-1/build-record.json`, with retained source copies and
binary. Source/binary pre/postchecks match. Tests include repeated key-downs,
autorepeat release artifacts, four mapped/native collision orderings,
native/native parity, overlap after one-shot returns Normal, raw refusal and
retry, guarded cleanup versus cancellation, navigation, and physical-identity
recovery after a simulated logical-key change. Recovery consumes the fresh
press/release without page action and requires an explicit subsequent raw retry.

Configuration and raw/recovery entry remain temporary C++ fixture APIs. The
probe configuration is installed between gestures, and navigation invalidates
its ready receiver until explicitly rearmed. Full compiled policy replacement,
browser focus observation and public command wiring retain their owning phases.
The initial fixture's per-event submission remains only a separate metadata
control; the new gesture cases use router-owned decisions throughout.

## Ownership and reentrant dispatch review

An application-owned capture now retains already claimed physical sources
through router destruction, including intervals with no window router. It
creates no destinations and performs no policy matching. A cancelled source's
repeats and terminal release remain captured; an already observed terminal
release leaves no phantom held record. Abandoned admission also preserves
capture if its router or target window disappears inside the callback.

Independent review exposed cross-router supersession after the newer gesture
had already retired. The retained failing regression shows both outcomes:
the surviving older router delivered a stale down, and destruction of the older
router left a phantom held record that refused raw mode. Pending admissions now
share stack-scoped supersession markers across routers. A newer fresh press
invalidates older callbacks for that physical identity even after its own
release. Markers unregister during synchronous unwinding; no permanent physical
key history or separate policy engine is introduced.

Reentrant cleanup also uses per-property request generations, so an older
epoch/mode setter cannot overwrite a newer request made during destination
release. The lifetime tests cover destruction during admission, destination
down, repeat and terminal release, nested physical release before destination
down, output reservation during reentrant winner release, and capture with no
router alive. At that checkpoint production BrowserUi integration and
platform/capability proof were still pending; their completed results follow.

| Record under `target/site-policy-feasibility` | Result and scope |
| --- | --- |
| `p01-native-ledger-red-1/record.json` | Six reproduced ownership/setter failures; retained before fixes. |
| `p01-native-ledger-fix-1/record.json` | 37 focused cases passed after initial fixes. |
| `p01-native-ledger-asan-1/record.json` | 132 native-insert and 24 lifetime cases passed under AddressSanitizer. |
| `p01-native-cross-router-red-1/record.json` | Both cross-router supersession regressions failed as expected before the fix. |
| `p01-native-cross-router-fix-1/record.json` | 134 native-insert and 24 lifetime cases passed under AddressSanitizer; both CTests passed in 34.96 s. |

Each record retains source snapshots, exact build/test commands, binary hashes
and an exact source postcheck. Sanitizer runs disable leak detection for the
Qt/plugin process; address/lifetime instrumentation remains enabled. These
results qualify the tested ownership paths, not performance or the full native
platform matrix. Independent scoped review found no remaining concrete issue
in the cross-router fix; this is not P01 exit approval.

## Native source-input proof

The source-input suites passed 11 QtTest cases each, zero failures/skips:

| Record under `target/site-policy-feasibility` | Input and result |
| --- | --- |
| `p01-native-platform-wayland-1/record.json` | An isolated owned Qt Wayland compositor resolves the mapped surface by PID/title and sends seat events. Persistent mapped, one-shot mapped and raw P each produce the exact expected DOM down/up pair. |
| `p01-native-platform-xcb-3/record.json` | XTEST through the current Xwayland server sends source P to the owned focused test window. The same three source-input rows pass. A short RAII server grab spans native focus verification and down/up; the helper reads keyboard state and preserves NumLock. |

Both log native source scan 33 and no keyboard modifiers. Mapped output is
`k/KeyK`; raw output retains `p/KeyP`. The output route remains the production
Qt adapter; neither source driver is a production remapper. Both suites also
retain zero/copied/explicit destination metadata controls. Their deliberately
incorrect copied-source case still yields `key=k, code=KeyP`; explicit virtual
K yields `KeyK`.

Earlier X11 attempts remain recorded. `p01-native-platform-xcb-1` rejected
NumLock under an overly strict source-state guard. `xcb-2` passed the one-shot
and raw rows but its initial persistent row observed no DOM events. The final
helper adds the reviewed native focus guard; `xcb-3` passes every row. The
initial delivery failure's exact cause is not proven. No earlier failure is
relabeled as passed.

These are real protocol/compositor source events, not physical hardware
qualification. Standalone Xorg, the user's full desktop path, other layouts,
IME/AltGr and the full destination/context/receiver matrix remain P10/P16 work.
The P00 historical Weston/input prerequisite outcomes are unchanged.

## Separate media and fullscreen outcomes

`sitePolicyNativeMedia` uses a local one-second PCM WAV and explicitly requires
a gesture. Playback first rejects with `NotAllowedError`, then resolves after
mapped K and reports the media element playing. This proves the observed API
outcome; audible device output is not claimed (the Wayland log retains ALSA
output-device errors).

`sitePolicyNativeFullscreen` separately disables and enables Qt fullscreen
support. Disabled support rejects with `TypeError` and emits no Qt request.
Enabled support emits one accepted Qt request, resolves the DOM fullscreen
promise and sets `document.fullscreenElement`; the test then exits fullscreen.
It does not assert native window presentation.

These capability cases use constructed Qt source events on each backend. They
are separate from the three native source-input cases and from trust/activation
observations. Both native-platform suite logs retain their exact outcomes;
`p01-native-capabilities-offscreen-1/record.json` also records all five focused
capability/init/cleanup cases passing. No website-wide permission or media
compatibility claim follows from this experiment.

## Production BrowserUi/QML integration

`p01-native-production-4/record.json` records a qualified production engine build,
its exact Cargo-selected generated header directory/archive hash, application
harness build and nine passing QtTest cases, zero failures/skips, in 11.961 s.
The source archive and full repository manifest bind the build/test inputs;
the source postcheck is exact. These are constructed Qt source events through
the production application, not the native fixture's simulated mode bindings.
Only temporary `p→k` configuration is installed from C++; public commands,
binding admission, input epochs, mode changes and completion use production
BrowserUi and QML.

Five gesture rows cover persistent/one-shot command entry, ordinary U binding
entry and its release, modifier-only input, autorepeat release artifacts,
mapped repeats/real release and an overlapping J gesture. Releasing selected
J returns the application to Normal while the admitted mapped P route retains
its repeat and release. Every K has the expected code, modifiers, repeat flag
and trusted observation, without P or entry-U leakage.

The actual page-confirmation request suspends passthrough and cancels the
mapped output while P remains held. Repeats are consumed during the prompt
and after restoration. The explicit confirm answer is false. Its trace permits
only the initial K-down and at most one authorized cleanup K-up; terminal
source release cannot change that prefix. Ordinary L input then supplies the
causal barrier, and raw retry becomes available only after the held source is
retired.

The production tab-strip close action destroys the mapped WebEngine object
(QPointer must become null) while P remains held. After the replacement is
visible and has actual DOM/Qt focus, repeat/artifact/real release produces no
P or K there. Its exact trace is `l,l`, proving the replacement receives ordinary
input while the cancelled gesture stays captured. Source/output counts return
to zero. Scoped test cleanup releases every still-held constructed source on
assertion exits before disabling the probe.

Earlier integration attempts are preserved as fixture failures:
`p01-native-production-1` used the reserved macro prefix `q` for binding entry;
`production-2` attempted a Normal/Command-only tab-close command in passthrough;
`production-3` lacked explicit replacement-receiver readiness and observed no
L sentinel. Run4 uses an ordinary U binding, the actual tab-strip route and
explicit visibility/focus preconditions. The old missing sentinel's exact cause
is not proven. No production behavior was relaxed to make these fixtures pass.

## P01 exit evidence and remaining full qualification

Both independent source-bound exit reviews approve P01-01 through P01-10
and P01-GATE, with no concrete blocker remaining. One reviewed source/contracts
and test adequacy; the other verified requirements, receipts and reproducibility.
The verdicts and exact scope are retained in `p01-exit-postcheck.json`. The final
current-harness sanitizer run, `p01-native-final-asan-1/record.json`, passed
137 native-insert cases and 24 lifetime cases, zero failures. Its three
offscreen platform-source rows explicitly skip because that backend has no
native source driver; the same rows passed on both actual source backends
above. Both CTests passed in 31.82 s. The retained `qt-test-detail.log` and its
hash preserve the individual QtTest outcomes. AddressSanitizer is enabled for
the adapter/lifetime executables, with leak detection disabled for Qt/plugins.

`p01-exit-postcheck.json` verifies the current native sources and frozen binaries,
production executable/archive identities and exact production code manifest.
The production4 manifest differs only in this subsequently reconciled evidence
document; no compiled input changed. The completed candidate proof is:

| Plan item | Direct evidence |
| --- | --- |
| P01-01 | Temporary C++-only literal configuration and router-owned admission; no held schema/policy consumer. Production BrowserUi run4. |
| P01-02 | Adapter gesture/collision traces, native-source platform rows and production BrowserUi down/repeat/up without P leakage or duplicate output. |
| P01-03 | Deliberate zero/copied/explicit metadata controls on XCB and Wayland. |
| P01-04 | Exact trust/activation traces plus separate local PCM and fullscreen enabled/disabled outcomes, with source and device/presentation limits above. |
| P01-05 | Actual asynchronous returned-clone test, subtype provenance, reentry/ownerless capture and sanitizer lifetime runs. |
| P01-06 | Production run4 entry release, modifier-only, repeat artifact, real release and overlap surviving Normal. |
| P01-07 | Focus/navigation/blocked/disabled adapter cases; production actual prompt and mapped-view destruction/replacement; synchronous router/window destruction and zero-router capture under sanitizer. |
| P01-08 | Separate source/output ledgers, all four P/K collision release orderings, unchanged-native parity, raw refusal/retry and physical-identity lost-release recovery. |
| P01-09 | X11/XTEST via Xwayland and isolated compositor-delivered Wayland source probes. No offscreen substitution. |
| P01-10/GATE | Final sanitizer and source/artifact postcheck passed; both independent exit reviews approve the documented feasibility scope. |

The full native matrix remains required. P01 establishes only unmodified P→K
in the temporary passthrough probe and its stated lifecycle/ownership paths.
P10 must replace temporary configuration with the compiled policy interface
and implement all required mapping contexts, destination classes, receiver
observations and public raw/recovery wiring. P16 still owns the complete
platform/layout/IME/installed-package matrix. No full-matrix cell is promoted
from Not run based only on this narrow probe.

The P00 original source/measurement checkout and completed records are
unchanged. P01 edits are in the router and native/application test harnesses;
preserved P02 now enters ordered revalidation, and partial P03 remains held.
