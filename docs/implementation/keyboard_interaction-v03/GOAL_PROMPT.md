# Single goal prompt — Ferric unified keyboard-interaction programme

Use the following as one starting instruction for the coding agent. It is a workflow prompt, not an assertion about any particular model's capabilities or an API invocation. Place this pack at `docs/implementation/keyboard-interaction-v03/` in the intended Ferric checkout, or supply its actual path to the agent once.

---

Implement the Ferric unified keyboard-interaction specification contained in `docs/implementation/keyboard-interaction-v03/`. Treat the full pack—not the archived proposals independently—as the task contract. Work through its ordered slices autonomously with durable progress and evidence, without requiring me to copy a separate prompt for each slice.

## First actions

1. Read the checkout's applicable repository instructions, then this pack's `README.md`, `PROPOSAL.md` C01–C05 and C22–C24, `SOURCE_MAP.md`, `plan.json` and `progress.json`. The archived v0.2 file is historical evidence, not an active competing specification.
2. Verify the pack using `python3 tools/validate_pack.py --integrity` from the pack directory. This validates the pack, not Ferric. Verify actual git HEAD, worktree state and the relevant source symbols against baseline `4337056f632de0c710943fa6df76c67cf14b8b47`.
3. Execute S00. Preserve unrelated work, private profiles, toolchain/dependency versions and the separate versioned-browsing branch. Do not reset, force-checkout, install, publish, push or merge on my behalf. Local commits on a dedicated implementation branch are permitted only where the environment/repository instructions allow.
4. Record the exact commands for the real Rust, WebEngine, native input, resource and Wayland suites before relying on them. An unavailable prerequisite is not a pass.

## Scheduling

Follow `plan.json` and the individual card for each slice. The default execution order is serial, ascending by ready slice ID. Do not start a production feature before its contract dependencies are implemented and their security/correctness tests pass. Missing native/live infrastructure can leave an implementation `implemented_unqualified`; independent pure/fixture work may continue, but no release claim or capability enablement may pretend the missing proof exists.

Run all core implementation slices and the bounded research slices. S21 runs only if S20's capability decision is `go`. An evidenced S20 `no_go` makes S21 `not_applicable`; an undecided or environment-blocked S20 leaves S21 `blocked_environment` with drag unavailable. A research `no_go` or `no_need` is a useful result, not permission to replace the architecture or declare the feature shipped. S19 and other native capabilities remain gated by their specified evidence. Research prototypes for editor/frame/accessibility/labels/wheel do not silently become production features.

Do not use parallel agents by default. Optional delegation requires tools actually available, separate worktrees, satisfied dependencies, disjoint lock groups **and** disjoint actual file scopes. Shared runner/registry/resource registration changes belong to the coordinator. Never let parallel workers overwrite shared DOM helpers, config commit logic, Hint schemas, key routing or generated documentation. A self-review is labelled self-review; do not claim an independent reviewer existed when none did.

## Per-slice working cycle

Read the slice card and only the named canonical contracts plus the existing relevant source. Check actual method names and ownership boundaries; do not implement fictitious API calls from conceptual type sketches. Write a short intended patch outline and map each planned test to the production path it exercises.

For suspected defects, add the reproduction first and run it against the baseline when feasible. Record actual red/green behaviour. If the suspected defect does not reproduce, record that and retain only justified, test-backed refactoring. New features can be unsupported at baseline; do not pretend that is a pre-existing failing test.

Implement the smallest cohesive slice. Existing helpers and authorities are reused: one Hint identity registry, one text-ranking model, one explicit scroll policy, one key router and one native pointer adapter. Default browser interactions remain stable. New data is bounded and typed. New effects enter through the existing reducer/command/operation authorities; QML is not a new policy engine.

Test positive and negative cases, not only happy-path screenshots or source-string assertions. Run the applicable repository gates including the CLI crate, WebEngine/native suite, generated docs and resource checks. Use the canonical TEST_MATRIX IDs in evidence. Native-event fixtures, offscreen tests, actual Wayland and live-site checks must be labelled separately.

Before finishing the slice, perform an adversarial review of exact element/paired-target identity, owner/tab/document/frame/config/epoch boundaries, native-event provenance, at-most-once effects, callback and map teardown, privacy, default behaviours and parser/CLI/IPC parity. Fix any violation before proceeding. An optimisation must show comparable before/after measurements rather than remove safeguards to look fast.

Write `evidence/Sxx.json` using `evidence/TEMPLATE.json`, with real commits, actual commands, outputs/log paths, results and limitations. Update `progress.json`. Never prefill passing evidence or count not-run tests as passes. Re-run pack consistency checks; evidence checks validate records, not the truth of logs.

## Non-negotiable boundaries

No universal WebApp mode, duplicated Pass Next Key, second search engine/DOM registry, arbitrary browser automation endpoint or page-supplied privileged bridge. Do not globally replace DOM click with native click, or retry a native action after uncertainty. Selector configuration must use the effective captured policy through final validation and atomically commit only after engine syntax validation. It never grants proxy or native-input powers.

Keep editor-proxy focus distinct from Bootstrap label-control activation and from rich-editor writeback. Do not focus an arbitrary hidden field, mutate CSS/tabindex/disabled state to force eligibility, or overwrite rich content by guessed framework internals.

Keep routing ownership distinct from native recipient focus. Preserve original Qt events; do not replay a key into another tab to manufacture balanced delivery. Native pointer preflight is not atomic with renderer hit testing. Apply the specified checks and disclose the residual race instead of asserting that it cannot happen.

Grid's scoped geometry must distinguish DOM CSS coordinates, Qt ViewLocal coordinates and device pixels. The only new pointer-entry privilege is the narrowly specified physical local UI permit handoff. CLI/IPC/macros/userscripts/generic remote actions cannot acquire it.

## When evidence or assumptions fail

Use the exact fallback in the relevant contract. If a public engine API is absent, document a capability no-go and keep the dependent feature unavailable. If an experiment cannot run because the environment is missing, record blocked/unqualified—not a technical no-go and not a pass. If the contract is genuinely contradictory or insufficient for a security-sensitive decision, set `blocked_specification`, state the precise issue and continue independent ready work. Do not guess, weaken a check, silently expand scope or stall the whole programme on an unrelated task.

Unrelated build failures remain recorded baseline issues. Do not upgrade the toolchain or lockfile, suppress warnings, skip tests or modify production data to get a green report. Live journeys use only disposable, authorised accounts/documents; do not send real messages, share files, purchase anything or change private production state.

## Completion

S27 performs a fresh integrated audit. Report which slices are complete, implemented but unqualified, blocked, research no-go or conditionally not applicable. State the integrated commit, changed capabilities/defaults, actual tests, measurements, native/live gaps and risks. Source/fixture proof is not a universal site-compatibility promise.

Deliver the coherent local implementation, updated documentation, test/qualification artifacts and progress ledger. Do not publish, merge or install into my daily browser without separate authorisation.
