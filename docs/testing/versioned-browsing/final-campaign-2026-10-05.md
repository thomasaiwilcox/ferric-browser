# Versioned Browsing final campaign — 2026-10-05

This report records the source state covered by the 2026-10-05 end-of-campaign
checks. Later continuation edits are not covered unless explicitly included
here. It supersedes earlier “not run” notes only for the tests listed here. A
passing shared gate is not feature-release approval; capability flags and
unfulfilled slice acceptance criteria remain authoritative.

## Environment and overall result

- Qt / QtWebEngine: 6.11.2. Chromium base and security-patch API fingerprints
  are recorded by the runtime probes and durable capture manifests; no engine
  fork or private API was used in the shipping build.
- `cargo xtask check --locked --offline`: PASS, exit 0 on the current tree.
  QML: 196 passed, 0 failed, 8 environment-gated skips. Native archive/capture,
  cookie, profile-download, HTTPS, release-manifest and license-policy probes
  completed in the check.
- `cargo xtask test archive`: PASS, exit 0. The disposable Weston/Wayland
  native archive probe and durable-adapter follow-on passed. The integrated
  matrix covered strict/partial multi-tab captures; cancellation, navigation,
  deadlines, tab close and storage failures; sidecar and POST/redirect/frame/
  editor/selection/media cases; archive-view lifecycle; and profile switching.
  The native route refusal reported exact cancellation/retirement and
  `same_tab_retry_blocked=1`.
- `cargo test --workspace --locked --offline`: PASS, 1,228 passed, 0 failed,
  30 ignored across the nonempty suites.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`:
  PASS. After the final storage-test edit,
  `cargo clippy -p ferric-browser-storage --all-targets --locked --offline --
  -D warnings` also passed.
- `cargo fmt --all -- --check`: PASS. `git diff --check` and
  `sha256sum -c SHA256SUMS.txt`: PASS. `python3 validate_pack.py --summary`
  passes structural checks for all 96 tasks, nine phases and 275 dependency
  edges; the ledger DAG and generated appendices agree, with 44 verified, 11
  in progress, 36 blocked and five not started. `python3 validate_pack.py
  --next` returns null because no not-started task has verified dependencies
  and satisfied capabilities.

The complete final check included the latest probe changes. Benign Qt/C++
header, compositor and Chromium Mojo diagnostics appeared during native/QML
fixtures; they did not fail assertions. Eight environment-gated QML cases are
reported as skips, not passes.

## Fixes and evidence reconciled

- Live GC now preflights all referenced ready-object index sizes and their
  aggregate before opening or hashing any object. The exact 2 GiB boundary,
  over-limit case, and damaged-index worker fixture passed in the 215-test
  storage suite. This bounds work according to trusted index metadata; it is
  not a physical-volume quota or protection from same-UID path races.
- SQLite's 1,000-frame automatic checkpoint is `PASSIVE`; it copies eligible
  frames but does not truncate the WAL. A 16 MiB `journal_size_limit` is a
  retained-size target after reset, not a hard cap on growth before an explicit
  checkpoint. Successful orderly flush requests `TRUNCATE`, and reports busy
  readers as failure. Both the large-commit/flush and pinned-reader regressions
  passed. See SQLite's [WAL checkpoint](https://sqlite.org/pragma.html#pragma_wal_checkpoint)
  and [journal-size-limit](https://sqlite.org/pragma.html#pragma_journal_size_limit)
  documentation.
- Qt 6.11.2 can deliver a delayed successful finish for a refused auto-started
  page save after another save begins; directly retrying the same view can
  stall its page-save pipeline. Production code quarantines that logical tab
  before another save call. The native probe was corrected to model the exact
  tab and assert that same-tab retry is blocked; it no longer bypasses the
  production preflight. This contains the unsupported retry rather than
  making Qt's view reusable. Other tabs remain eligible.
- The first two attempts at that probe correction exposed test-harness defects:
  the test owner initially lacked the production poison-list property, then
  the refusal was placed before later raw-view cancellation probes. Both were
  corrected; the final native archive command and full gate passed.

The archive isolation fixture again recorded zero CSS/image/frame/form
callbacks and zero network hits because Qt/Chromium suppressed those requests
before Ferric's interceptors. This is a bounded no-network observation, not
proof of per-resource interceptor blocking. The saved-MHTML embedded-download
case is also suppressed before the application callback.

## Remaining blockers and scope

- All production capability flags remain false. Public Qt still does not give
  Ferric complete general cookie source state (including SameSite/Partitioned
  identity/completeness) or a complete WebRTC-absence certificate for live
  retention. Their dependent slices remain blocked; no product capability is
  advertised.
- Archive capture/view remain disabled pending their complete release gate.
  Arbitrary viewless ordinary-download delivery through the durable ledger,
  forced-owner-teardown completion, and full VB-057 acceptance remain open.
- Media sidecar URL fingerprinting remains deterministic and unkeyed; a
  product/security acceptance or reviewed key lifecycle is still needed.
- Cross-version requalification is incomplete: no second Qt/Chromium runtime
  is available, and the VB-090 integration is unavailable here.
- The 2 GiB logical budget and WAL flush behavior are not an exact disk quota.
  Same-UID races, physical full-volume behavior, mid-sync failure and power-loss
  durability remain outside the completed evidence.

The validated pack contains 96 slices: 44 verified, 11 in progress, 36 blocked,
and 5 not started. `validate_pack.py --next` reports no dependency/capability-
ready not-started slice. This is not goal completion: continue implementable
in-progress work, but do not force blocked behavior or enable production flags
without the missing evidence or backend.

## Supplemental RTC research probes — 2026-10-05

After the shared workspace gate, the current source-pinned RTC fixtures were
rebuilt and exercised independently. The native RTC fixture passed three
fresh off-the-record runs in each of Qt's default and strict site-isolation
configurations. The exact Qt Quick owner fixture passed three fresh runs. The
current native fixture includes fake audio, camera, combined audio/video,
permission denial and synthetic desktop-capture routes, plus frame delivery,
activity-interlock and renderer-loss cleanup checks. The Qt Quick fixture
includes synthetic camera, microphone and combined-capture permission paths
on the exact view, as well as lifecycle, profile-replacement and renderer-loss
checks. All runs were diagnostic, used fixed fake devices, and left production
capability disabled.

The renderer-loss guard was corrected after the first current native run
showed that the page-reported renderer can be behind a QtWebEngineProcess
helper and that Chromium's command line is not stable identity evidence. The
final harness verifies the Qt-reported target, exact isolated executable,
bounded ancestry and process start identities on both sides of `pidfd_open`,
then signals only through the bound pidfd. The corrected leg passed in every
default and strict-isolation run and observed exactly-once invalidation.

Evidence and exact log hashes are recorded in
[`tools/versioning-engine-spike/source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json)
and [`VB-033.md`](VB-033.md). These probes expand only the tested synthetic
routes; they do not establish complete WebRTC absence, physical-device
coverage, production Rust ownership or shipping qualification. VB-033 and the
production capability gate remain blocked/disabled.
