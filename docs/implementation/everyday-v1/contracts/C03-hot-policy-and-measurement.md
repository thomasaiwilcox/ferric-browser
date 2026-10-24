# C03 — One bounded spare and proof of benefit

One speculative spare per engine process, not per window/profile. The coordinator may prepare for the most recently active eligible normal persistent environment. An environment key includes actual native profile identity/generation, real storage partition, context isolation where applicable, relevant settings generation, presentation kind and destination Qt Quick window for internal mode. Profile name equality is insufficient. Private and ephemeral requests always take the cold path.

States: Disabled → Preparing → Ready → Claimed; Preparing/Ready may terminate Evicted. No open waits for Preparing. Misses execute cold immediately. Refill after interaction settles with coalescing; opening ten pages does not launch ten preparations. Use the existing main-loop scheduling mechanism, not periodic busy polling. Bound initial preparation timeout to 5 s and at most two preparation retries per unchanged environment before suppressing it until an explicit relevant change. These are chosen v1 limits, not benchmark results.

Evict on profile/host closure, relevant configuration change, privacy or partition change, renderer/graphics failure, shutdown and available reliable memory-pressure signals. `on` does not override safety. Do not add a periodic battery/process-scanning framework just for auto mode. Never preconnect, load a user start URL, disable sandbox/site isolation or keep a process running solely for the spare.

## Benchmark

Measure off/on using the same release binary and deterministic local page. Separate input-to-browser-input-ready, input-to-engine-ready and input-to-useful-destination-interaction. Define the primary metric before collecting results. Native start-page readiness is distinct from rendering the first website. Record cold process startup separately from already-running browser opens.

Use >=10 warmups and >=100 measured samples per steady case; randomized A/B ordering, raw samples, p50/p95 (nearest rank), errors and cache-hit rate. Include bursts and cold misses, 1,000 create/claim/close cycles, a constrained laptop and another machine. Record compositor/Qt/Chromium/GPU/display scale, process-tree proportional memory and idle CPU. Never add child-process RSS totals and call that PSS. If PSS isn't available, label the alternative and do not claim that memory gate passed.

Proposed gates retained from the proposal: primary p95 improves by both >=15% and >=15 ms on the constrained target; input-ready and cold-miss p95 regressions <=5%; incremental steady process-tree PSS <=96 MiB for the one spare; no sustained refill activity or retained-view growth. Native numbers require native execution. Do not manufacture numbers from offscreen fixtures or change thresholds after seeing an unfavourable result.

`auto` eligibility is enabled only for qualified paths. Keep default off. A measured negative experiment can close the research finding, but does not constitute delivery of a performance claim. Report supported paths and keep ordinary browsing unchanged.
