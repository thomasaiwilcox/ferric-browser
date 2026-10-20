# Action-first hints and rendering validation — 2026-10-19

## Behaviour

Unqualified `hint` and built-in `f`/`F` variants now use `hint actions`.
This view omits generic focus-only wrappers while preserving ordinary controls,
editable fields, links, disclosures, media, scrolling targets and frame actions.
`hint all` retains comprehensive visible targets and existing custom bindings
remain unchanged. `hint focus` explicitly chooses a keyboard region and enters
Insert only after verified focus; it supports first/index selection but rejects
rapid mode, scripts and non-current targets.

The new views inherit `hints.selectors.all`, including replacement policies.
Rust validates the family contract; the page cannot choose execution authority.
Editor descriptions say Edit, while generic keyboard regions say Focus.

Shared visibility checks use the target's native `checkVisibility()` result,
including skipped content visibility, alongside existing geometry, style,
clipping and action-specific checks. Layoutless slots and `display: contents`
ancestors do not incorrectly hide visible descendants. A bounded fallback checks
closed disclosures and hidden content through composed ancestry when the native
method is absent. Results are cached only during one collection pass and are
rechecked before actions. Native disclosure receiver preference remains
identity-based; distinct overlapping controls and nested links remain distinct.

## Qualification

Focused source QML checks passed: 23 action/visibility cases and 31 native-control
cases, with zero failures. The rendering subset against the captured baseline
failed all eight regression rows (document, shadow, slot and frame, each with
native and fallback variants); the corrected implementation passes them.

Serial production collection, timed by an outer wrapper, used ten warmups and 100 samples
per row, comparing previous default `all` with new default `actions` on identical
synthetic button pages (`benchmark.log`, `benchmark-summary.json`):

| Targets | Collector before → after | Change | Round trip before → after | Change |
| --- | --- | --- | --- | --- |
| 1,000 | 91.40 → 95.90 ms | +4.9% | 152.33 → 152.43 ms | +0.1% |
| 5,000 | 837.35 → 861.45 ms | +2.9% | 1,015.70 → 1,044.59 ms | +2.8% |

Both comparisons meet the planned 10% threshold. These measurements concern
hint collection on this machine, not overall page or browser speed. Native
visibility checking adds bounded work; scripts now also filter by action kind.
The full repository gate passed (`check.log`, 486.83s): 1,118 Rust tests passed,
zero failed, two ignored; 685 QML checks passed with zero failures or skips.
Lint, generated documentation, architecture, profile/download native checks,
release-input validation, licensing and formatting also passed. An earlier run
found one stale source-contract assertion for the extended linked-image branch;
it was updated and the complete gate rerun successfully. Its log is retained as
`check-stale-contract.log`.

The isolated live Reddit probe passed three checks (`live-reddit.log`, 9.26s).
On that public thread, comprehensive hints had 50 targets and action-first hints
47: three visible focus-only wrappers were excluded, no hidden targets were
admitted, and exact disclosure activation opened the thread. The probe used a
separate off-the-record Qt profile without affecting the running browser.

The locked, offline release build passed in 529.54s (`release.log`). The native
application executable was rebuilt against that engine (`native-build.log`). All
four selected CTest gates passed in 142.57s (`native.log`, `native-details.log`),
with 41 QtTest checks passed, zero failed and zero skipped:

- `application_hint_controls`: default `f`, explicit all/focus, stale hidden
  targets, text editing and native controls across 80%, 100% and 125% zoom plus
  a secondary window; embedded resource parity, refresh/confirmation checks,
  and scoped Hint/Search journeys.
- `application_hint_controls_dpr2`: the same native-control matrix at DPR 2.
- `application_offscreen_dpr2`: native coordinates within a scoped grid.
- `application_frame_dpr2`: cross-origin consent-frame hint journeys.

Release CLI version/help smoke checks passed (`release-smoke.json`). The prepared
`target/release/ferric-browser` SHA-256 is
`afbed7c8cf8ebe8b83e070d5a9862ad97fe3b7657087175f99c0f7867516c70a`.
Before installation, the installed binary was checked separately as
`deca1ca925c97bd0275ddc4d1308c587ecc997a08b05fc0b7ad9e304632b97c5`.
Formatting and `git diff --check` passed. No Clippy run was added for this change.
Artifacts live in the ignored `target/hint-actions-fix/` directory.
The baseline scripts were captured from the previously validated native-control
fix (facade version 23, page interaction version 5), before this change.
The resulting resources use facade version 24 and interaction version 6.

## Limits

Live Reddit evidence supplements deterministic fixtures; site content, account
state and DOM variants can change. No site-specific selectors or click guessing
were added. Controls without usable action semantics may require `hint all`,
`hint focus` or Grid. Explicit focus keeps the existing Insert behaviour so custom widgets can receive keyboard input.

## Local testing installation

At the user's request, the validated release was installed to
`/home/tom/.local/bin/ferric-browser` on 2026-10-19. Its checksum matches the
qualified release above. Version, help, isolated diagnostics, shared libraries
and desktop/icon cache checks passed (`installation.json`). The shell resolves
`ferric-browser` to this per-user executable. User documentation was refreshed;
profiles and settings were not changed.

The previous installation was checksum-verified and saved to
`/home/tom/Backups/ferric-browser/20261019-145142`, including a manifest and atomic
restorer. To roll back, close Ferric and run:

```sh
python3 /home/tom/Backups/ferric-browser/20261019-145142/restore.py
```

Quit any running Ferric instance and reopen it to use the new build. No commit
or repository cleanup was performed.
