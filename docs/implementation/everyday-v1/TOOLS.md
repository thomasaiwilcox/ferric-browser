# Using the pack tools

Python 3.11+ standard library; Linux is the execution target. No pip packages are required for these orchestration utilities. Browser/helper/test-RP builds have their own dependencies described in ENVIRONMENT and TESTING. Run from the unpacked/installed pack, or pass absolute script paths.

## Initial checks and setup

```sh
python3 tools/validate_pack.py
python3 tools/test_tools.py
python3 tools/bootstrap.py --workspace "$HOME/ferric-everyday-work"
```

Bootstrap is a real clone operation when executed by the agent. It needs network access and ordinary Git access. It is intentionally not pre-run against the user's machine. Existing-checkout mode is `--repo /absolute/clean/repo`. No setup mode overwrites dirty files or an existing integration branch.

Bootstrap creates RUN_CONTEXT.json plus external `control/state.json`, `control/capabilities.json`, `control/test_results.json`, `control/builds.json`, `evidence/`, `packets/`, `builds/` and `worktrees/`. Read those exact paths when resuming. The source pack's `*.initial.json` files remain immutable examples.

## Preflight, readiness and packets

```sh
python3 tools/preflight.py --repo /absolute/repo --output /absolute/run/evidence/preflight-a1.json
python3 tools/ready.py --state /absolute/run/control/state.json
python3 tools/make_packet.py H00 --output /absolute/run/packets/H00-a1.md
```

Readiness is advisory. It checks accepted dependencies, active named locks and exact file overlaps, then suggests a compatible batch. It does not change state, claim locks, check hardware or spawn agents. The coordinator must also reserve build/native/physical resources and proof gates. Use `--plan /absolute/execution-plan.json` after a reviewed plan amendment; changed task files/contracts/matrix must be kept consistent in the installed pack before validation and packet creation.

A packet contains WORKER.md, its complete slice and required contracts, capped at 24,000 UTF-8 bytes. It does not contain the entire repo. The coordinator adds a small header identifying the current worktree/base/integration SHA and resolved paths. That header must fit the same context budget. Read source under the documented separate retrieval limit.

## Command receipts

```sh
python3 tools/run_gate.py --repo /absolute/repo \
  --out /absolute/run/evidence/H00-a1/unit --label H00-unit \
  --timeout 1800 --require-tests -- cargo test -p ferric-browser-core --locked
```

The wrapper runs exactly one command synchronously, terminates only its owned process group on timeout/output overflow, and writes output.log and receipt.json. Output is bounded to 64 MiB; overflow is failure, not silently truncated success. It recognizes common Rust, Qt, CTest, pytest and unittest summaries but cannot understand all test formats. For an unknown format, keep the unqualified raw receipt and add a reviewed parser or structured report; do not forge counts. No executed tests or reported skips fail `--require-tests`. Clean source, matching source SHA and review are separately required for acceptance.

Use a new output directory per attempt. Format/lint/build commands do not use `--require-tests`. Do not wrap remote writes, global install scripts or live personal-account tests under the pretext that this tool authorizes them. The wrapper only executes an explicitly requested command; repository/task authority still limits that command.

## Acceptance ledger

Only coordinator updates external state, using atomic writes (common.write_json is provided). Example record after actual integration and review:

```json
{
  "status": "accepted",
  "attempts": 1,
  "author": "actual-worker-session",
  "reviewer": "different-review-session",
  "source_sha": "40-character-integrated-source-sha",
  "evidence": "evidence/H00/accepted.json",
  "blocker": null
}
```

This is an example, not valid evidence. Fill templates/EVIDENCE.json using real T* cases, actual receipts and hashes. Evidence and review paths are relative to the external workspace. `command_receipt` points to run_gate's JSON; `report` points to reviewed output; `qualification_record` points to a filled QUALIFICATION_RECORD.json. Hash files with SHA-256 after writing them. Review decision must be `accept` for the integrated source. Don't count a synthetic validator fixture as feature evidence.

```sh
python3 tools/validate_pack.py --state /absolute/run/control/state.json
python3 tools/validate_pack.py --state /absolute/run/control/state.json --strict
```

Non-strict checks all currently accepted records. Strict also requires every task accepted and final capability records at the final integration SHA. It is expected to fail on initial or unqualified state. Physical/native/benchmark cases need explicit actual-environment records; a unit receipt alone cannot satisfy them. Checks are structural integrity checks, not independent proof that a test or physical event occurred.

## Registering and cleaning build artifacts

A build is eligible only if it was deliberately created under `/absolute/run/builds/NAME` and marked at creation. Copy templates/BUILD_OWNER.json to its `.ferric-build-owner.json` with the current run ID and owner. Add an entry to external `control/builds.json`:

```json
{"run_id":"actual-run-id","artifacts":[{"name":"native-integration","active":false,"retain":false}]}
```

Mark active=true and create `control/build-active.lock` before the build. Clear them only after the owned process has ended. Root repo target/, global Cargo caches and source worktrees are not eligible for automatic cleanup.

```sh
python3 tools/cleanup.py --workspace /absolute/run
python3 tools/cleanup.py --workspace /absolute/run --apply --artifact native-integration
```

Default is preview. Apply requires explicit eligible names. The tool rejects mismatched ownership, active builds, symlink-controlled targets and path escapes. It uses fd-safe tree deletion and does not follow symlink targets within a registered build tree. Retain logs/evidence before cleanup. Missing ownership or ambiguous process lifetime means leave data alone.
