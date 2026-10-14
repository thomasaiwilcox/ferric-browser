# S25 working plan — optional labels versus text search

S25 evaluates fixed-width default labels and the existing text-search interaction against numeric, deterministic scattered-prefix, and fixed-dictionary word-pair prototypes. All prototype code remains in one opt-in integration test and uses a fixed synthetic corpus; no production label or input configuration changes.

## Patch outline and test mapping

| Test | Planned evidence and production path exercised |
| --- | --- |
| T040 | Re-run the engine gate on the current head. Rust `HintInteraction::SetText` and its shared `switcher_rank` path, the WebEngine candidate fixture, and QML identity/explicit-confirmation tests exercise the current Target Search path. No matcher or cache is added. |
| T043 | Run `tests/qml-perf/tst_hint_performance.qml` for 10 warm-ups/100 samples across the 500-target zooms and the 1,000/5,000-target scenarios; run the existing Rust Hint pipeline sampler for 10/100 samples at 500/1,000/5,000. Retain raw timings and calculate p95 from the sorted samples using the existing harness. |
| T053 | Run the opt-in `tests/label_probes.rs` against the fixed fixture. It drives real `HintInteraction` for stable/numeric/scattered/word codes and existing text search, asserts exact identity and confirmation, checks deterministic prefix-free labels, and measures a 10% replacement plus the numeric 1,000→1,001 width boundary. Modeled keystrokes/time remain separate from human error and completion-time evidence. |

The text-search query is each synthetic accessible name in full, which avoids treating a short query as unique when the existing fuzzy matcher returns additional results. The model counts one text-mode entry, each typed character (including the separating space), and an Enter confirmation; it excludes common Hint-session startup. Label schemes use the current `FullMatch` activation policy. A fixed 200 ms per gesture is only a reporting assumption.

## Boundaries and expected decision

The word dictionary is static fixture data and never comes from a page value. Search still uses Ferric's existing Hint ranking and exact target record. No page text is persisted, no human-study claim is made, and no prototype is exposed through configuration or UI. A modeled gesture reduction without an observed human error rate cannot authorize a production strategy change under C21; retain the current alphabet, width, refresh and rapid semantics.
