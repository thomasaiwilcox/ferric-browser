# Qt 6.11.2 QML JIT safety workaround

Ferric forces Qt's supported interpreter mode before constructing QApplication
when compiled with the qualified Qt 6.11.2 SDK. This avoids the defective QML JIT
SetLookup path. It also disables QML AOT C++ execution. WebEngine's V8 page
JavaScript, native key routing and the SDK/dependency selection are unchanged.

Qt's upstream fix is [4c1054b0f997f2977a54d42523856805b309db9a](https://github.com/qt/qtdeclarative/commit/4c1054b0f997f2977a54d42523856805b309db9a),
“V4: Keep the accumulator intact across the JIT's SetLookup”. The pinned SDK has
the defective InAccumulator calls for void SetLookupStrict/Sloppy functions.
The fix ignores the return register and restores the accumulator. Qt describes
how the old code can expose garbage to the collector.

The live preview crash retained in `p13-live-native-20261115-5` has a GC mark-stack
pointer into a plain iterator-result object's continuation slot. That is
consistent with the upstream mechanism, but the original application's precise
cause has not been uniquely reproduced. The workaround removes a confirmed
corruption path; successful replays alone are not proof of the original cause.
The small standalone assignment checks passed both modes and are not presented
as a causal failing regression. The first iterator diagnostic used an unsupported
matchAll API; its TypeError is a diagnostic limitation, not an application defect.

Use [Qt's documented configuration](https://doc.qt.io/qt-6.11/qtqml-javascript-finetuning.html)
for the fallback semantics. In particular, threshold -1 does not disable JIT.
Ferric explicitly sets QV4_FORCE_INTERPRETER=1 before application creation even
when QV4_JIT_CALL_THRESHOLD=0 is inherited. A startup regression checks this in
fresh production application test processes.

Remove this workaround only after qualifying an SDK containing the upstream
fix and repeating the affected editor/capture/preview and input checks. No
performance pass is claimed for interpreter mode. The owner's 15 November
scope decision stops further extreme-load optimization; historical failures
remain recorded.

Evidence: `target/site-policy-p05-revalidation/p16-crash-editor-repro-1`
(upstream source/commit and diagnostic limits), `p16-qml-jit-safety-1`
(production build/startup/capture/preview checks),
`p16-interpreter-qml-checks-1` (49 QML checks) and
`p16-jit-native-regressions-1` (105 native checks before the final modal fix).
The crash-focused build passed 1,318 QML checks, 17 native editor/page-test checks,
15 owned-Wayland checks and nine focused native checks. The subsequent pre-PR
build includes the reviewed nested-input fix and passes 1,319 QML checks plus
18 native editor/page-test/resource-parity checks. Exact artifacts and scopes
are recorded in [final qualification](site-policy-qualification.md).
