# Evidence record format

Use `TEMPLATE.json` as a fresh record. Do not leave placeholder commits, paths or results in a completed record.

A `test_results` entry has these fields:

```json
{
  "test_id": "T002",
  "level": "webengine-fixture",
  "result": "not_run",
  "command": "Record the exact command actually attempted, or explain that it was not attempted",
  "commit": null,
  "log": null,
  "reason": "Record the real missing prerequisite; this is only an example shape",
  "observations": []
}
```

Repeat a test ID for each evidence level. `passed` requires an actual sanitised log path relative to the pack. Passing record completeness does not prove an observation is true. A native Qt offscreen log cannot be assigned the native-wayland level.

The slice record's `commit` is the actual 40-character integrated or slice commit assessed; `environment` records OS, build mode, Qt/Chromium, compositor/seat, zoom/scale and suite/tool versions as applicable. Individual per-result commits can identify a before-patch baseline and an after-patch run. Record baseline failure/non-reproduction honestly.

`review_kind` is `self-review` or `independent-review` with a real reviewer record; never invent an independent review. `scope_amendments` describe actual added paths and why the contract required them.

For capability work, set `capability_decision` to `go`, `no_go` or `no_need` and reference a real Markdown/JSON report in `capability_report`. Duplicate this bounded decision in progress for scheduling, consistently. `go` requires all planned minimum evidence levels before the research gate counts as complete. An absent environment is not a technical no-go.

For S21 `not_applicable`, link the S20 no-go report and retain no production drag capability. Research-only no_need/no_go outcomes must have an inspected/experimented basis, not just all tests unavailable.

The full repository gates and their actual command outputs can be additional test_results under the relevant integration/contract IDs or separate referenced logs. Do not delete baseline failures from the ledger merely because they are unrelated.
