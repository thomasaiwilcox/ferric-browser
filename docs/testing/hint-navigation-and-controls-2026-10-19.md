# Hint navigation lifecycle and inherited CSS controls — 2026-10-19

## Reproductions

The installed build was `38a58e63c5e4130cab8c52f3f6fb6d8ffc691a147b205552fea8bcd6d171c577`.
Artifacts are retained under ignored `target/hint-followup/`.

Document replacement while hints are active clears the Rust session and stops
page callbacks, but leaves the core/UI mode as Hint. An activation whose
callback is retired by navigation can therefore leave the browser stuck. The
native replacement baseline reproduces the retained Hint mode. A hinted
button which navigates and continues its handler for 200 ms also reproduces
the failure: the destination loads but mode remains Hint. A simple
location-setting click did complete normally in the baseline, so this is not
claimed to happen on every navigation. A separate baseline showed that a
background tab loading clears the active document's retained hints.

On the [reported Mumsnet thread](https://www.mumsnet.com/talk/_chat/5128792-help-what-to-do-about-a-mouse-in-our-house),
Quote and Add post are native buttons. React is a div declaring `type="button"`
and `tabindex="-1"`. Their ancestor has pointer-events:none, while the controls
explicitly restore pointer-events:auto. Actual WebEngine hit testing reaches
the controls, but Ferric's ancestor check rejects them. React also fails the
built-in selector set. The live probe uses an isolated anonymous profile and
closes the consent prompt using its X button; it does not accept cookies or
post any content. Collection/freshness checks are run after scrolling the
first post's action row into view. With the old scripts it collected five
viewport candidates and none of the three reported controls.

The [CSS UI specification](https://www.w3.org/TR/css-ui-4/#pointer-events-control)
permits descendants to restore pointer targeting. The analogous inherited
visibility rule is covered by the [CSS Display specification](https://www.w3.org/TR/css-display-3/#visibility).

## Changes

- Active-document navigation cancels the hint session and exits Hint mode;
  background navigation does not clear active hints.
- Leaving Hint mode invalidates the collection epoch and clears pending state
  so a retired callback cannot block the next hint session.
- Shared style checks use the receiver's computed visibility/pointer-events.
  Ancestor display:none and zero opacity still exclude descendants; rendered
  state, hit testing, clipping, disabled/inert state and frame validation remain
  required. Only ancestor facts that cannot be overridden are cached together.
- Custom controls with both `type="button"` and a tabindex are discovered and
  classified as button actions. This bounded pattern covers programmatic
  receivers without adding every negative-tabindex element or pointer cursor.
  Existing selector replacement policies and fresh validation still apply.
- BrowserScripts 25 and PageInteractionScripts 7 have paired version guards.

## Validation

Before the script fix, the expanded HintActions suite had 23 passes and 17
failures: sixteen CSS override cases and the custom receiver case. Afterward,
all 40 checks passed, including ordinary DOM, shadow DOM, slots, same-origin
frames and the native-visibility fallback. Changes to the receiver's CSS or
button annotation invalidate retained candidates.

The updated live Mumsnet probe passed its three QtTest checks. It collected 22
viewport candidates, including Quote, React and Add post, and verified fresh
visibility for each. Counts can vary with advertisements and live content.

The native fixture covers primary/secondary windows, button and form
navigation (including the busy-handler race), same-document history, document replacement, background loads,
and fresh hint collection after navigation. It also exercises native keyboard
activation of Quote, React, revealed Thanks, and Add post at three zoom levels
and doubled display scale, with one click per selected receiver.

The full repository gate passed: 1,118 Rust tests passed, none failed and two
were ignored; all 704 QML checks passed. Formatting, documentation, architecture,
native components, release-manifest and license checks passed. The optimized
release build passed.

All twelve native lifecycle rows passed, including the busy-handler navigation
race, replacement and background loads in both window types. The existing
history/usability gate and doubled-scale cross-origin consent gate passed.

The first native control run rejected the fixture's exact React name in every
zoom/window row: its opacity-hidden caption contributed to the parent name,
producing “React Thanks.” This was a fixture mismatch, independently confirmed
with the collector. The fixture now models icon buttons with an aria-label and
a hidden caption, matching the live site's text structure; collection returns
React while hidden Thanks remains excluded. A focused native activation row
then passed. No production source changed after the repository/release checks.

The corrected native hint gate passed all 25 QtTest checks, and its doubled-scale
control gate passed all six. The earlier unchanged history/usability and
cross-origin consent gates contributed 14 and 17 checks respectively. A separate
frame invalidation/operation-lifetime run passed 13 checks. Together these are
75 successful native checks, with no remaining failures or skips.

After the fixture correction, focused QML reruns passed all 40 HintActions and
31 HintControls checks. The original failed native logs are retained as
`initial-native.log` and `initial-native-details.log`; final hint gate results
are in `native-details.log` and frame invalidation results in `frames.log`.
`qualification.json` records the successful rerun and identifies the two
unchanged gates already passed. The final `git diff --check` passed.

## Local installation

The validated release was installed to `/home/tom/.local/bin/ferric-browser`.
Its SHA-256 is `e146d32a714aab24057b360b215f9f785a3ce77ac18951c0d302b15dae4c969f`.
The previous installed build and associated installation files were saved and
checksum-verified under `/home/tom/Backups/ferric-browser/20261019-162131`
before atomic replacement. Post-installation version, help, isolated diagnostics,
dynamic libraries, desktop-cache and icon-cache checks all passed.

Profiles and settings were preserved. Existing browser processes were left
running; quit all Ferric windows and reopen to use the new build.

To roll back, close Ferric and run:

```sh
python3 /home/tom/Backups/ferric-browser/20261019-162131/restore.py
```

`release-smoke.json` contains the new and previous executable hashes;
`installation.json` records the backup and successful installation checks.

## Limits

This verifies the anonymous live markup and local interaction contracts. It
does not post, react or quote using an authenticated Mumsnet account. Actual
site actions may require login. Hidden reaction choices remain excluded until
their menu is exposed.
