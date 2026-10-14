# Ferric unified keyboard-interaction implementation pack • v0.3

The full v0.2 vision and the targeted F-01–F-08 recommendations have been merged into one normative implementation design. The pinned Ferric baseline is `4337056f632de0c710943fa6df76c67cf14b8b47`.

**Start with [PROPOSAL.md](PROPOSAL.md).** It defines 24 contract sections and 26 unified invariants. [SOURCE_MAP.md](SOURCE_MAP.md) explains coverage and conflict resolutions. [SLICE_INDEX.md](SLICE_INDEX.md) is the readable plan; `plan.json` is its machine-readable dependency graph. There are 28 bounded slice cards and 57 planned test identities.

## One starting instruction

Place this folder at `docs/implementation/keyboard-interaction-v03/` in the intended checkout and use [GOAL_PROMPT.md](GOAL_PROMPT.md) as the agent's single starting instruction. It directs the agent through the plan and does not require one prompt per slice. No specific goal-runner API or unavailable delegation tool is assumed.

Serial work is the default. Parallel work is optional and additionally constrained by dependencies, lock groups, real file scopes and coordinator integration. Native/config/identity assumptions have explicit failure and go/no-go paths rather than being left for a smaller model to invent.

## What is deliberately different from a simple concatenation

Activation/focus correctness comes before more discovery features. Native clicks, DOM actions, proxy focus, typed configuration and scope navigation have separate capabilities but share identity and validation. Search reuses existing Hint text mode. Selector validation is atomic through a real engine parser. Scoped Grid receives a narrow local-UI authority amendment and explicit coordinate types. Existing Pass Next Key is retained, not rebuilt.

Double-click and drag are gated. Rich-editor/frame/accessibility/label/wheel work has evidence-based research outcomes; no-go does not mean a feature exists. Exact semantic activation and native point targeting have different guarantees, and the JS/native race is stated plainly.

## Files

| File/directory | Role |
| --- | --- |
| `PROPOSAL.md` | Canonical design, behaviours, safeguards and limits |
| `SOURCE_MAP.md`, `TRACEABILITY.json` | Every source section/F-ID/source invariant mapped to ownership; conflicts resolved explicitly |
| `SLICE_INDEX.md`, `plan.json`, `slices/` | Ordered dependency graph and per-slice scope, steps, tests, prohibitions and exit conditions |
| `GOAL_PROMPT.md` | One coordinator instruction for the whole programme |
| `TEST_MATRIX.json` | Planned assertions with evidence-class requirements; not executed test results |
| `progress.json` | Empty implementation progress template |
| `evidence/TEMPLATE.json` | Evidence record template; no prefilled pass results |
| `examples/new-settings.toml` | Syntactically checked proposed configuration; not accepted by current Ferric until the relevant slices land |
| `tools/validate_pack.py` | Offline, non-mutating structural/integrity/evidence-record checker |
| `tools/test_validate_pack.py` | Negative tests for the pack checker |
| `SHA256SUMS.txt` | Delivery integrity for the normative pack files |
| `PACK_VALIDATION.json` | Actual pack-check result only—not Ferric validation |
| `sources/v02-original.md` | Unmodified historical source, explicitly non-normative |

## Check the pack

From this folder:

```sh
python3 tools/validate_pack.py --integrity
python3 tools/test_validate_pack.py
```

After implementation evidence has been filled:

```sh
python3 tools/validate_pack.py --evidence
python3 tools/validate_pack.py --strict-completion
```

The last command is expected to fail on this untouched delivery because no implementation/test results exist. Integrity checking excludes mutable progress/evidence and the validation report. Deliberate specification changes require reissuing the integrity list; implementation progress does not.

## Evidence honesty

Source inspection and document reconciliation were performed. Pack structure, negative-check cases and TOML syntax were tested. Ferric itself was not compiled or exercised in this preparation session. A suspected shadow-root defect remains suspected until the assigned fixture reproduces it. All 57 test entries start `not_run`, and all 28 slices start `not_started`.

The pack is a complete merged plan, not a promise that every current website or engine can support every advanced capability. Missing infrastructure, unsupported public APIs and residual native-targeting races are explicit recorded outcomes. Production changes, commits, account actions, publication and release are governed by the goal prompt and repository rules—not by the mere existence of a task card.
