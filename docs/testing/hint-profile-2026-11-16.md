# Hint collection profile — 16 November 2026

The largest measured stage is ordinary DOM discovery, including the retained
inventory built for active site policies. On the traced documents this takes
roughly half to three fifths of the time from `f` to the first rendered markers.
Policy extraction and native verification add substantial work. Label publication
is comparatively small. These results identify work in Ferric to investigate;
they do not establish a fixed QtWebEngine performance floor.

## Current artifact and workload

- Source: `066cf8b` (`feature/site-interaction-policy`). Production sources match
  the previously qualified binary's compiled inputs; no implementation changed.
- Executable SHA-256: `85ceb9286b1c0e641036e2a3f991f62f1865ff3d81b028b503b760a92275ab38`.
- Qt/WebEngine 6.11.2, native Wayland, AC connected, 1280 × 720 page viewport,
  100% zoom. The existing QML interpreter safety workaround remains enabled.
- Public YouTube search results for `linux` and Reddit `/r/popular/`, top and
  1,800 px scroll. Separate normal profile with copies of the four installed
  site policies; no personal cookies, account session or userscripts. Optional
  YouTube cookies were rejected in this owned profile.
- Constructed Qt `f` key events exercise the normal binding. The endpoint is
  the first swapped Qt frame synchronized after visible hint markers exist,
  not physical-keyboard latency or monitor/compositor presentation.

## Stage timing

Medians of three detailed samples per case, in milliseconds. Ordinary wall time
includes the renderer call, parsing/serialization, queueing and return handling;
its execution column times the collector body inside the renderer. Policy wall
is extraction's renderer round trip. Verification is the complete asynchronous
native sealing step, including its own renderer probes. Publication is the
synchronous receiving callback, including Rust publication and QML updates.
Columns are nested/independent medians and must not be added as a decomposition.

| Case | First marker frame | Ordinary wall | Ordinary execution | Policy wall | Verification wall | Publication callback |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Youtube top | 354 | 177 | 152 | 80 | 35 | 15 |
| Youtube scrolled | 291 | 148 | 132 | 56 | 29 | 12 |
| Reddit top | 601 | 346 | 318 | 90 | 91 | 21 |
| Reddit scrolled | 565 | 334 | 306 | 80 | 80 | 19 |

Pure policy execution medians were about 45–63 ms. Native publication plus JSON
parsing was about 2–4 ms, within the larger publication callback. The subsequent
frame gap was about 8–11 ms. Thus drawing labels and QML publication are not the
largest measured costs in these cases. Renderer wall/execution differences must
not be described as pure IPC: compilation, scheduling and conversion are included.

## DOM work within ordinary discovery

A separate operation-count diagnostic on Reddit recorded, at the top:

| Operation | Calls | Diagnostic time, ms |
| --- | ---: | ---: |
| Rectangle reads (`getBoundingClientRect`) | 9,556 | 39.4 |
| Selector checks (`matches`) | 53,745 | 29.2 |
| Tree-walker advancement (`nextNode`) | 44,974 | 25.4 |
| Document hit tests (`elementFromPoint`) | 100 | 44.3 |
| Root lookups (`getRootNode`) | 20,785 | 7.6 |
| Direct computed-style calls (`getComputedStyle`) | 750 | 1.1 |

The scrolled diagnostic had similar volume for 34 visible hints; the top
diagnostic displayed 38. These
are method calls, not unique nodes or the native ledger's inspection units.
Computed-style property reads are not included in the direct-call row. Existing
per-pass caches and earlier offscreen filtering already reduce that old hotspot.

Counters wrap DOM operations only in the browser's isolated collector realm.
Their ordinary/policy windows match the renderer timing markers exactly. Counter
instrumentation adds work; these operation timings rank areas for investigation,
not precise production CPU shares. V8 profiles also show traversal, inventory
recording/resumption and geometry work. Large unclassified `(program)` buckets
prevent a complete CPU attribution, so no exhaustive function-percentage claim
is made.

## Original resources and same-document policy control

The diagnostic QML resources were then removed for reference runs on the exact
installed executable. A final paired experiment alternated policies enabled and
disabled on the same loaded, scrolled document (three pairs, alternating order).
Every sample retained the same document identity, viewport, scroll position,
light-DOM count and a SHA-256 fingerprint of the ordinary targets' identities,
kinds, names, URLs and rectangles. Only fingerprints/counts are retained in the
pairing receipt; target contents are not written there.

| Site | Ordinary targets | Policies enabled, ms | Policies disabled, ms | Difference, ms |
| --- | ---: | ---: | ---: | ---: |
| Youtube | 25 | 292 | 175 | 117 |
| Reddit | 39 | 431 | 273 | 158 |

This isolates a material cost of the enabled policy path on those documents.
Disabling policies changes features such as numbered site hints; it is not an
equivalent optimized implementation. The difference is not all removable work.
Separate navigations produced different page sizes/timings, so the earlier
reference batches are retained but are not used as matched speedup evidence.

## Recommended next experiment

Focus on the ordinary scan's per-node policy inventory and geometry work.
`PageInteractionScripts.js` walks and classifies physical nodes and performs
secondary-scrollable discovery; `InteractionRootRuntime.js` indexes the inventory
and converts retained nodes to weak references for resumption. Candidate
experiments are avoiding unnecessary retained entries/indexing for text/comment
nodes that cannot match selectors, and reducing redundant rectangle/selector
queries. These are hypotheses, not verified optimizations.

Keep complete selector evidence, work accounting, ordinary target coverage and
fresh action checks. Test one change against the original, keep it only if
same-target end-to-end measurements improve, and retain the existing negative
geometry/identity cases. No Qt fork, SDK change or collector rewrite is justified
by these results. The 5,000-target stress case was not reopened.

## Evidence, limits and cleanup

Raw evidence and the diagnostic tool sources are under ignored
`target/hint-profile-20261116/`. Primary receipts are `summary.json`,
`live-samples.json`, `dom-operation-samples.json`, four V8 `.cpuprofile` files,
`original-resource-reference.json`, `same-document-policy-control.json` and
`cleanup.json`. Stage hooks were temporary resource copies of the current QML;
the executable and installed files were unchanged. The resource override was
verified present for stage diagnostics and absent for original-resource controls.

There were 24 timing-comparison samples, four CPU diagnostics, four DOM-operation
diagnostics, 24 original-resource references and 12 same-document policy controls;
all reported measurements completed. Warmups and the initial narrow-window
YouTube consent pilot are excluded. Small samples, changing public content,
profiling overhead and the logged-out profile limit generalization. No p95/p99,
release performance qualification or universal site speedup is claimed.

Setup failures are preserved separately. One owned process aborted because the
initial diagnostic helper passed an integer to QtTest's character-key overload
for Escape. The core identifies the QtTest assertion/helper path. The corrected
helper uses the existing performance harness's direct QKeyEvent dispatch; all
reported measurements used it. No personal browser was running or restarted.

The owned browser and renderer group were stopped, the debugging endpoint was
verified closed, the profiling socket removed, and prior desktop focus restored.
The owned policy configuration was restored; installed binaries and personal
configuration were not changed. Production code remains at `066cf8b`.
