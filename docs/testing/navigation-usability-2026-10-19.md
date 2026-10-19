# Back/Forward and single-tab URL copying — 2026-10-19

## Findings and changes

The keyboard binding path dispatched reducer effects but converted captured
navigation, history, reload and stop effects back into legacy action strings.
The refactored QML dispatcher consumes typed requests for those operations, so
an H binding could report “Binding executed” without navigating. Bindings now
queue the same captured requests as command-bar dispatch. Only non-targeted
presentation effects retain their legacy presentation path. Counts and target
identity remain attached to the engine effect.

SPA reconciliation imported valid logical history without emitting
`content_history_revision`, leaving Back/Forward availability stale. Accepted
imports now publish the revision. A same-URL push from the first history entry
also lost its original position because that entry has no earlier logical
baseline; reconciliation now retains both positions.

Normal-mode mouse Back/Forward gestures are handled once through the owning
window's browser command path rather than independently by WebEngine. Other
buttons and other input modes keep their existing routing.

The primary and secondary status bars now expose a labelled Copy URL button,
including when there is one tab and the tab strip is hidden. The button respects
`ui.statusbar`: the default `in-mode` policy hides the entire status bar in Normal
mode; `set ui.statusbar always` keeps it visible. Normal-mode yy works with
hidden chrome. The button delegates to the existing Rust-owned `browser.url.copy` action in its own window, retaining
safe URL handling, clipboard failure reporting and ownership. Normal-mode yy and
`:yank url` remain available. Bare `hint` bindings also now use the same action-first
default as the command bar instead of taking the old links-only shortcut.

## Reproduction and qualification

Artifacts are retained under ignored `target/navigation-usability/`.

- Existing command-bar/native-page history tests passed six QtTest checks
  (`history-baseline.log`), demonstrating their coverage gap.
- Added production journeys failed all six rows against the installed engine
  (`journeys-before.log`): ordinary opens, link clicks, hints, SPA pushState,
  repeated-URL state and mouse Back. Keyboard H left the current URL unchanged;
  SPA imports left availability stale; mouse Back could skip an entry.
- Focused updated source QML checks passed: 27 native-history checks and 46
  chrome checks, including preservation of the first repeated-URL state and
  visible, accessible URL-copy controls in both window shells.
- The production integration gate covers H, counted L, command Back, reload,
  normal-mode mouse Back, custom bare-hint bindings and single-tab copying in
  primary and secondary windows. It is registered in CI alongside the hint gates.

- The full repository gate passed: 1,118 Rust checks passed, none failed and
  two were ignored; all 687 QML checks passed. Documentation consistency,
  formatting, architecture, native component, release-manifest and license
  checks passed as part of the gate.

The optimized release build passed. The final workspace Rust run after removal
of the obsolete hint shortcut produced the same totals.

The first native gate passed all history journeys but correctly rejected the
URL-copy fixture's assumption that the status bar is visible in Normal mode. The
fixture now checks that `in-mode` hides the button, then temporarily enables
`always` and clicks the button while the single-tab strip remains hidden. This
changes no production visibility policy. The corrected navigation gate passed.

All three final native gates passed: 14 navigation/usability checks, 13 hint
control checks and six hint checks at doubled display scale, with no failures
or skips. `native-details.log` preserves the individual QtTest results. The
final `git diff --check` passed.

## Local installation

The qualified binary was installed to `/home/tom/.local/bin/ferric-browser`.
Its SHA-256 is `38a58e63c5e4130cab8c52f3f6fb6d8ffc691a147b205552fea8bcd6d171c577`.
The previous installed build (`afbed7c8cf8ebe8b83e070d5a9862ad97fe3b7657087175f99c0f7867516c70a`)
and associated installed files were saved and checksum-verified under
`/home/tom/Backups/ferric-browser/20261019-153803` before atomic replacement.

Post-installation checks passed for version, help, isolated diagnostics, dynamic
libraries, desktop cache and icon cache. Browser profiles and settings were
preserved. Existing browser processes were left running; quit all Ferric
windows and reopen to use the new build.

To restore the previous installation, close Ferric and run:

```sh
python3 /home/tom/Backups/ferric-browser/20261019-153803/restore.py
```

The installation manifest and checks are recorded in `installation.json`;
release checks and the two executable hashes are in `release-smoke.json`.

## Limits

The navigation issues above are reproduced independently of any live website.
The user's report that hints are better on some targets and worse on others
still needs a concrete page/control example; this change does not guess at a
new universal hint policy. Mouse side-button normalization is intentionally
limited to Normal mode, preserving page input and pass-through behaviour in
other modes.
