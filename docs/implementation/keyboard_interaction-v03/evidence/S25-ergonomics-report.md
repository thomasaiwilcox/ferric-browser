# S25 — Optional label and Target Search evaluation

## Result

No label strategy is promoted. The current fixed-width, prefix-free labels and existing Hint text-search interaction remain the production behavior. The deterministic model found a numeric-label keystroke reduction at 1,000 candidates, but the reduction is synthetic and the numeric width change at 1,000→1,001 candidates relabels every retained target. No human task or error-rate data exists to satisfy C21, and the fixed word-pair prototype is longer in the model.

## Method and boundaries

The opt-in `tests/label_probes.rs` harness uses 12 fixed synthetic control tasks at 500, 1,000 and 5,000 candidates. It calls Ferric's production `assign_labels`, `refresh_labels` and `HintInteraction` APIs for default and numeric labels and existing text search; scattered-prefix and fixed-dictionary word-pair labels are test-only prototypes. Text search uses each full synthetic accessible name, then explicit activation. The word dictionary is fixture data and never derives from a page. Modeled time is 200 ms per typed character or confirmation gesture; it is not measured user time.

All model tasks activated the intended exact candidate, and every label set was deterministic, unique and prefix-free. “Zero modeled mismatches” measures the test model, not user error.

| Candidates | Default labels | Numeric | Scattered | Word pairs | Existing text search |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 500 | 3 gestures / 600 ms | 3 / 600 ms | 3 / 600 ms | 13 / 2,600 ms | 14 / 2,800 ms |
| 1,000 | 4 / 800 ms | 3 / 600 ms | 4 / 800 ms | 13 / 2,600 ms | 14 / 2,800 ms |
| 5,000 | 4 / 800 ms | 4 / 800 ms | 4 / 800 ms | 13 / 2,600 ms | 14 / 2,800 ms |

The numeric prototype saves one modeled gesture only at 1,000 candidates. At the 1,000→1,001 width boundary, it changes all 1,000 retained labels; the current default changes none. For 10% candidate replacement at each tested scale, all strategies preserve every retained label in this synthetic fixed-width case. Scattered labels reduce the model's longest same-first-key run to one, but that distribution statistic says nothing about readability or comfort.

The result does not meet C21's human-evidence trigger. Numeric labels could be considered for a separate user evaluation, with the width-boundary churn made explicit; there is no justification to add a production option now. Search is already available through the shared Hint matcher and explicit confirmation path, so S25 adds no searcher or cache.

## Performance and T040 evidence

The full offscreen QML/WebEngine matrix passed with 10 warm-ups and 100 samples per row. The raw callback and overlay samples and nearest-rank p95 calculations are in `S25-qml-perf.log` and `S25-qml-perf-summary.json`; the Rust-stage raw samples and summary are in `S25-rust-perf.log` and `S25-rust-perf-summary.json`.

Compared with S17's same-host run, all 500-target callback medians were higher. In the full S25 run, p95 increased from 81.72 to 92.14 ms at 80% zoom, from 74.31 to 86.21 ms at 100%, and from 73.68 to 94.03 ms at 125%. Focused repeat runs recorded 92.71, 83.80 and 83.76 ms respectively. The 80% and 125% repeat rows exceed both C17's 10% and 10 ms investigation thresholds; the 100% repeat is 12.8% higher but 9.48 ms above baseline, while the first full run was 11.90 ms above. The size-only 1,000, 5,000 and mutation rows were also higher than S17 and remain separate scale results.

The 500-marker overlay time-to-visible p95 was 224.29, 220.75 and 212.60 ms at 80%, 100% and 125% zoom, above C17's 200 ms reference ambition. Corresponding layout-compute p95 values were 169.79, 168.77 and 161.75 ms. The 5,000-target Rust pipeline p95 was 99.38 ms decode, 2.83 ms labels, 1.62 ms interaction setup, 70.82 ms ranking and 0.13 ms snapshot; these remain close to S17's 101.31, 3.16, 1.67, 69.24 and 0.13 ms.

The S25 benchmark used the same i5-7200U, Linux 7.2.3, Qt/QtWebEngine 6.11.2 and Chromium 140 identity as S17. Host load averages observed immediately after the full run were approximately 2.56/2.74/3.03 on four logical CPUs. The focused 80% repeat reproduced the initial p95, while the other two repeats varied. S25 changes no runtime performance code, so the timing difference is not attributed to a code change; the threshold crossing and above-ambition overlay result remain unqualified pending a controlled low-load run.

The fresh engine gate passed 441 engine tests (one existing ignored sampler), all 144 QML/WebEngine tests and native CTest 1/1. This re-exercises T040's existing matcher, WebEngine fixture and QML explicit-confirmation path on the current source tree.

## Limitations

This is a deterministic synthetic comparison, not a human study, site-compatibility claim, user error measurement or completion-time measurement. Text queries are exact full control names; natural tasks may use shorter or ambiguous queries. The word-pair alphabet and labels exist only inside the opt-in test. No default, runtime UI, parser, or configuration changed. The offscreen software renderer and host load limit the performance conclusion; the callback threshold crossing needs a controlled repeat before release qualification.
