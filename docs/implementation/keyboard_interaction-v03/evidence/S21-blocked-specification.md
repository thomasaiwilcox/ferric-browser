# S21 blocked by conflicting conditional-status rules

> Historical audit. The [completion report](completion-report.md) records later repairs and application coverage. The S21 specification contradiction is resolved; its current status is `blocked_environment`.

S21 cannot proceed because S20 has no positive `go` decision: S20 is `blocked_environment` after the nested Weston runner failed to expose the virtual-keyboard protocol required for native input. The offscreen S20 feasibility probe is not a technical no-go and does not authorize production drag behavior.

The pack gives incompatible instructions for this exact state:

- `GOAL_PROMPT.md` says S21 runs only if S20 is `go`; otherwise mark S21 `not_applicable`.
- `slices/S21.md` says to mark S21 `not_applicable` when S20 is not `go`.
- `evidence/README.md` says conditional S21 can be `not_applicable` only with S20 no-go evidence.
- `tools/validate_pack.py` enforces that an `not_applicable` slice's predecessor status is exactly `research_no_go`.

Here, S20 is environment-blocked and has no capability decision. Marking S20 `research_no_go` would misstate missing platform evidence; marking S21 `not_applicable` would contradict the evidence contract and fail the pack validator. Following the pack's instruction for a contradictory security-sensitive gate, this record marks S21 `blocked_specification`, keeps drag unavailable, and records all S21-specific drag tests as not run. Existing general T010/T055 checks are retained as partial shared-contract evidence; they do not qualify a drag implementation.

To unblock S21, the specification must state whether a missing S20 environment means (a) S21 remains blocked until S20 can decide go/no-go, or (b) S21 is not applicable without a technical no-go, and update the evidence contract/checker consistently.
