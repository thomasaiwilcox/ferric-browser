# Hint-policy and memory investigation — 17 November 2026

Policy latency grows with the logical collection evidence Ferric extracts and
verifies, even when few hint targets are visible. Memory use is consistently
higher than qutebrowser in matched fresh sessions, including on the actual
desktop. The obvious small key-only inventory shortcut improved collector
execution but missed the repeated first-label benefit gate. No production
optimisation or installation change was made.

All previous uncommitted work was reviewed and committed as `38dba93` before
this investigation changed any files. The installed executable remains source
`8c27c75a371861dc7e5040fd09226b0461a95486`, SHA-256
`0f374110d84b4a6e576fe0ebf63a1fefd883b62b7378c2ed6015154f02472e38`.

Transient Insert is intentional for searches and AI prompts. The comparison
report now includes Locked Insert and modified Enter, including Ctrl+Return,
which keeps transient editing active. Ferric forwards the combination; the
website determines whether it inserts a newline. No editing change is proposed.

## Scope and controls

- Same installed browsers as the [comparison](browser-comparison-2026-11-17.md):
  Ferric Qt 6.11.2; qutebrowser 3.7.0 with system Qt 6.12.0/WebEngine 6.11.2.
- Owned temporary profiles and deterministic local fixtures. No personal
  cookies, configuration, account sessions or browser processes were used.
- Performance experiments ran serially. No application/native rebuild occurred.
- Three paired fresh sessions in the existing software Wayland test compositor,
  with start order reversed in the middle pair; two further paired sessions on
  the actual desktop with normal rendering settings and reversed start order.
- Each memory session sampled a fresh one-tab state, eight visited tabs, then
  one remaining tab after closing seven. These were the same mixed local
  article/feed/form pages, with no hint exercise before memory measurement.
  Two-second settling intervals are short; they are not a GC/leak qualification.
- Policy experiments used the same unchanged 300-card feed within each batch.
  Five configurations, two batches in reversed order, one warmup and five
  recorded samples per configuration/batch. A diagnostic session was followed
  by a plain-resource control session: 50 recorded samples in each.
- Diagnostics used a temporary copy of one QML resource, loaded only in the
  owned process by the existing resource-registration helper. They logged
  stage timings and renderer execution. No generic inspection plugin was loaded.
  An independent V8 CPU profile used an ephemeral loopback debugging endpoint
  on an owned browser. No installed resources were edited.
- First-label results remain coarse **capture-return** timings at 640 × 480;
  capture costs roughly 40–45 ms. They are not physical presentation times.
  Diagnostic timings are explanatory, not unbiased production stage percentages.

## Policy cost, separated by feature

Medians in milliseconds. First-label figures below come from the unmodified
resource control, ten recorded trials per configuration. The other columns
come from the separate diagnostic session, also ten trials per configuration.
They are independent/nested medians and must not be added as a decomposition.

| Configuration | First captured labels | Ordinary renderer body | Policy renderer body | Native verification wall | Publication callback |
| --- | ---: | ---: | ---: | ---: | ---: |
| No interaction policy | 217 | 93 | — | — | 8 |
| Top-document page keys only | 310 | 120 | 2 | 23 | 7 |
| One home anchor | 322 | 114 | 19 | 26 | 7 |
| Numbered collection | 409 | 122 | 58 | 77 | 10 |
| Keys + anchor + collection | 417 | 112 | 66 | 82 | 6 |

All 100 recorded first-label samples succeeded. The diagnostic/control
first-label medians were approximately 216/217, 312/310, 300/322, 419/409,
and 406/417 ms respectively. This supports the broad stage ranking; the
observer and normal timing variation preclude precise per-stage savings claims.
Ordinary candidate count was nine in all diagnostic samples.

The key-only case performed **zero selector predicate steps** in policy
extraction, but still paid for retained ordinary inventory and the policy proof
pipeline. That establishes an opportunity to investigate work that is unused
when there are no compiled anchor/collection queries. It does not establish
that its whole approximately 93 ms overhead is removable.

### Collection evidence includes logical offscreen items

A separate unmodified-resource inspection of the full-policy feed recorded:

- 9 ordinary candidates;
- 300 logical collection items, of which only 5 had eligible primary targets;
- 302 rooted query facts and 1 document;
- 92,552 annotation characters plus 3,460 fragment-catalogue characters.

These are logical root facts, not 302 WebEngine frames or browser tabs.
The collection-only policy's recorded annotation debit was about 94.7 KB;
combined policy debit was about 96 KB. The debit is bounded wire-work
accounting, not allocated heap size.

Source confirms the mechanism. In
[`InteractionRootRuntime.js`](../../crates/ferric-browser-engine-qt/qml/scripts/InteractionRootRuntime.js),
`observedItems` enumerates logical items. Its `intersects` test concerns the
selected scope/entered document; it is **not viewport culling**. Ordinary
eligibility excludes offscreen primary targets while logical item/root facts
remain available to native matching. This supports ordering, nesting and
uniqueness semantics. Simply discarding offscreen items would change the proof.

A small diagnostic scaling check kept the visible target arrangement fixed
and changed only document length. Three recorded samples after one warmup per
case:

| Cards | Ordinary collection wall, no policy | Collection wall with numbered policy | Policy extraction wall | Verification wall | Annotation debit |
| --- | ---: | ---: | ---: | ---: | ---: |
| 30 | 32 ms | 129 ms | 38 ms | 38 ms | 15.1 KB |
| 100 | 64 ms | 179 ms | 52 ms | 45 ms | 35.5 KB |
| 300 | 100 ms | 298 ms | 71 ms | 84 ms | 94.7 KB |

These are collection callback endpoints rather than first-label timings. The
small, single scaling batch supports the size relationship; it is not a new
throughput/tail-latency qualification.

### CPU evidence and verification boundaries

Across six full-policy hint cycles, the V8 diagnostic sampled approximately
192 ms in `getBoundingClientRect`, 130 ms in ordinary `scan`, 100 ms in
`inventoryNode`, 66 ms in `matches`, and 55 ms in inventory `record`.
There were also substantial anonymous and `(program)` buckets. These are
sampled self-time totals across six cycles, with idle/wait intervals included
in the overall profile, not exhaustive production CPU percentages.

The result supports the existing diagnosis: DOM discovery/geometry and
inventory/evidence processing matter more than painting labels. It provides
no evidence that Rust instruction tuning is the first useful intervention.

[`verifyCollectionPolicy`](../../crates/ferric-browser-engine-qt/src/hint_request_adapter.cpp)
captures a catalogue, revalidates the dependent document, recaptures the
catalogue and projects geometry. Within
[`hint_frame_broker.cpp`](../../crates/ferric-browser-engine-qt/src/hint_frame_broker.cpp),
initial capture and recapture each perform graph checks around native document
checks, with exact snapshot comparison. The 77–82 ms verification wall includes
this whole asynchronous pipeline. It is not a measurement of pure IPC or JSON
parsing. These continuations establish different freshness boundaries; their
similar-looking work is not sufficient reason to delete a probe.

## One narrow experiment: unused key-only inventory

A resource-only candidate skipped per-node root-inventory recording for the
constrained case of a top-document, unscoped request with no compiled queries
and no entered document. It retained root begin/finish, tracked documents,
physical inspection debits, ordinary eligibility, witness recording and all
native verification. It was never installed or applied to production sources.

Two batches of ten alternating baseline/candidate samples after two warmups:

| Metric | Batch 1 baseline / candidate | Batch 2 baseline / candidate |
| --- | ---: | ---: |
| First captured labels | 312 / 295 ms | 309 / 303 ms |
| Ordinary renderer body | 116 / 104 ms | 111 / 101 ms |
| Complete collection wall | 187 / 168 ms | 180 / 168 ms |

All 40 recorded captures succeeded; each retained nine candidates and 13,147
ordinary inspections. Equal counts and sample screenshots are not a complete
identity/freshness regression qualification.

The earlier optimisation work used a repeated benefit gate of at least 5% and
10 ms at the user-visible endpoint. This candidate met it in the first capture
batch but missed it in the second. Capture quantisation limits what can be
concluded about a roughly 10 ms change. The result is a modest internal
improvement, **not a qualified shipping improvement**. Production retains the
original code; the experiment remains only in ignored diagnostic artifacts.
It does not justify a broader receiver-only pipeline rewrite.

## Memory attribution

Process-tree PSS apportions shared mappings. Each cell lists the actual fresh
session values in MiB, rather than combining them into a single headline.

| Environment / state | qutebrowser | Ferric |
| --- | --- | --- |
| Software compositor, fresh one tab | 324, 323, 323 | 363, 362, 361 |
| Software compositor, eight visited tabs | 460, 457, 457 | 519, 504, 504 |
| Software compositor, after closing seven | 381, 380, 359 | 387, 388, 387 |
| Actual desktop, fresh one tab | 341, 335 | 378, 382 |
| Actual desktop, eight visited tabs | 443, 445 | 505, 520 |
| Actual desktop, after closing seven | 353, 354 | 395, 396 |

The actual-desktop gap is approximately 37–47 MiB fresh and 62–75 MiB with
eight tabs. Thus the gap survives normal rendering, although its size depends
on state. Both browsers retain some memory after closing tabs; these short
samples do not establish a leak or guarantee full reclamation.

### Larger main process and a dedicated parser document

Fresh actual-desktop main-process PSS was Ferric 255/257 MiB versus qutebrowser
220/215 MiB. At eight tabs it was 263/262 versus 221/221 MiB. The main-process
difference therefore accounts for a substantial part of the whole-browser gap.

Ferric had two renderers for one visible tab and nine for eight; qutebrowser
had one and eight. Its debug target list exposed the visible fixture and a
separate browser-owned data URL containing `ferric-selector-parser-document`.
[`FerricSelectorValidationSurface.qml`](../../crates/ferric-browser-engine-qt/qml/components/FerricSelectorValidationSurface.qml)
creates that hidden WebEngine view eagerly and retains it to validate configured
CSS selectors using WebEngine's actual parser. The primary window instantiates
it unconditionally. The additional process has a legitimate purpose.

**A child process's PSS is not the memory that would be saved by removing it.**
Library pages shared with surviving browser processes would be reapportioned.
For example, a renderer in the fresh Ferric native snapshot had about 38.5 MiB
PSS but only 6.5 MiB private clean/dirty mappings. No renderer-removal ablation
was performed; an exact saving from making validation temporary is unmeasured.
It cannot explain away the larger main process.

### Mapping-level breakdown

With eight native tabs, anonymous PSS was qutebrowser 200/202 MiB versus Ferric
242/257 MiB. Shared-memory mappings contributed about 4.6 versus 15.3 MiB.
Qt/WebEngine, Ferric executable/resources, Python/PyQt, and other libraries
contributed the remainder, with differences in both directions. The raw process
and mapping breakdown is retained separately from the condensed results.

This locates much of the gap in anonymous memory and buffers rather than just
an extra executable mapping. Anonymous mappings can contain Qt/QML, Chromium,
V8 and application allocations; these data do not identify a Rust container,
particular cache or leak. Runtime versions, widget-versus-Quick UI, shared
library accounting, and renderer settings remain confounds. The samples are
same-origin fixtures, not eight heavy production sites.

## Missing policy roots: no confirmed desktop regression

The diagnostic full-policy run failed sealing on the media page lacking the
configured home/feed selectors. Its plain-resource control displayed hints.
A separate actual-desktop test then used a combined anchor/collection/key rule
on pages lacking those selectors: typing into the form worked and both media
gestures succeeded, at 100 ms and 700 ms gaps. This extends the earlier
key-only desktop check.

Consequently, the isolated failure is not sufficient evidence for a production
policy-engine fix. The diagnostic/compositor anomaly remains recorded and
unexplained. It must not become a website-specific workaround or a relaxed proof
check. Existing native tests already include absent containers/anchors; a future
policy optimisation should include the combined-rule case in its regression set.

## Recommended next work, in order

1. **Keep collection optimisations local to one captured request.** Measure
   repeated preparation, traversal and representation costs for the original
   evidence, then try one change at a time. A useful first candidate is sharing
   immutable native preparation across initial capture and recapture where the
   original request is identical. Preserve every fresh document/graph/geometry
   probe and work-admission ticket. Some strings already use Qt implicit sharing;
   inspect actual rebuilding/parsing costs before introducing another cache.
   The largest feature cost lies here, but the removable fraction is unproven.
2. **Treat the key-only shortcut as lower priority.** The tested version saved
   about 10–12 ms in the collector and did not reproduce the first-label gate.
   Any revisit should use the existing synchronized-frame endpoint, preserve
   target identity and accounting, and test remaps, one-shot exit, stale
   documents, cancellation, frames and mixed policies. Do not add a separate
   user-visible mode or persist cached DOM proofs across sessions.
3. **Attribute main-process allocations before redesigning memory ownership.**
   Use matched fresh/warmed states and identify retained Qt/QML/application
   allocations and buffer lifetimes. The hidden parser can be evaluated as a
   separate lifecycle experiment: recreate the same isolated WebEngine parser
   for validation, retain generation/timeout/cancellation guarantees, and test
   invalid CSS, overlapping reloads, bootstrap and first-hint latency. Keep it
   only if whole-process-tree PSS improves enough to justify the added lifecycle.
   Replacing its real CSS parser or changing site isolation would alter quality.

Preserve the existing strict eligibility, policy semantics, work limits and
trusted-input boundaries. The current evidence supports focused experiments;
it does not establish a cheap universal speedup. The 5,000-target stress case
was not reopened.

## Evidence and validation

[Condensed measured results](browser-followup-2026-11-17/results.json) contain
all policy summaries, memory snapshots by process role/mapping group, scaling
results, the inventory experiment and native policy receipts.
[Manifest](browser-followup-2026-11-17/manifest.json) records local source/log
hashes and the installed build. Full scripts, logs, snapshots, profile and
resource copies remain in ignored `target/browser-followup-20261117/`.

There were 30 memory snapshots, 100 policy timing samples, 18 scaling samples,
40 inventory experiment samples, and six CPU-profile hint cycles. All 158
recorded hint-capture measurements completed. The diagnostic absent-root failure
is separate from those feed timing samples and is not hidden by their success.
The first policy setup attempted an unsupported colon key through the owned
compositor; it was rerun using the public CLI `command -- config-reload`.
The first native-policy script import was corrected before any browser launched.

No production sources or installed files were changed. Temporary browsers,
compositors, local servers and the owned debugging connection were closed.
Documentation/JSON checks apply to this investigation; no new application test
suite or full release qualification is claimed.
