# Guide article first-scroll layout

Status: automated qualification passed, 2026-10-23.

## Reproduction

The native Guide journey can lose the first Page Down after opening or
reopening an article. The observed Wayland run fails in secondary and popup
windows while native focus remains on `ferricGuideArticle`, with Normal mode,
reader mode active, and no editor taking input.

Immediately before Page Down, the viewport is 290 pixels tall but reports
218 pixels of content. After layout, the same article reports 1,148 pixels of
content. The first key clamps its movement to zero using the earlier height.
The failure facts are in
`target/tab-keyboard-focus-guide-observed-wayland.log` (two failures, four
QtTest passes including setup/cleanup). The earlier combined recovery/Guide
run also records two failures, alongside four passing recovery cases and
two passing Guide cases, in `target/tab-keyboard-focus-popup-wayland.log`.

The final QML regression settles the collapsed reader's layout, then opens the
article and requests movement in the same event turn. It never inserts a render
or layout barrier between those actions. The six cases cover first opening and
reopening with Page Down, Space and `j`, followed by the corresponding upward
key. The original Guide, verified against the completed tab-focus source hash,
fails five cases: `target/guide-first-scroll-baseline-final-qml.log`.

## Change

Before reading scroll bounds, Guide finishes pending layout for its columns,
scroll viewport and article using Qt's public `ensurePolished()` API. Page
steps use the resulting viewport height. The action runs immediately with the
current layout; it does not queue a later scroll that could belong to another
feature or search.

All 58 focused chrome QML checks pass, including the six new cases and their
reverse movements: `target/guide-first-scroll-chrome-qml-final.log`.
The native fixture retains failure-only focus/geometry facts and now verifies
the compiled Guide and focus-controller resources against source.

## Qualification

Production is frozen in `target/guide-first-scroll-source-hashes.json` (44
files). Only `FerricGuide.qml` differs from the completed tab-focus snapshot.
The full QML suite passes 1,164 checks in 211.05 s with the required forced-GC
flags: `target/guide-first-scroll-full-qml.log`.
The all-target release build passes in 13 min 48 s. The compiled Guide journey
passes in primary, secondary, private and popup windows offscreen (six QtTest
passes including setup/cleanup, 38.63 s) and with compositor seat keys (six
passes, 30.87 s). The original secondary and popup failures now pass with the
same immediate Page Down and unchanged scrolling assertion. Logs are
`target/guide-first-scroll-native-guide.log` and
`target/guide-first-scroll-application_keyboard_guide_wayland-qt.log`.

All 502 release engine unit tests pass, with one opt-in benchmark ignored,
in `target/guide-first-scroll-release-engine-tests.log`.
`target/guide-first-scroll-qualification-status.json` records complete success;
the runner verifies all 44 production hashes and successful completion of the
full release build. The tab-focus qualification covers the unchanged focus
implementation; this follow-up changes only the Guide's layout and scrolling.
