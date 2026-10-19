# Page search and native activation — 2026-10-19

## Status

The renderer-based search implementation and its subsequent native-fallback
hotfix failed user testing on YouTube's recommendation homepage: visible text
could take about ten seconds to appear, and Enter did not activate the result.
Their earlier automated passes did not establish acceptable live-site behavior.
They are superseded by the native implementation described below.

The user explicitly selected **Use native search and activation** after being
informed that this changes the earlier verified-URL-only design to normal
browser keyboard behavior, including wrapped links and many buttons.

## Corrected behavior

- `/` and `?` dispatch directly to Qt's native find. Chromium owns matching,
  highlighting, scrolling, and frame traversal. There is no per-query JavaScript
  search, custom highlight, renderer initialization, or frame-lease discovery.
- Search-bar edits explicitly wake runtime dispatch rather than waiting for
  its idle maintenance timer. New query edits immediately supersede earlier native finds. Explicit `n`/`N`
  steps remain ordered. Late callbacks cannot replace a newer result or revive
  a canceled session.
- The first Enter keeps the selected result and exits the search bar without
  advancing. A second Enter activates the native selected result. Shift+Enter
  sends the native Ctrl+Shift+Enter foreground-tab gesture for links. Its
  one-shot, source-scoped native new-tab request supplies the destination to
  Ferric's existing typed foreground-tab action rather than opening an ordinary
  popup window. This scope expires after one second and is consumed once.
- Native find is ended with Qt's selection-preserving empty query before
  activation. An isolated-world renderer acknowledgement prevents Enter from
  racing this asynchronous selection handoff. The acknowledgement runs only on
  activation, has a 750 ms deadline, and is canceled on query, mode, document,
  tab, or view changes. Native prompts retain priority.
- Activation uses the selected page's native keyboard behavior. Ferric does not
  infer a URL from duplicated text, match ordinals, DOM ancestry, or coordinates.
  Page handlers and control behavior can therefore run, as with ordinary native
  keyboard activation. There is no separately verified destination preview.
- Captured tab/generation/document checks and pending-find checks protect the
  source. The native key adapter also requires the requesting view's own focused
  window. Configured bindings, Mixed routing, prompt ownership, and held-key
  suppression remain ahead of activation.
- Both window types share the controller. `target-search links` remains the
  separate list interface. Escape clears retained search and queued finds.

The old `PageSearchScripts` renderer resolver, custom highlights, search-specific
frame broker, and URL-follow bridge have been removed. This also removes the
blanket exclusion of ordinary heading/span wrappers. No Qt patch, extension,
remote-debugging port, IPC protocol change, or profile migration is involved.

## Why the previous design failed

The controller initialized the entire frame tree sequentially before searching.
Individual renderer operations could each wait for a one-second deadline, and
native fallback could then wait for all renderer cleanup callbacks. This put
unrelated frames and initialization ahead of ordinary text matching. The exact
reported ten-second run was not reproduced in the personalized YouTube profile,
but the controller had no session-wide latency bound.

The URL resolver deliberately declined possible closed-shadow hosts, which
included ordinary spans, headings, divs, and custom elements. That conservative
public-DOM approach excluded common recommendation titles. Falling back to
native matching restored some highlighting but deliberately granted no link
activation, so Enter still could not deliver the requested behavior.

A native prototype also exposed a second race: an immediate key following
`findText("")` could arrive before Chromium completed the selection handoff.
The renderer acknowledgement addresses that race; repeated activation fixtures
include no artificial settling delays between search completion and activation.

## Validation

Environment: Linux x86_64, Qt 6.11.2, offscreen QPA, software Qt Quick rendering.
Artifacts remain ignored under `target/find-link-plan/`.

- Focused real-WebEngine tests: **27 passed, zero failed** in
  `qml-native-search-qualified.log`. Coverage includes wrapped titles, CSS disabling
  selection, blocked page styles, disabled page scripts, open/closed shadow roots,
  native button clicks, new-tab requests, native match ordering, search-bar focus,
  immediate supersession of stalled callbacks, queued steps, canceled callbacks,
  activation canceled during its handoff, and background/captured-target behavior.
- One focused case performs **20 successive native activations**, alternating
  selectable and unselectable wrapped titles, without settling delays.
- The initial native probe also confirmed real navigation requests to a link
  inside an opaque closed shadow root (`native-activation-probe.log`).

- Full QML suite: **731 passed, zero failed** (`qml-native-search-full.log`).
- QML lint: 100 files checked, zero critical diagnostics, exit zero
  (`native-search-qmllint.json`).
- Locked offline release build passed in 9m 26s (`release-native-search-final.log`).
- Production application search gate: **18 passed, zero failed**
  (`application-native-search.log`, `application-native-search-details.log`).
  These cases exercise physical keyboard routing in both window types, query
  acceptance/held repeats, wrapped titles with/without selection disabled,
  current/foreground tabs, fragments, custom bindings, canceled queued requests,
  and native frame targets. `_self` stays within its frame and `_top` navigates
  the main page, as ordinary native activation does.
- Rust workspace excluding the Qt engine and executable: **513 passed, zero
  failed** (`native-search-rust-tests.log`). Qt Rust invokables were compiled in
  the release and their activation guards exercised by application tests.
- A 2,000-link/20-frame probe measured 51 ms initially and a 3 ms median for
  subsequent controller steps, compared with 43 ms / 3 ms for direct Qt find
  (`native-search-latency.log`). This small interaction probe overlapped the
  tail of the QML run; it is indicative, not a general browser performance claim.
- A disposable production-browser YouTube check searched `Subscriptions` and
  opened `/feed/subscriptions` using Enter (`native-live-production.log`). It
  used the built release engine and real application QML, not a DOM URL guess.

Standalone Qt Quick Test YouTube probes showed inconsistent native focus/selection
handoff, despite finding visible labels in about 55–70 ms. Directly focusing a
known test link proved that keyboard navigation itself worked, but that is not
the implementation or qualification for search activation. The decisive live
check uses the production application and actual search key routing. The
standalone results remain available as diagnostics; they should not be presented
as live activation passes. Consent was dismissed only in a disposable profile.
The logged-out homepage had no recommendation cards, so none of these checks
qualifies the user's personalized recommendations.

The full native keyboard/input suite passed **94 cases, zero failures**
(`native-insert-search-qualified.log`). Its initial run exposed two fixture
failures: simulated Normal-mode V bindings omitted the real application's
clipboard ShortcutOverride ownership callback. The fixture now models that
callback; no existing production key-filter branch was changed for those tests.

Measuring the *complete* physical query, rather than an earlier matching prefix,
exposed a further roughly one-second primary-window delay. `search_changed` queued
native work but did not emit the runtime wake-up used by normal-mode keyboard
commands; repeated edits also leave the status string unchanged. It now emits
`runtime_work_available` for each edit. Application tests assert the wake-up and
the complete delivered query after every character. Final qualification of that wake-up fix:

- Locked offline release rebuild passed in 8m 21s
  (`release-native-search-wakeup.log`).
- The final application search gate passed **18 cases**, including per-character
  wake-up/delivered-query assertions; navigation/history passed **14 cases**
  (`native-search-final-gates.log`, `native-search-final-gates-details.log`).
- Embedded-resource parity passed against **112 source files**
  (`native-search-final-resources.log`).
- A complete physically typed `/subscriptions` query on the production YouTube
  homepage was ready in **125 ms**, down from **1,104 ms** before the explicit
  wake-up. The first Enter retained it; the second navigated to
  `/feed/subscriptions` (`native-live-wakeup-qualified.log`). This is one local
  interaction measurement, not a cross-site latency guarantee.
- A preceding production check also verified the physical `/` workflow with
  Shift+Enter, two tabs in the requesting window and no extra browser window
  (`native-live-physical-youtube.log`). The activation/tab-routing implementation
  is unchanged by the final wake-up; final offline application cases exercise it
  again against the rebuilt engine.

Live probes used a temporary test-only branch, archived as
`target/find-link-plan/native-live-physical-probe.cpp` with its object file.
The final probe relinks that fixture object with the final release engine through
`link_native_live_probe.py`. The branch is removed from repository test sources;
normal/CI tests do not access YouTube or depend on its consent UI.

Installed atomically at `/home/tom/.local/bin/ferric-browser`:

- SHA-256: `6706f4c912083b78a674d05e8bf966bc49c3290f1650a9c6abea46af10dacc8b`.
- Previous installed binary:
  `/home/tom/.local/state/ferric-browser-install-backups/20261019-230740-before-native-search/ferric-browser`.
- Backup SHA-256: `57f92753b51146efe1216513d70d72cdc10f3c193045acc3d475390089280bbf`.
- Installed help, version and disposable diagnostics commands passed
  (`installed-native-search-help.log`, `installed-native-search-diagnostics.json`).
- Existing running instances were not terminated, and user profiles/configuration
  were not changed. A complete quit/reopen is needed to load the new executable.
- Generated probes, build products and logs remain ignored under `target/`.


## Earlier evidence (superseded implementation)

The initial renderer implementation passed 1,119 Rust cases (two ignored),
749 full QML cases, and targeted application/navigation/hints/frame gates.
The fallback hotfix passed 757 full QML cases, 13 page-search application cases,
14 navigation cases, and embedded-resource parity. These historical passes are
recorded in the earlier `check-complete.log`, `qml-qualified.log`,
`qml-search-fallback-final.log`, and `native-search-fallback-final.log` artifacts.
They do not qualify the replacement native implementation or negate the user's
observed regressions.
