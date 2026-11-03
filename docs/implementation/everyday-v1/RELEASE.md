# Completion and capability release gates

## Completion means delivered behaviour

This pack's scope includes hot preparation, explicit phone registration/sign-in and configurable page windows with Hyprland grouping. Flags, a QR drawing, native window rendering, pure mocks, or safe refusal of all requests do not satisfy those outcomes. Preserve the stable feature-off internal browser throughout.

A source implementation and an advertised capability are separate states. Initial features remain off; internal presentation remains default. Enable a capability for development only through a deliberate option and report unqualified status. No automatic promotion based on merged code, documentation or source-string tests.

## Capability-specific evidence

- **Hot internal:** single-spare/side-effect/isolation/teardown tests, native hot-vs-cold benefit within C03 budgets and no unacceptable memory/CPU regression on the intended constrained class plus a second machine. Offscreen timing does not qualify latency.
- **Hot windows:** all hot invariants plus unmapped-host/no-flash/native-focus testing and separate timing/memory evidence. Internal results cannot be reused as proof.
- **Phone top-level:** accepted isolated-world/native-document proof; compatibility and native-provider tests; bounded supervised helper with compliant egress; adversarial/fuzz fixtures; independent security review; iPhone and Android create/get verified by an independent RP; actual native-key coexistence. Registration uncertainty is not automatically retried.
- **Native page windows:** both commands and everyday keyboard tools, privacy, popup/opener, history/recovery, background opening and first-host closure; generic window mode remains useful without Hyprland.
- **Hyprland grouping:** exact current host mapping, real supported group operations, foreground/background/standalone distinctions, external regrouping/mixed groups/locks, focus races, compositor restart/address reuse and native user-task tests. Standalone degradation is not proof grouping works.

Final code should include the scoped implementations even if external qualification is blocked. Do not conceal the remaining blocker. Leave the relevant switch gated and deliver the exact resumption steps, physical fixture setup and necessary evidence. Other features can be individually qualified and delivered without misrepresenting the whole programme as complete.

## Final source and evidence

Freeze the final integration SHA, build its actual browser/helper and refresh affected positive qualification. Each gate records which source/build it exercised. If a later documentation-only commit follows a qualified binary, identify both and include a reviewed source-dependency justification; never overwrite receipt SHAs. If a later code change can affect a gate, rerun it.

Run the pack structural validator and its tests. Then run ledger validation against the external state:

```sh
python3 tools/validate_pack.py --state /absolute/run/control/state.json --strict
```

An initial/unqualified run must fail strict validation. `--strict` checks declared evidence, not reality independently. Reviewers still inspect traces/assertions and physical records.

Deliver FINAL_REPORT with repository/branch, implementation and qualified source SHAs, current defaults, migration/rollback, dependencies/toolchains, full test inventory/results, measured distributions, security review, supported phone/compositor matrix, unresolved blockers and concise usage instructions. Clearly list anything not executed. No push, merge, PR creation, system-wide install, public release, real-account credential enrollment or live desktop changes are included without separate user authorization.
