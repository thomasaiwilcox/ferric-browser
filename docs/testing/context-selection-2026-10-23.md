# CLI selection of browser-created contexts

Status: complete and locally qualified. The broader browser usability goal remains active.

The previous qualified release rejected `open --context runtime --target tab URL`
with `E_CONFIG` even after the live context catalog contained `runtime`. It also
rejected the saved context after the owned browser exited. The baseline fixture
records both failures and the published context identity in
`target/context-selection-baseline/`; it reaps its own browser process.

Explicit opens forwarded to an existing instance now let that instance validate
context/profile selectors against its committed catalog. Cold startup reads a
missing authored definition from the generated registry only when the record was
created through browser commands. The lookup shares the registry's directory,
lock and schema validation, does not rewrite the catalog, and excludes known
configuration-owned identities. Saved records are not inserted into authored
configuration, so later reconciliation retains their ownership and identity.

The instance claim holds its listener while startup selection is checked. The
IPC server starts after cold profile/context and engine validation; a rejected
startup does not publish a server thread. Existing-instance forwarding retains
its startup-only option checks and carries the original target and selectors.

Missing contexts report `E_NOT_FOUND`/status 3. Conflicting profile/context
selectors report `E_INVALID_ARGUMENT`/status 2, and an unreadable saved catalog
reports `E_STORAGE`/status 1. Live routes still enforce the requested profile's
affinity before assigning context or navigating.

The new public-process fixture creates contexts through the running browser,
waits for catalog publication, exercises foreground/background/current opens,
multiple inputs and a window in another profile, and saves real tab membership.
It then checks cold selection in both profiles with the original context IDs.
Unknown contexts, wrong profiles, removed authored definitions and corrupt
catalogs must fail without changing the existing tabs or saved registry.

Frozen source and qualification results are recorded under
`target/context-selection-`. The CI gate is `application_context_selection_cli`;
hosted CI has not been run.

## Final verification

The final seven-package all-target release build passed in 9 min 43 s. All
**1,078 Rust unit checks** passed: Core 170, IPC 36, storage 113, runtime 9,
application 123, Qt adapter 510 and CLI 117. One optional Qt integration test
remains ignored. Strict seven-package all-target release Clippy passed with
`-D warnings` (19.92 s reported by Cargo). Formatting and whitespace checks pass.

| Check | Result |
| --- | --- |
| Public context selection, warm and cold | 19 scenarios passed |
| Context reload and ownership | 33 native checks passed |
| Resource parity, catalog bootstrap, failed-profile retention and window routing | 11 native checks passed |
| CLI closed pipes, failed writes and accepted receipts | 32 process cases passed |
| Existing public CLI destination journey | 10 steps passed |
| Startup recovery, native confirmation after real process restarts | 2 scenarios passed, 2 restarts each |

The context process fixture launches and reaps three owned browser processes.
The non-default profile uses the explicit user configuration; the default
profile's cold launch removes `--config` and uses built-in configuration. Both
retain their original created context IDs and selected profiles. Saved membership
is present before each restart. The ten owned document URLs each reached the
loopback server exactly once, including the two new cold destinations; saved
pages were not fetched again merely to enter the context before those opens.

The first process run exposed a separate registered `profile-create` IPC command
failure. That behavior is preserved in
`target/context-selection-profile-create-failure/` and the follow-up list. The
fixture creates its second profile through the supported public
`open --target window --profile work URL` path, then creates its context through
the public context command. No production code was changed after the frozen
release build; only that Python fixture was corrected and extended to cover the
original built-in-config setup. Rust and native results remain tied to identical
implementation and C++ fixture sources.

The audit verifies all 691 frozen source/fixture/CI files, exact Rust test
executables and the production CLI, native runners, Qt engine archive and test
plugins. The native CMake cache selects the canonical release archive. The
context, CLI output and destination fixtures all report the same final CLI hash.
The new gate appears once in the actual 35-entry CI browser-surface selection;
hosted CI was not run. All owned browser processes are reaped.

Authoritative evidence is `target/context-selection-final-facts.json`,
`target/context-selection-artifact-hashes.json`,
`target/context-selection-source-hashes.json` and their linked logs. The broader
goal remains active, with the separately reproduced profile-command execution
and rejected-command repeat issues recorded for follow-up.

## Profile lifecycle command follow-up

The public CLI fixture now creates its second profile with
`command -- profile-create work` before opening that profile's window. The
previous binary rejects that command with `E_INVALID_ARGUMENT` and status 2;
its successful context routing alone did not cover profile creation.

Interactive and IPC profile commands now share the existing profile workflow
and registry validation. IPC validates its window route and rejects profile or
context selectors for these lifecycle commands. Creation still uses the
existing background mutation worker, and deletion retains its confirmation
workflow.

The updated public-process fixture passes all 20 scenarios, requiring the new
profile to appear in the saved registry before it is opened. Its existing warm
and cold context checks then use that created profile. The public Repeat
regression also passes; rejected-command repeat had already been corrected by
the separate repeat/macro admission work. The compiled workspace test binaries
pass 1,169 tests, with two opt-in tests ignored. Final qualification details are
recorded in `target/review-issue-fixes-final-facts.json`.
