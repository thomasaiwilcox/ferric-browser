# Page search feedback and editor ownership

Status: automated and rendered qualification passed, 2026-10-23.

## Reproduction

The existing native search gate passes 18 QtTest checks in 38.08 s, but it does
not check composition ownership or visible match counts. Its baseline is
`target/page-search-usability-baseline-native-qt.log`.

New QML tests reproduce three premature actions during explicit preedit:
Return and keypad Enter accept search, and Escape requests closure. They use
actual `QInputMethodEvent` preedit, not an assigned composing flag:
`target/page-search-editor-baseline-final-qml.log` (three failures).
Two feedback cases fail because the text has no native position/total and
because the search bar covers the enabled status bar while the feedback
fallback remains hidden: `target/page-search-feedback-baseline-qml.log`.

The clean native offscreen baseline fails all eight preedit cases across
primary, secondary, private and popup windows, plus the missing-count case:
`target/page-search-editor-baseline-qualified-native.log` (nine failures,
two setup/cleanup passes). The Wayland baseline reproduces the same eight
composition failures, while all ten result-activation cases pass through the
owned keyboard seat: `target/page-search-editor-baseline-qualified-wayland.log`
(12 passes including setup/cleanup, eight failures).

The clean popup fixture opens its real link through a configured hint binding
and waits for its actual `/popup.html` document. The shared keyboard observer
filters zero-scan-code internal Enter deliveries used by native search
activation. It still requires the seat's one native override/release pair,
non-repeated input, matching scan codes and modifiers, and cleared modifiers
afterward. The initial exploratory logs include those fixture/observer problems;
the qualified baseline logs above contain the production failures.

## Change

The search editor keeps composing keys with Qt and suppresses acceptance during
preedit. After a commit, the next fresh Return or Escape performs its ordinary
search action.

Feedback reads the native active match and count, for example `Match 2 of 3`.
It shows a count alone if an active position is unavailable, and clears saved
native position state when its session ends. Pending or superseded queries
retain Searching feedback rather than showing a previous result.

The feedback panel remains above the active search bar. After acceptance,
the existing status bar carries the same result, with the panel as the fallback
when that status bar is hidden. The panel uses its window's font, wraps to at
most two lines, and preserves the beginning of the message when constrained.
Neither search chrome nor feedback changes the webpage viewport.

## Verification

All 31 focused PageSearch QML checks and five SearchEditor checks pass. They
cover native order and wrapping, no matches, stale queries, explicit preedit,
fresh action recovery, status visibility and a 320-pixel panel at 26-point text.
The complete QML suite passes 1,173 checks in 211.05 s with the required
forced-GC flags: `target/page-search-usability-full-qml.log`.

The native matrix checks three status policies, match stepping/wrapping,
retention and page focus after Enter, clearing, no matches and stable viewport
dimensions in all four window types. It also checks a 360-pixel window with
26-point configuration and the effective window font, and optionally captures
that rendered feedback. Preedit cases require no early browser action and a
working fresh action after commit. Source/resource checks cover the search,
focus and window components.

The first compiled revision is frozen in
`target/page-search-usability-source-hashes.json` (45 files), changing only
SearchBar and PageSearch from the completed Guide revision. Its all-target
release build passes in 13 min 36 s. The 12-case native canary passes 14 QtTest
checks in 44.45 s and the Wayland search gate passes 24 checks in 62.58 s.
The existing offscreen gate passes 18 checks in 39.52 s; its explicit selector
has since been expanded to include the 12 new cases. The shared-helper Guide
gate passes six checks in 13.71 s. All 502 release engine units pass, with one
opt-in benchmark ignored. The first qualification status records complete
success in `target/page-search-usability-qualification-status.json`.

## Visual follow-up

The first native images at `target/page-search-usability-native-images` show
readable match feedback, but expose undersized arrow symbols: their Text items
keep the 9-point application default while the editor uses 26 points. Two QML
cases reproduce that mismatch at 12 and 26 points in
`target/page-search-symbols-baseline-final-qml.log`. The symbols now use their
own window's font, with native assertions for both buttons. The cue is shortened
to `Enter: keep` so the large-text view shows a complete primary action.

The final 45-file revision is frozen in
`target/page-search-polish-source-hashes.json`; only the two search components
differ from the first qualified revision. All seven SearchEditor and 31
PageSearch QML checks pass. The final full QML suite passes 1,175 checks in
214.45 s: `target/page-search-polish-full-qml.log`.

The final all-target release build passes in 13 min 50 s. The compiled canary
passes all 12 new cases (14 QtTest passes including setup/cleanup) in 53.78 s.
The expanded offscreen search gate passes 30 checks in 83.70 s; the Wayland
gate passes 24 in 61.72 s. Detailed logs are
`target/page-search-polish-native-canary.log`,
`target/page-search-polish-application_page_search-qt.log` and
`target/page-search-polish-application_page_search_wayland-qt.log`.
All 502 release engine unit tests pass, with one opt-in benchmark ignored.

All four final offscreen images in `target/page-search-polish-native-images`
were inspected. The 360-pixel views show the complete match counter and
`Enter: keep` cue, with arrow symbols scaled to the same font as the editor.
The native checks also verify the configured size, effective font, bounded
feedback and page focus. `target/page-search-polish-qualification-status.json`
records complete success after checking all 45 final production hashes and
successful completion of the full release build.

These are explicit preedit-event checks. Platform IMEs, physical keyboard
layouts and assistive-technology interaction remain separate qualification.
