# Authoring and local validation report

Prepared 24 October 2026.

## Source research

Ferric main was read through the connected GitHub API and pinned to `e4aa108f9d63c99c3b3e977c2061d6d8dde48568`. A final ref read during packaging returned the same SHA. This is a targeted source/integration review, not a complete browser security audit. Source manifest entries describe full versus partial reads. The older proposal is retained unchanged and explicitly superseded where the source evolved.

No Ferric source was changed, built, installed or executed during this authoring task. A local network clone attempt was unavailable; actual source research used the GitHub connector. The supplied bootstrap's live GitHub clone therefore has not been demonstrated in this container. Its existing-checkout workflow was tested against disposable synthetic Git repositories with network fetch mocked; hook suppression and dirty/wrong-remote/branch conflict protection were tested. There were no remote writes.

## Tests actually executed

`python3 tools/test_tools.py`: **41 tests passed**. See tool-tests.log. These are tests of the implementation-pack tooling, **not Ferric browser tests**. The suite covers structure, dependency cycle/unknown reference rejection, all worker packet budgets, initial-state honesty, lock/file/capacity scheduling, path escape rejection, safe bootstrap with synthetic repos, preflight provenance, process result/timeout/log-bound handling (including an exited leader with a live child), zero-test/skipped/failed-summary rejection, source dirtiness capture, protected artifact cleanup, evidence hashes, separate reviewer requirement, and missing native/physical evidence.

`python3 tools/validate_pack.py`: passed structural validation for **55 work orders, 165 acceptance specifications and 15 contracts**. The dependency graph is acyclic and each task has consistent requirement/test mappings. Initial acceptance cases remain `not_run`.

`python3 tools/validate_pack.py --state control/state.initial.json --strict`: **exit 1, as expected**. An unstarted pack is not qualified software. See strict-initial-expected-failure.log.

All 55 complete worker packets were constructed in memory and checked against the 24,000-byte cap. Largest packet: **15,486 UTF-8 bytes**. Bootstrap goal: **732 UTF-8 bytes**, below the pack's chosen 1,000-byte budget. These are pack budgets, not a claim about a platform's current goal-field limit.

A P14 packet was separately written and initial scheduler output was checked: only S00 is ready. Python source compilation checks passed. Local Markdown links were checked, excluding the archived original proposal's historical external/repository links.

## Outstanding implementation and qualification

No product task has been performed. No compiled Qt ABI/native surface, Hyprland grouping, Bluetooth, iPhone/Android ceremony, real security key or browser performance result is asserted. The pack explicitly assigns these tests and prevents unit receipts from standing in for native/device records.

The ledger checker verifies structure, source references, declared evidence classes and file hashes; it cannot independently establish that a physical observation occurred or a test assertion is correct. Fresh review and actual hardware qualification remain essential.

The archive contains no Ferric source checkout, build products, credentials, personal profiles, secrets or font files. Packaging excludes Python bytecode caches.
