# YouTube playback alias and tab receiver ownership — 16 November 2026

Two separate causes were reproduced on the installed build:

1. The saved watch policy explicitly admitted Normal-mode `p` through a Mixed
   site setting and included `normal-page-key` in the `p` → `k` remap. Its continued
   Normal-mode behavior was configured, rather than a retained Hint gesture.
2. The window registry retained the first page view. Receiver probing and receipt
   acceptance used that registration snapshot, so another selected tab could not
   obtain its native receiver proof even when its document was focused and
   classified noneditable.

## Changes

`49aae13` resolves the host's selected page when determining the diagnostics /
receiver owner. It preserves registry metadata, enforces host/lifetime checks,
and rejects receipts from an old selected view or epoch. The registry's captured
view is left intact; it is no longer treated as current receiver authority.

The starter watch preset now maps `p` only in Hint and explicit passthrough
contexts. The existing installed watch rule was updated through the policy draft
service, its two legacy Normal admission settings removed, and the exact saved
configuration activated through the service. Other rules and global settings
were compared against the pre-change snapshot. Normal `p` again belongs to the
browser's `pp` / `pP` prefixes. No public API or configuration schema changed.

## Validation

- The new ownership suite fails four checks against the old resolver. All seven
  checks pass with the fix: selected-view resolution, second-tab probing, old-tab
  and old-epoch receipt refusal, owner/lifetime guards, and primary fallback.
- All 23 selected native application checks pass, including resource parity,
  held/reentered Hint gestures, blocked receivers, same/foreign frames,
  two-window ownership, Normal routing and explicit passthrough.
- All six existing page-focus handoff checks pass.
- A separate normal browser profile, two local HTTP pages, and a real looped WebM
  exercise the qualified Wayland input path. Its policy uses the same Hint and
  passthrough admission/remap settings as the corrected watch preset; only the
  URL pattern points to the owned fixture server. The user's open tabs are not
  used as media test fixtures.
- Ten native `p` gestures each produce one trusted `k` keydown/keyup pair and the
  expected real HTML video paused-state change. Hint gestures return to Normal;
  Normal `p` sends no page event. Switching A → B → A never changes the inactive
  tab's event stream. Both `;v` and `;V` behave as specified.

Normal `p` is tested as a browser prefix, not as a no-op; independent subsequent
commands wait for that test's chord to expire. Passthrough diagnostics use
`interaction.status`, since `site-policy-explain` is unavailable in those modes.
Early fixture setup failures are retained separately from passing product checks.

## Evidence and installation

Ignored `target/youtube-p-routing/` contains `two-tab-trace.json`,
`owner-before.log`, `owner-after.log`, `build/`, `profile-fix.json` and
`live-tabs.json`. The saved-config backup is private and excluded from Git.
The disposable fixture process and server are stopped after validation.

Qualified release SHA-256: `da92a37d12bf3b9d6a6380dfc42ec2c678da8e4d991911f33c57ce4320dc9436`.
The exact source inventory and validation-tree parity are retained in the build
record. Binary installation / rollback and whether the running browser still
requires a restart are recorded in `final-verification.json` in that directory.
The live profile correction applies immediately; the receiver code requires
restarting an existing browser process. No fix to the separate busy-page Hint
collection invalidation issue is claimed here.
