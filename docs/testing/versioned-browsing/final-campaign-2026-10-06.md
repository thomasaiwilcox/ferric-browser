# Versioned Browsing verification continuation — 2026-10-06

This report records the final workspace-gate run for the current executable
source tree after the 2026-10-06 continuation changes. It supersedes earlier
“not run” notes only for checks explicitly listed here. The implementation
pass is still active: this is not final goal reconciliation, pack validation,
or feature-release approval.

## Results

- `cargo xtask check --locked --offline`: PASS, exit 0. The QML suite reported
  223 passed, 0 failed, and 8 capability/environment-gated skips. Native
  profile-download, archive/capture, cookie, HTTPS, release-manifest and
  licensing probes completed where supported; workspace Rust tests and doc
  tests passed.
- `cargo xtask test versioning-profile-download`: PASS. The isolated Qt Quick
  fixture exercised exact same-profile Save Page capture routing for source
  and restored-child owners when Qt exposed a null view, and a separate live
  child-view attachment streamed bytes before cancellation. The probe uses
  the production owner/path routing helpers and exact permit. This is not a
  witness for an ordinary content download arriving without a view or for a
  durable Ferric ledger row from such a request.
- Native archive/capture journeys passed, including durable-adapter callback
  cases, stable and mutating documents, partial multi-tab outcomes,
  navigation/deadline and view-loss refusal, collector/archive failure
  separation, POST/redirect, frame/editor/selection/media recovery, storage
  full cases, cancellation and archive-view lifecycle.
- The active-runtime compatibility API and exact-fingerprint regressions ran
  as part of the Rust suite. No second Qt/Chromium runtime was available for a
  cross-version transition matrix.
- Separately, the source-pinned RTC admission fixture passed three fresh
  off-the-record runs each in default and strict-site-isolation modes, and the
  exact Qt Quick owner fixture passed three fresh runs. A subsequent focused
  rerun of that fixture creates and closes its data-only peer in
  `ApplicationWorld`; all three fresh runs passed and are recorded with the
  updated fixture digest in `source-lock.json`. These cover only bounded
  synthetic-device/media, owner/lifecycle, ApplicationWorld peer-registration
  and renderer-loss cases; they do not prove complete WebRTC absence or enable
  the production live-retention capability.
- `cargo fmt --all` was applied after the gate identified three formatting
  differences; the subsequent complete gate passed. No standalone strict
  Clippy or pack-checksum result is claimed by this report.

## Corrected findings

The Save Page fixture no longer assumes that a Qt `SavePage` request must
expose a view. On the tested Qt 6.11.2 runtime, both source and restored-child
Save Page requests expose a null view, and the exact same-profile private
capture path identifies them without consulting that getter. Ordinary
attachments remain on their separate per-view stream/cancel route. This
qualifies these exact-permit Save Page paths only; it does not generalize to
arbitrary page-less `downloadRequested` delivery.

The owner-routing hardening now snapshots caller-controlled own properties
once and invokes an own getter at most once before using its value. Focused
QML regressions and Rust source-boundary checks passed in the full gate. The
download router still fails closed on malformed, duplicate, inherited, or
throwing ownership state.

## Remaining limits

- The MV3 extension-based ordinary viewless-download journey remains skipped:
  extension activation crashes on the tested Qt 6.10.3 and 6.11.2 runtimes.
  A tested Qt 6.12/WebEngine 6.140.0 release-candidate build also crashed
  during extension activation. Public `QWebEnginePage::download()` works but
  produces a non-null page and cannot be attached to Ferric's Qt Quick profile
  through a documented public bridge. No ordinary viewless request-to-ledger
  evidence exists.
- Production archive-capture and offline-view capabilities remain false.
  Native success on reachable routes does not satisfy their full acceptance
  matrices or VB-057 release qualification.
- WebRTC absence remains unqualified for current-window live retention.
- Media-source matching stores a deterministic unkeyed URL digest. The
  disclosure is implemented, but product/security acceptance or a reviewed
  key lifecycle has not been decided.
- Pack documentation/ledger integrity now passes `python3 validate_pack.py
  --summary`: 96 tasks, 9 phases, 275 dependency edges, valid DAG and
  appendices, balanced fences, and zero enabled production capabilities. The
  ledger reports 46 verified, 45 blocked, and 5 not started; no slices remain
  marked in progress.
  `python3 validate_pack.py --next` returns null because no not-started task
  has verified dependencies and satisfied capabilities. `sha256sum -c
  SHA256SUMS.txt` and `git diff --check` pass. This is pack integrity, not a
  Ferric build or final scope/release approval.

See the slice-specific records [VB-024](VB-024.md), [VB-044](VB-044.md),
[VB-045](VB-045.md), [VB-050](VB-050.md), [VB-055](VB-055.md), and
[VB-093](VB-093.md), plus the active execution ledger and
[`AGENT_HANDOFF.md`](../../../CurrentImplementation/ferric-versioned-browsing/AGENT_HANDOFF.md).
