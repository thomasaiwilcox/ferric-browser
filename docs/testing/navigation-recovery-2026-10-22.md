# Page-load recovery, form reload and keyboard Guide — 2026-10-22

## Current verification

The final v6 release build passes against the 40 frozen production files in
`target/journey-identity-v6-source-hashes.json`. The full QML suite passes
**1,116 checks**; the release engine suite passes **500 tests**, with one opt-in
benchmark ignored. Core **162**, runtime **8**, application **122 unit plus
5 integration**, and storage **105 unit** tests also pass for this source.

All nine navigation, history, form, recovery and page-search CTest gates have
passing v6 runs. The SQLite gate verifies the exact durable visit, including
selection while older writes are queued. The offscreen history gate initially
failed one of 64 cases with a stale `cancelled` load state; a later clean run
passes all 64. This intermittent observation remains unresolved and is not
counted as a source fix. The [final compiled results](#v6-compiled-verification)
record the failure, subsequent diagnostic runs and their limits.

The sections below preserve earlier baselines and qualification stages;
their older counts do not describe the final source.

## Behavior

The load-failure dialog previously offered only Dismiss. Secondary, private
and popup windows did not instantiate the dialog even though their backend
reported navigation failures. Opening the same URL could also stop at the
native content wrapper without issuing another engine request.

The dialog now offers `r` to open the address again, `e` to edit the address,
and `b` to go back when history is available. The initial keyboard selection
is Close. Escape closes the dialog. These controls use the existing typed
action boundary in each window shell.

Opening again issues a fresh URL navigation rather than Reload. This preserves
the actual address, including fragments and parameters hidden from the safe
display, and avoids resending a failed form submission. Explicit same-address
Open commands now reach the engine as well; model synchronization remains
idempotent. A completed document keeps Chromium's same-document fragment
behavior. Ferric explicitly settles these visits, including reopening an
unchanged fragment, because Chromium can omit loading and URL notifications.
Changed fragments wait for Chromium's new history entry before being recorded,
so Back can return to the previous anchor. Proven same-document SPA route
changes also update the document address used for this decision.
The document's form-submission identity is preserved across these visits.
Failed documents still take the fresh GET path.

Editing quotes the address for Ferric's command grammar, including literal
command separators. Focus moves to the command field with the caret inside
the closing quote. The closing dialog cannot restore focus over that field.
Changing the tab, view or address expires the dialog's captured recovery
target. Technical details share the dialog's scrolling detail region.

## Verification

The focused product suite passed 33 checks, including recovery shortcuts,
safe initial Enter, disabled Back at a history boundary, stale owners, and
action navigation in 320-pixel windows at 26-point text. The native-content
suite passed 41 checks, including same/changed/empty fragments on GET and POST
documents, deferred history entries, failed/pending documents, SPA routes and
stale completion guards. Rendered normal, compact and large-text dialogs were
inspected. A pre-existing library rendering wait now explicitly requests a
frame before waiting; all 71 library checks passed.

The final all-target release build passed. The freshly built Qt-engine unit
test executable was selected from Cargo's JSON artifacts and run directly:
495 passed, none failed, and one opt-in hint-performance check was ignored.
The full QML suite passed all 1,027 checks.

All six compiled native gates passed against the final resources:

| Gate | Result |
| --- | --- |
| `application_navigation_recovery` | Passed, 24.21 s |
| `application_keyboard_guide` | Passed, 16.33 s |
| `application_navigation_recovery_wayland` | Passed, 24.58 s |
| `application_keyboard_guide_wayland` | Passed, 15.94 s |
| `application_page_search` | Passed, 39.68 s |
| `application_navigation_usability` | Passed, 68.25 s |

The native fixture rejects stale embedded recovery, window and navigation
resources before testing behavior; this guard rejected the superseded build.

The production journey uses an owned loopback server that drops requests,
then serves a recovered document. It checks fresh GET recovery after a failed
POST with a fragment, explicit GET after a successful POST, exact URL preservation, same-address
Open, address editing and submission, focus handoff, Back and owner isolation.
The completed POST's Reload decision initially selects Keep page. Enter sends
no further request; confirming bypass-cache Reload sends exactly one POST with
the original owned form payload.
It also checks changed and unchanged fragments on a completed GET document,
including Back between fragments, without issuing another network GET. It runs
in primary, secondary, private and popup windows. Its Wayland gate uses compositor
seat keys and checks the native shortcut override and release, with at most one
delivered press and no repeats or modifiers. Qt consumes matching shortcut presses
before window event delivery. Popup creation is fixture setup
using Qt events.

Final logs are `target/keyboard-guide-final-build.log`,
`target/keyboard-guide-final-rust-tests.log`,
`target/keyboard-guide-boundary-qml-full.log`,
`target/keyboard-guide-boundary-final-offscreen.log`,
`target/keyboard-guide-boundary-final-wayland.log` and
`target/keyboard-guide-boundary-final-regressions.log`. Cargo artifacts and the
selected test executable are recorded in `target/keyboard-guide-final-build.jsonl`
and `target/keyboard-guide-final-rust-executable.txt`.
Production Guide, recovery and form renders are under
`target/keyboard-guide-final-offscreen-artifacts` and
`target/keyboard-guide-final-wayland-artifacts`. Normal and compact renders
were inspected.

## Ordinary form reload

The fresh-request recovery path does not qualify ordinary Reload. A separate
owned browser process loaded a successful POST response completely, then
received the public Reload command. Its loopback server observed a second
POST with the same body without another decision. Qt 6.11.2's adapter calls
Chromium Reload with `checkRepost = false`.

Browser Reload commands now ask before resending a completed or failed POST. The decision
occurs before issuing native Reload. Keep page is initially selected; Enter,
Escape or `n` cancels and keeps the document and its previous loading state. `y`
resubmits with the captured normal or bypass-cache option. The prompt checks
the typed target, renderer document owner, view, tab and host before acting.
Changing any owner expires it without restoring focus over the new selection.
Reloading a background form tab requests selection before confirmation.

All 18 focused form-resubmission QML checks passed, including cancellation,
one-use confirmation, cache options, stale owners, background tabs and compact
large-text layouts. The extended native journey passed in all four window types,
offscreen and on Wayland, using the final compiled resources.

Repeating the original public CLI reproduction against the final binary left
the server at one POST before and after Reload, with no resubmission during
the observation interval. The active-tab query retained the same URL, title
and document revision, and reported a completed load while awaiting confirmation.
The disposable observation is
`target/form-reload-observation-v3/observation.json`; the runner is
`target/probe-form-reload-v3.py`.

The disposable reproduction and observations are
`target/probe-form-reload.py` and
`target/form-reload-observation-v1/observation.json`.

A separate Qt-only probe confirmed that a POST reload is reported as
`ReloadNavigation` with `hasFormData = true`. Rejecting that request emits a
stopped load (`-3`) while preserving the existing document; a later accepted
Reload resubmits normally. The implemented browser-command gate keeps the
decision ahead of that native load transition. The
probe and event trace are `target/probe-form-navigation.py` and
`target/form-navigation-probe.log`.

## Failed forms and window-owned Guide

A follow-up native test found that ordinary Reload after a failed POST still
resent the form without confirmation: its server observed the POST count rise
from two to three. The completed-document gate had skipped failed responses.
The baseline is `target/failed-form-reload-baseline.log`.

The gate now uses the last engine load result and the failed navigation's
method. Cancelling restores the failed state. If the error dialog was open
when Reload was requested, its input, opacity and accessibility are suspended
during confirmation. Its original details and captured owner remain intact;
cancellation reveals that same dialog again. An already dismissed error dialog
stays dismissed. Confirming retains the cache option and original owned payload.
Failed GETs do not inherit an earlier POST document's method.

The keyboard Guide was unavailable in secondary, private and popup windows.
The native baseline also found that a new `binding-explain` query could update
the model without updating the input after the user cleared an earlier query.
The baseline is `target/keyboard-guide-baseline.log`.

Each window now owns its Guide, effective UI model, query and focus restoration.
Auxiliary windows create it on first use. Their command bars accept
`binding-list` and `binding-explain`. Explicit queries update the live input,
reset section filters and leave compact article mode. Reopening a compact
Guide focuses its visible search field. A Guide in another window keeps its
query and stays usable independently.

The focused form suite passed 28 checks, the chrome suite passed 49 and the
final full QML suite passed 1,027, including two extra checks for suspended
error ownership. The final compiled recovery and Guide journeys passed in all
four window types, offscreen and through the owned Wayland seat. The Guide
journey types `binding-list` in each command bar, uses `g ?`, and verifies a
new `binding-explain` query after clearing the old input. Navigation, page
search, the public CLI probe and all 495 Qt-engine Rust checks also passed.

The owned compositor resolves the physical `AB10` key for `Shift+/` using its
US keymap. Qt's semantic Slash lookup selected keypad divide, which could not
produce the Guide's `?` key. The fixture records native shortcut/press/release
events and modifiers; it does not connect to the user's compositor.

Further form qualification remains around Reload during an in-flight or stopped
navigation. These checks cover completed and failed form responses.

## Guide search relevance and keyboard context

The compiled Guide renders exposed another rough edge: searching `zoom`
selected Choose where to scroll, with Page zoom below two scrolling articles.
Search previously treated matches in shared article text and section names
as equal to a feature name, then kept the previous selected row as the query
changed. Its footer also advertised `j/k browse` while those keys typed into
the search field.

Search now ranks exact names, configured commands and shortcuts first, followed
by prefixes, partial names, summaries, article text and section/mode matches.
Every shortcut for a matching feature stays in the reader, ranked together by
its best match. Exact shortcut case has priority (`o` and `O` remain distinct),
and printable chains accept both `g g` and `gg`. A command may include its `:`
prompt prefix. Article and category searches remain available.

Results use relevance order without repeated section headings. Clearing the
query restores the section catalogue. Changing the query selects its best
result; refreshing the same query retains the user's chosen article. Keyboard
hints follow the focused search, list, article, copied command, section chooser
or disclosure control, including compact windows. The modal footer advertises
the Escape action that actually applies, and the command header identifies
`:binding-list`. Escape cancels an open section chooser before closing the Guide.

Two new QML checks failed against the previous implementation: changing the
query left Reload selected instead of the first result, and search focus still
advertised list navigation. The baseline is
`target/guide-search-baseline-qml.log`. The focused start-page suite now passes
71 checks and the chrome suite passes 52. Compact 320-pixel/26-point and
480-pixel/20-point renders were inspected in `target/guide-search-qml-artifacts`.
The chrome fixture uses the same inherited font and palette contract as the
browser windows.

The compiled Guide journey additionally checks that typing `zoom` selects Page
zoom, compact `gg` selects Jump to the top or bottom, reader/list hints follow
real key events, and a configured `x` shortcut finds zoom while retaining its
other shortcuts. It runs in all four window kinds, including through the owned
Wayland seat. Final verification for this follow-up is recorded below.

The complete QML run passed 1,031 checks; its sole failure was the forced-GC
stress check because the invocation omitted Chromium's required
`--js-flags=--expose-gc`. Rerunning that check with the CI flags passed all
1,000 scope create/cancel cycles with forced collection. This qualifies all
1,032 QML checks across the full run and the corrected targeted invocation.
Logs are `target/guide-search-qml-full.log` and `target/guide-search-qml-gc.log`.

The all-target release build completed in 13 min 32 s. Its fresh Qt-engine unit
test executable passed 498 checks, with one opt-in performance check ignored.
Cargo's artifact records and the selected executable are retained in
`target/guide-search-final-build.jsonl` and
`target/guide-search-final-rust-executable.txt`; the build and test logs are
`target/guide-search-final-build.log` and
`target/guide-search-final-rust-tests.log`.

The application harness was linked against the new engine archive. These
compiled gates passed:

| Gate | Result |
| --- | --- |
| `application_keyboard_guide` | Passed, 41.88 s |
| `application_keyboard_guide_wayland` | Passed, 39.71 s |
| `application_navigation_usability` | Passed, 91.51 s |

Logs are `target/guide-search-final-native-link.log`,
`target/guide-search-native-offscreen.log`,
`target/guide-search-native-wayland.log` and
`target/guide-search-native-navigation.log`. These runs overlapped release
optimization and are functional qualification, not timing benchmarks.
Production renders are in `target/guide-search-offscreen-artifacts` and
`target/guide-search-wayland-artifacts`; primary search, compact private search
and popup reader renders were inspected. Page zoom is selected first, search
hints describe typing, and the popup reader describes scrolling and return to
the feature list. Formatting, whitespace and the 278-row requirements register
checks passed. No diagnostic source override was used.

Those renders also exposed an editorial opportunity: movement features shared
a broad explanation, so Page zoom started by explaining history and scrolling.
The next pass addresses those articles and keyboard access to Guide search.

## Focused movement articles and keyboard access to Guide search

Zoom, short scrolling, page steps, top/bottom jumps and scroll-target selection
now have separate explanations and recovery guidance. The zoom article explains
the current tab's scale, default shortcuts, explicit percentages and returning
to 100%. This corrects the former claim that reset used a configured baseline:
the command handler explicitly uses 1.0. Page-step help explains the overlap
between full-page steps and how the selected panel determines viewport size.
The generated command manual uses the same authored content and was regenerated.

From the Guide's feature list or reader, `/` now reveals and focuses search,
including from a compact article. The current query is selected for replacement.
A slash typed into the search field remains literal input. The footer exposes
this shortcut where it applies. Search ownership and deferred focus stay inside
the existing Guide instance.

The new keyboard check failed in all four baseline cases: wide/compact layouts
from the list/reader. The baseline is `target/guide-articles-slash-baseline.log`.
All 75 focused start-page checks now pass in
`target/guide-articles-qml-start.log`. The full repository check passed all
1,036 QML checks with the required forced-GC flag, followed by the profile-lifetime
and download-source gates.

The native journey now requests Guide search from both the list and reader
using actual key events, replaces the selected query, and captures the focused
zoom article. It checks that webpage scroll, browser mode and page-search text
stay unchanged. The Wayland journey uses the owned compositor's physical `AB10`
key for `/` and verifies native gesture delivery.

`cargo xtask check` completed successfully. It also qualified QML lint,
requirements and architecture contracts, generated references, release/license
policy, formatting and all workspace Rust tests: 1,124 passed, none failed,
and two opt-in performance/label-evaluation checks ignored across 18 suites.
Its log is `target/guide-articles-xtask-check.log`.

The all-target release build passed in 21 min 11 s, overlapping the workspace
check's native compilation. The build log and Cargo artifacts are
`target/guide-articles-final-build.log` and
`target/guide-articles-final-build.jsonl`. The application harness links the
fresh production archive and rejects stale embedded Guide/window resources.

| Gate | Result |
| --- | --- |
| `application_keyboard_guide` | Passed, 19.81 s |
| `application_keyboard_guide_wayland` | Passed, 15.27 s |
| `application_page_search` | Passed, 41.33 s |

The initial compact render placed the article's explanation below the first
viewport. An added native Page Down check confirms that its content moves,
keeps the selected article, and preserves the webpage's scroll position in all
four window kinds. Both Guide gates passed again with that check. The wide
zoom article, compact popup article, scrolled popup body and private-window
movement article were inspected. The explanation is reachable and readable;
the keyboard footer remains visible while reading.

Final native logs are `target/guide-articles-final-native-link-v2.log`,
`target/guide-articles-native-offscreen.log`,
`target/guide-articles-native-wayland.log` and
`target/guide-articles-native-page-search.log`. The earlier Guide runs are
retained with the `-pre-scroll.log` suffix. Renders are in
`target/guide-articles-offscreen-artifacts` and
`target/guide-articles-wayland-artifacts`, including
`keyboard-guide-zoom-body-popup.png`. Source/whitespace and the 278-row
requirements register checks passed. All build, test and probe jobs completed.

## Streaming-form Reload baseline

While those builds ran, a separate public-CLI probe checked the remaining form
lifecycle gap using an owned loopback server and disposable browser roots on
an isolated D-Bus session. Its form submits only once per session, so reopening
the form's GET document cannot silently submit again through page JavaScript.
The server holds the POST response open. For the streaming case it sends HTML
that calls back to `/committed`, proving the new document ran before Reload,
while the response's remaining bytes are still withheld.

The streaming case observed one POST before Reload and two afterward, with the
same `payload=owned-fixture` body, without another user decision. This is a
confirmed remaining defect. The core query reported a provisional load during
the streaming response. Completed/failed-form protection above does not qualify
this case.

Before response headers, Reload instead reopened `/form` with GET and the POST
count stayed at one. After stopping a committed streaming response, the core
reported complete and Reload produced no additional POST during the observation
interval. The stopped case still needs native prompt, cancellation and approval
qualification; the observation alone does not establish the full workflow.

The runner is `target/probe-form-loading.py`. Observations are
`target/form-reload-streaming-baseline-v2/observation.json`,
`target/form-reload-pending-baseline-v2/observation.json` and
`target/form-reload-stopped-baseline-v3/observation.json`, with matching `.log`
files under `target`. The first streaming probe incorrectly waited for a title
in the core query; that query publishes its commit state at load completion.
The revised probe uses the owned page callback as commit evidence. The first
stopped probe also assumed the core would retain an incomplete loading state;
the corrected probe records both the accepted Stop and the withheld response
tail. These were probe precondition failures, not product corrections.


## Reloading a form response before completion

The confirmed streaming POST defect above is addressed at the native Reload
navigation policy boundary. Browser Reload captures its target and cache option;
Qt reports whether the entry it will actually reload contains form data. A POST
reload is rejected before another HTTP request and presented as the existing
keyboard decision. A pending POST can still reload the preceding GET without
an unnecessary form decision.

Rejecting Reload during a live response leaves its original connection and DOM
running. Idle rejections emit a transient start/stop pair, which is kept from
replacing the document owner or losing the original resting state. Approval
permits one reload with the captured cache option. Local navigation serials join
the existing tab, generation, document, view, UI, host and URL guards. Cancelling
a site's BeforeUnload decision releases the captured reload for a later retry.

Natural response completion keeps the same document target. An open form
decision therefore remains usable, updates its recorded state to complete, and
preserves that state if Reload is repeated or Keep page is chosen. The new
stopped-navigation acknowledgement settles only its captured current target and
issues no engine command. Runtime observations retain that tab, window and
captured cancelled state even after a later Reload.

Source checks passed: 1,051 QML cases, including 43 focused form cases; 157 core
unit tests; and eight runtime unit tests. Real WebEngine source-component probes
also passed cancellation, approval and BeforeUnload retry against owned loopback
responses. Their request logs contain exactly the original POST and its one
approved replay with the same body. These source probes precede the compiled
application qualification recorded below.

The change covers captured browser Reload commands. Broader renderer failure
handling and existing internal or site-triggered direct reloads remain separate
qualification work.


The rebuilt production executable passes the original public-CLI reproduction
in all three modes. With the response's tail withheld and the owned commit
callback observed, streaming Reload keeps POST count at **one to one** and the
core load remains provisional. The baseline was **one to two** with an identical
body. Before headers, Reload again GETs the preceding form page with one total
POST. After Stop, Reload also keeps one total POST; Qt reports that stopped
entry as complete in this public query. The native journeys separately qualify
the form decision and keyboard cancellation/approval.

The probes use separate temporary XDG/browser roots and private D-Bus sessions.
Current observations are `target/form-reload-{pending,streaming,stopped}-fixed-v1/observation.json`
and matching `.log` files, using `target/probe-form-loading-fixed.py`.
The release build completed in 14 m 40 s. Release core tests passed 157 cases;
release engine tests passed 498 cases with one opt-in case ignored. Logs are
`target/streaming-form-final-build-v3.{log,jsonl}`,
`target/streaming-form-release-core-tests.log` and
`target/streaming-form-release-engine-tests.log`.


The first compiled form gate passed 16 of 20 data rows. All four BeforeUnload
rows prevented an unapproved POST and retained the document, but failed the
following `j` scroll. Detailed native tracing established that the browser had marked the
interrupted response failed. Keep page correctly restored its failure dialog,
which owned the key. The journey now records the state presented by the engine,
asserts that cancellation preserves it, verifies the restored failure surface,
and dismisses it with Escape before checking page scrolling. The original
scroll assertion remains.

A separate focus guard substitutes the captured page view when a form decision
opens with hidden, disabled or root focus, and retains an available prior
control. Three source regressions cover those cases; all 46 focused form checks
and all 1,054 QML checks pass. The earlier focus-only diagnosis did not explain
the native BeforeUnload failure; the active failure modal did.

Initial failure and diagnostic logs are
`target/streaming-form-offscreen-ctest.log`,
`target/streaming-form-native-focus-diagnostic.log` and
`target/streaming-form-focus-diagnostic-v3.log`. Temporary focus diagnostics
were removed after identifying the owning surface; source-parity guards were
retained throughout.


The next Wayland run passed the other 16 rows but exposed a second issue at the
BeforeUnload retry boundary. After Stay, the view reported `loading: false`
while content state remained loading. An immediate retry captured a native
attempt that received no policy callback. Retaining verified form metadata
across an approved Reload is separately covered by a source regression, but
that change alone did not resolve these native failures: the site dialog had
appeared before a form decision in this fixture.

The decisive timing evidence is Qt's `QQuickWebEngineViewPrivate::loadFinished`:
it changes `isLoading` to false, then posts `loadingChanged` with a zero-duration
Qt timer. The test retried in that interval. A source regression that delivers
the old load result immediately after requesting Reload failed against the
previous code. Captured browser Reload requests now wait for an owned timer,
revalidate their owner and typed target, and preflight against the settled load
state. A replaced document expires the queued request; queued repeats retain
the first cache option. This avoids issuing another native reload in the gap.

Verified method information also survives an approved Reload policy callback
until a replacement document starts; it never carries approval into a retry.
Stay acknowledges a stopped view when the corresponding notification has not
yet arrived. Other navigation types discard the verified entry, and target,
view, UI, host, URL and document-owner checks remain enforced.

All 50 focused form checks pass, including the formerly failing timing sequence,
queued-owner expiry and cache-option coalescing. Both updated real-WebEngine
source probes pass. The pre-fix regression log is
`target/streaming-form-reload-dispatch-regression-before.log`; current logs are
`target/streaming-form-deferred-reload-source.log`,
`target/streaming-form-deferred-beforeunload-real-qt.log` and
`target/streaming-form-deferred-fragment-real-qt.log`. Native trial logs are
`target/streaming-form-wayland-retry-diagnostic-v2.log` and
`target/streaming-form-v5-wayland-beforeunload.log`.

The preceding source suite passed 1,055 QML cases, including 47 focused form cases;
the final deferred-dispatch suite is qualified below.
A separate owned WebEngine source probe also passes cancel, bypass approval and
stopped-response handling when the POST address contains `#result`. Its native
Reload URL checks preserve the fragment without rejecting the authorized replay.
That probe is `target/probe-stream-native-content-fragment.py`, with log
`target/streaming-form-current-fragment-source-probe.log` and observations in
`target/stream-native-content-fragment-probe/observation.json`.


The deferred-dispatch source suite passes **1,058 QML cases**, including **50
focused form cases**, with no failures or skips. The full-suite log is
`target/streaming-form-final-qml-v4.log`. The two real-WebEngine source probes
also pass against this source. Compiled application qualification follows below.


### Final compiled qualification

The current release build (`v6`) completed successfully in **13 m 24 s**. Its
engine unit executable passed **498 cases**, with one opt-in case ignored.
The 157 core tests and eight runtime tests also pass; their source has not
changed since those runs. The final public-CLI streaming probe observes the owned commit callback,
withholds the response tail, and keeps POST count **one to one**, with the load
provisional both before and after Reload. The previous baseline was one to two.
Current logs are `target/streaming-form-final-build-v6.{log,jsonl}`,
`target/streaming-form-v6-release-engine-tests.log`, and
`target/streaming-form-v6-public-cli-streaming.log`. The CLI observation is
`target/form-reload-streaming-fixed-v6/observation.json`.

The four BeforeUnload cases pass independently on both platforms. The complete
form gate passes all **20 data rows offscreen and 20 on Wayland**, covering all
five phases in all four window types. Keep page sends no POST; approval sends
one identical-body POST where exercised. Live cancellation retains the arriving
response, completion during confirmation preserves a usable decision and the
completed state, and stopped cancellation preserves its resting state. Native
queued-load timing no longer leaves the immediate BeforeUnload retry pending.
When the browser reports an interrupted response failed, cancellation restores
its failure dialog and state; Escape then returns to page scrolling. Improving
that generic failure classification remains separate recovery work.

Fresh primary, secondary, private and popup renders were reviewed, including the
640-by-480 Wayland confirmation. Text, initial Keep page focus and key hints fit;
the webpage viewport remains behind the overlay. Render directories are
`target/streaming-form-v6-application_form_reload-artifacts` and
`target/streaming-form-v6-application_form_reload_wayland-artifacts`.
The form and recovery journeys compare their relevant source and embedded
resources before running; no source interceptor or diagnostic resource override was used.

| Final native gate | Result |
| --- | --- |
| `application_form_reload` | Passed, 123.01 s |
| `application_form_reload_wayland` | Passed, 60.84 s |
| `application_navigation_recovery` | Passed, 24.74 s |
| `application_navigation_recovery_wayland` | Passed, 22.55 s |
| `application_navigation_usability` | Passed, 65.08 s |
| `application_page_search` | Passed, 36.20 s |

Logs use `target/streaming-form-v6-{gate}.log`. Both qualification runners
finished with exit code zero and a `complete` status. Formatting, whitespace,
requirement-table shape and the six registered gate names were checked.
NAV-006 remains in progress for broader failure handling and internal or
site-triggered direct reloads outside this captured command path.


## Stay keeps the page without a generic failure

The next pass addresses the remaining error dialog after choosing Stay on an
unfinished response. A read-only observer in the native fixture recorded Qt's
terminal event on both platforms: status failed, error code 0, error domain 0,
empty error text, and the same retained document URL. The stricter native
baseline failed because the browser reported failed instead of cancelled.
Observations are `target/stay-interruption-observe-{offscreen,wayland}.log`;
the strict baseline is `target/stay-interruption-baseline.log`. The temporary
observer was removed after collecting those facts. Resource parity checks were
retained throughout.

An explicit Stay now captures the exact typed target and existing view, UI,
host, document owner, address and navigation serial. Only its next matching
terminal event may use cancellation semantics. A stopped event or a failure
with no error facts settles the interrupted response as cancelled and retains
an already completed or failed state. Genuine errors, downloads, error pages,
new navigations and stale owners retain normal handling. Automatic prompt
cleanup does not create a cancellation record. The owning active window reports
Stayed on page; no replacement page or HTTP request is issued.

All 79 focused form checks pass, including real error facts, single consumption,
owner expiry, completed state and ordinary non-form documents. The updated real
WebEngine source probe passes as well. Logs are
`target/stay-interruption-focused-qualified-v2.log` and
`target/stay-interruption-real-qt-source.log`.

The native journey now has 44 data rows: the existing response phases plus Stay
on an unfinished page, immediate Reload retry, Stay on a completed page,
and cancellation of Open address and Back on both unfinished and completed pages,
in primary, secondary, private and popup windows. Its POST address has a fragment
for these Stay cases. It requires no error dialog, retained document contents,
correct loading state and `j` scrolling immediately after Stay and after Keep
page. A repeated site prompt is accepted explicitly with Tab to Leave page and
Enter before native form policy is evaluated. Final compiled results follow.


The expanded v7 native run exposed the same issue after Open address: Qt first
exposes the proposed address, then rolls it back to the retained document after
Stay. The captured and terminal typed target, local serial and document owner
were identical, while the address changed from `/other` back to `/posted#stay`.
Completed Open and Back cases already passed. Read-only owner traces are
`target/stay-open-observe-beforeunload-{open,open-complete,back-complete}.log`.
Their temporary observer was removed after diagnosis.

Cancellation now validates all document/target ownership fields and requires
the terminal URL to agree with the retained engine URL. The stricter captured
URL check remains on form-resubmission actions. A source regression proves this
specific rollback, and all **80 focused form checks** pass. The updated
real-WebEngine Open-address source probe also passes. Logs are
`target/stay-interruption-focused-rollback.log` and
`target/stay-interruption-open-real-qt-source.log`. The current complete native
matrix has **44 rows per platform** and checks the retained address as well as
page contents, loading state, absence of a failure dialog and keyboard scrolling.

The first full v8 Wayland run passed 43 rows and failed the secondary unfinished
Back case before it could exercise Stay. Its diagnostic reported No earlier
history and a single recorded form visit, whereas passing runs also had the
startup visit. A new view can report `loading=false` before its startup request
runs. The fixture now waits for completed startup and form documents to enter
Ferric's history and asserts that Back is available before invoking it. Native
engine-history availability was not captured in that exact failing run; four
later diagnostic runs with that field all passed. This correction establishes
the Stay test's required setup. It does not qualify Back destinations during
streaming responses: history reconciliation currently waits until loading ends,
which remains a separate navigation follow-up.

### Compiled Stay qualification

The current all-target release build completed successfully in 13 min 19 s.
Its fresh Qt-engine unit executable passed **498 tests**, with one pre-existing
ignored test. The full QML suite passed **1,088 tests**, including all 80 focused
form checks. The unchanged core and runtime sources retain their earlier
157- and 8-test passes. Build and test logs are
`target/streaming-form-final-build-v8.{log,jsonl}`,
`target/streaming-form-v8-release-engine-tests.log` and
`target/stay-interruption-full-qml-v2.log`.

The current public-CLI probe also passed: its owned commit callback arrived
while the response tail was withheld, Reload was accepted, POST count remained
**1 → 1**, and the same tab remained provisional before and after the request.
The observation is
`target/form-reload-streaming-fixed-v8/observation.json`; the release runner
finished with a successful complete status.

Ten targeted native cases passed on each platform before the complete matrix.
The complete form matrix then passed **44 rows offscreen and 44 on Wayland**.
Fresh popup and 640-by-480 secondary/private confirmation renders were reviewed:
Keep page has initial focus, both choices and their keyboard hints fit, and
the existing page remains behind the modal. The post-Stay screenshots show the
scrolled response viewport; the fixture's text is above that position. Retained
document markers, addresses and actual scroll offsets are asserted separately.
Artifacts are under
`target/streaming-form-v8-application_form_reload{,_wayland}-artifacts`.
Compiled/source resource parity checks remain enabled, with no resource override.

| Current native gate | Result |
| --- | --- |
| `application_form_reload` | Passed, 147.10 s |
| `application_form_reload_wayland` | Passed, 142.27 s |
| `application_navigation_recovery` | Passed, 24.83 s |
| `application_navigation_recovery_wayland` | Passed, 23.48 s |
| `application_navigation_usability` | Passed, 65.66 s |
| `application_page_search` | Passed, 35.67 s |

Logs use `target/streaming-form-v8-{gate}.log`. Both native and release runners
finished with exit code zero and a successful complete status. Formatting,
whitespace and the 279-row, eight-column requirements table were checked.
NAV-006 remains in progress for the streaming-history follow-up, broader failure
handling, and internal or site-triggered reloads outside this command gate.

## Back follows the visible streaming document

The next native baseline establishes two completed HTTP visits, then holds a
third response open after its document runs. Shift+H incorrectly skips the
preceding form page and lands on the startup page. Both GET and POST cases fail
the destination assertion. The read-only diagnostic shows native history index
2 and native Back available, while Ferric's history cursor is still 1 with only
the two completed visits. The response tail was never sent. Evidence is
`target/streaming-history-native-baseline.log`; source/resource parity checks
passed before those assertions, without an override.

History commands now wait for the exact view's document probe before selecting
a destination. A proven streaming document can enter history before its load
finishes. A probe of the preceding document while response headers are pending
keeps that document's recorded history position. Neither path changes the
loading lifecycle or assigns a form method. Delayed commands retain the typed
target, document owner, UI, host, address and navigation serial; new navigation
invalidates them. Signed counts arriving while the same probe is pending combine
within the supported bound. Native start and blank entries use their logical
history immediately. Missing proof while a response is in flight reports that
history is changing instead of choosing a stale destination.

The focused source suite passes 55 tests, including streaming history without
load completion, pending headers, queued counts and eight owner-expiry paths.
The native matrix has 40 rows: GET and POST in primary, secondary, private and
popup windows, each covering ordinary Back, counted Back, pending headers,
a native start-page anchor and an earlier document with the same URL across
an intermediate visit. A standalone public-Qt probe showed that a consecutive
same-URL navigation can replace its engine entry, so the native fixture
establishes that intermediate visit before testing `2H` to the older document.
That observation is `target/streaming-history-native-same-url-observation.log`;
it is engine behavior evidence, not a compiled Ferric qualification. The final
compiled matrix passes **40 rows offscreen and 40 on Wayland**, after ten focused
cases pass on each platform. The previous failing primary GET and POST journeys
now reach the immediately preceding form page.

The all-target release build passed in **13 min 51 s**, and the current engine
unit executable passed **498 tests** with one existing ignored test. The complete
current QML suite passed **1,102 tests**, including the 55 focused history checks.
The current public-CLI Reload probe also passed with the streaming response tail
withheld: POST count remains **1 → 1** and the tab remains provisional. Logs are
`target/streaming-history-build-v1.{log,jsonl}`,
`target/streaming-history-full-qml-final-v1.log`,
`target/streaming-history-v1-release-engine-tests.log` and
`target/streaming-history-v1-public-cli-streaming.log`.

| Current native gate | Result |
| --- | --- |
| `application_streaming_history` | Passed, 116.81 s |
| `application_streaming_history_wayland` | Passed, 118.25 s |
| `application_form_reload` | Passed, 148.74 s |
| `application_form_reload_wayland` | Passed, 142.97 s |
| `application_navigation_recovery` | Passed, 24.44 s |
| `application_navigation_recovery_wayland` | Passed, 23.32 s |
| `application_navigation_usability` | Passed, 65.31 s |
| `application_page_search` | Passed, 36.11 s |

Logs use `target/streaming-history-v1-{gate}.log`. Both qualification runners
finished with exit code zero and a successful complete status. The three
production QML files retain their build-time hashes, and the native matrix
compares source and compiled resources without a resource override. Formatting,
whitespace and the requirements table were checked.

A separate current-binary public-CLI probe found a remaining saved-Journey
alignment defect. It completes `/first` and `/form`, verifies that the durable
current pointer names `/form`, then holds a GET `/stream` after its owned commit
callback. Back correctly returns the visible tab to completed `/form`, but the
durable `journey_current` row points to `/first`. Read-only SQLite inspection
records the mismatch after the return completes. The regression assertion fails
as expected; the browser and all data belong to a disposable probe root.
Evidence is `target/streaming-journey-v1/observation.json` and
`target/streaming-journey-v1.log`.

The source audit explains the gap: the Journey path records completed loads,
whereas the content history can now include a committed streaming document.
`JourneyGraph::request_traversal` stores an offset against the former path,
and `commit_navigation` consumes it without verifying the actual destination.
Queued signed counts require the same alignment review. Native Back destination
checks do not qualify this saved graph; reconciling committed-document history
with Journey lifecycle remains the next task. No completion claim is made for
NAV-004 or the broader usability goal.

### Journey follows content-visit identity

The next implementation binds each completed content-history entry to its
Core Journey node. Export/import reconciliation preserves that opaque reference
only while its node, tab ownership and URL remain valid. Engine commits use a new
typed `CommitNavigationAtVisit` event. It selects the exact owned visit when
available, or records the actual committed destination when its reference is
missing, stale, foreign or redirected. It clears the old pending offset in
either case. Fresh navigations also use this commit path and cannot consume
leftover intent after Stay or a cancelled command. Ordinary reducer users keep
their existing offset contract; the Qt adapter no longer assumes Journey and
engine history have identical positions.

The Core suite passes **162 tests**, including new checks for a shorter Journey
path, separate visits with identical URLs, missing/mismatched references,
peer-tab isolation and content-entry identity retention. All **8 runtime tests**
pass. The native matrix now queries Journey around each traversal and requires
the original visit ID, not just its URL. Against the previous production archive,
the primary and private streaming GET cases fail those assertions after Back
correctly reaches the form page. The trace is
`target/journey-identity-native-baseline.log`; no production resource override
was used. These native Journey queries are memory-backed; the existing
public-CLI/SQLite regression checks the durable pointer separately and now also
requires its original node ID. Current release and native qualification remain
in progress.

This change preserves the existing policy of recording a Journey visit when
its navigation completes. It does not claim that an unfinished streaming visit
has already been durably recorded.

Before v1's build finished, a public Qt URL observation established another
boundary fact: a QML URL string decodes spaces and Unicode (`café`, `λ`), while
the Rust/Qt canonical boundary stores their encoded form. Direct comparison
would discard valid Journey bindings. That build and its waiting qualification
runners were deliberately interrupted for this verified defect, not restarted
because a wait expired. The observation is
`target/journey-url-observation.log`.

The corrected v2 implementation compares through the existing canonical URL
boundary after a direct string match. Reserved encoded separators and distinct
fragments still compare differently. Import accepts only references already
owned by the tab's prior snapshot and still present at the matching canonical
Journey address. The ownership check uses the tab's reference set rather than
searching its full Journey path for every history row. A new engine unit check
covers Unicode paths, encoded query text, IDN hosts and distinct separators or
fragments. The native matrix is expanded to **48 rows per platform**, including
encoded-address GET and POST journeys in each window type. Core 162 and runtime
8 tests pass; the v2 release build and compiled qualification are in progress.

### Compiled v2 findings and v3 corrections

The v2 all-target release build passed in **15 min 41 s**. Its ordinary public-CLI
Journey regression now restores the original durable node ID, and the streaming
Reload probe still sends no extra POST. The engine suite reported 498 passed,
one failed and one ignored: the new URL regression rejected the native query
containing a decoded space. The existing strict Rust validator rejects raw
spaces before the Qt canonical conversion can re-encode them. Logs use
`target/journey-identity-v2-{release-engine-tests,public-cli-journey,public-cli-streaming}.log`.

The expanded native regression also fails after two Back commands combine,
a fresh Open completes, and another Back returns to the original page. The
current Journey ID is a new node rather than the captured original ID. The
initial diagnosis suspected queued command markers; the v3 trace below isolates
the remaining native history race. Evidence is `target/journey-identity-v2-offscreen-canary.log`. The fixture now has
**56 rows**, including this sequence for GET and POST in every window type.

A second public-CLI case leaves `/stream` unfinished, completes `/after`, then
returns to `/stream`, whose second response completes. Before returning there
is no stored stream node. The visible tab ends on completed `/stream`, but the
durable pointer remains on `/after` and no stream node is written. This exposes
the distinction between initiating a history action and actually reusing a
stored visit. Evidence is
`target/streaming-journey-identity-v2-retry-unrecorded/observation.json`.
The first version of this extra probe encountered a locked database while
scanning profile files; the corrected reader caches the discovered Journey
database, and the retry above reaches the actual pointer assertion.

V3 observes each main-frame engine navigation's history/fresh kind against its
exact typed target. It removes the accepted-command traversal queue, which
cannot model combined commands reliably, and consumes or clears policy facts
with their load lifecycle. Core node reuse, not command intent, determines
whether persistence selects an existing ID or records a new node. Native URL
text re-escapes literal spaces before applying the unchanged strict Rust/Qt
URL and origin checks. Content history retains the native representation for
engine matching while validating its escaped form. Reserved separators,
fragments, authority and control checks remain covered.

The revised complete QML suite passes **1,104 tests**. Focused navigation-policy
and form suites pass 57 and 80 checks.

### Compiled v3 findings and v4 corrections

V3's all-target release build passed in **13 min 12 s**. Its ordinary public-CLI
Journey and streaming Reload regressions pass. The unrecorded-stream probe
also passes: exactly one new stream node is stored and selected. The engine
suite reports **499 passed, one failed, one ignored**. Its remaining failure is
an obsolete assertion that the native QUrl conversion rejects a decoded space.
The corrected assertion requires `%20` at the native boundary and separately
checks that the raw user-input validator still rejects spaces.

The two coalesced native GET/POST cases still fail, while the other 12 primary
canaries pass. Read-only native Connections show Chromium changing the history
index to zero while the view still reports the preceding `/fresh` URL. Ferric
then treats that intermediate pair as replacement of the selected destination
and drops its Journey reference. A debugger trace confirms exactly one history
policy callback and one commit; the adapter passes no visit reference to Core.
This rules out duplicate commits as the cause. Evidence is
`target/journey-identity-v3-{history-observe,gdb-observe,core-observe}.log`.
No source-resource override was used, and the temporary observers were removed.

V4 preserves an explicitly selected content entry while its native URL and
history index settle. Three QML regressions cover model-first, URL-first and
same-URL updates. All three fail against v3 and pass after the correction;
the focused suite now passes **60 checks**. Logs are
`target/journey-identity-ordering-qml-{baseline,fixed}.log`.

A separate public-CLI reproduction holds an owned SQLite writer lock, completes
two visits to the same URL across a bridge, then goes Back two entries before
releasing the lock. V3 selects the newer durable UUID instead of the older
requested visit. Its writer lock lasts **1.54 s**, below the storage busy timeout.
The observation is under
`target/journey-history-v3-permanent/run-77e5a09r/pending/observation.json`.
The pending mapping already owns the exact UUID. V4 resolves both acknowledged
and queued mappings for pointer and parent writes, removing the URL fallback
from the navigation commit path.

The permanent `application_journey_history` CTest gate covers retained,
unrecorded and pending-write visits. It waits for the actual returned document,
then requires a later background visit to be saved before reading the original
tab's durable pointer. This storage barrier replaces a timing guess. The
baseline gate passes its retained and unrecorded cases and fails pending writes.
Core **162**, runtime **8** and the complete QML suite (**1,107**) pass for v4.
The full QML log is `target/journey-identity-full-qml-v4.log`. The all-target
release build passed in **16 min 18 s**. All **500 engine unit tests** pass,
with one opt-in benchmark ignored. Both public-CLI probes pass, retaining the
original durable visit ID and keeping streaming Reload at one POST. The
14-case primary canaries pass offscreen and on Wayland (16 QtTest passes each
including setup and cleanup). The full 56-row history matrices pass on both platforms (171.92 s offscreen,
167.64 s Wayland). Both form and recovery gates also pass. The wider navigation
gate stops with one failure: after mouse Back and another Back, `2L` lands on
the middle page. A focused rerun and a subsequent six-method navigation run
pass. This timing-sensitive case remains under investigation; page search and
the permanent SQLite gate were not reached by that runner.

The pending-write gate now polls through the public `tabs.query` IPC method,
while issuing navigation through the public CLI. A stricter baseline waits for
the returned document title (`Same visit 3`) before releasing its writer lock.
It still reproduces selection of the newer UUID, with a **2.87 s** lock hold
while the release build is active. Evidence is
`target/journey-history-v3-ipc-fixed-baseline/run-v8b2427q/pending/observation.json`.
An earlier CLI-polling trial under compilation load exceeded the probe's 4.5 s
lock budget and was rejected as a harness timing failure; no pointer result is
claimed for that trial.

### Pending-write ordering and completed same-address visits

The v4 pending-write probe reaches the returned third response and releases its
writer after 1.43 s, but observes only the first of two same-address visits after
its background barrier. A second run captures all stored nodes and confirms
that `/bridge` and the newer visit are still absent at that observation point.
These results do not establish lost writes: application admission can submit a
new request ahead of its existing backlog. A later background visit therefore
was not a valid FIFO barrier against the v4 coordinator. Evidence is
`target/journey-history-v4-pending-{early,nodes}.log`.

A deterministic coordinator regression queues two same-address visits and then
requests the older current pointer. The old coordinator executes the pointer
first and ends with the newer visit selected. The failing assertion is
`Some("newer")` versus `Some("older")` in
`target/journey-storage-order-baseline.log`. Admission now retains queue order,
including requests whose operation becomes available while older work waits.
Request validation remains immediate through the shared storage validator.
All **122 application unit tests**, **5 integration tests** and **105 storage
unit tests** pass; the new durable-outcome regression passes after failing
before the change. The log is `target/journey-storage-order-fixed.log`.

An additional native control completes two visits at the same address, across
an intermediate page, then returns to the older one. Each response carries a
nonce so an unchanged address cannot satisfy the wait before traversal occurs.
Both GET and POST controls fail against v4: the Core Journey ID remains the
newer visit. The history-success callback binds the selected content entry to
the previous Core current node before the real Core commit, and the same URL
allows that incorrect binding. Evidence is
`target/journey-identity-v4-completed-same-url-baseline.log`.

The adapter now preserves the selected reference while an owned engine policy
fact awaits commit. The normal commit then binds its actual Core selection.
This also avoids lending an old same-address reference to an unrecorded entry.
The expanded native matrix has **64 rows per platform**. These source changes
and the pending-UUID fix subsequently received the v6 compiled qualification
recorded below.

### Load success before the native history model moves

Two observed six-method navigation reruns pass, including mouse Back and counted
Forward; their read-only traces retain the numeric prefix while a prior Back is
still provisional. The temporary observers were removed. The native test keeps
an immediate prefix assertion and a final-history failure diagnostic instead of
silently retrying until a pass.

A deterministic QML regression exercises another permitted native order: the
returned document reports load success while Chromium's history model still
has the preceding index. The old `noteCommit` immediately publishes that index
on the selected entry. The regression fails with one history callback where
none is allowed; evidence is `target/journey-identity-late-model-qml-baseline.log`.
This establishes a stale-anchor defect that can introduce an extra logical
entry and affect later counts. It does not prove the exact cause of the earlier
intermittent native failure.

V5 carries the native index already resolved by the history request through the
shared view wrapper. Load success defers content-history publication until the
model reaches that index. Reconciliation retains it after load completion and
handles an originally stale logical anchor or a redirect. Stop, Stay and a new
fresh navigation release the captured position. Four success-order cases and
explicit Stop/Stay checks pass; the focused suite now has **66 passing tests**.
The log is `target/journey-identity-late-model-qml-final.log`.

Production source was frozen for v5 qualification, with 40 changed production
files recorded in `target/journey-identity-v5-source-hashes.json`. Full QML,
release and compiled native verification were started. The native runner now
checks durable Journey and the wider navigation gate before the longer complete
64-row matrices, to expose either remaining regression early. Native resource
parity checks remain enabled.

### Follow-up commands while a previous history position is pending

An additional deterministic check proved that retaining the selected anchor was
insufficient on its own. A follow-up `2L` could still execute while a previous
Back's model remained stale. The callback received `count: 2` but resolved
`offset: 1`, reproducing the middle-page destination without losing the numeric
prefix. The forward, combined-count and expired-owner baseline rows all fail
at this premature execution point. The captured values are in
`target/journey-identity-pending-position-qml-qualified-baseline.log`.

The v5 build and its waiting qualification runners were deliberately terminated
before changing production source, because of this verified defect. They were
not restarted because an observation wait expired. The earlier complete QML
run passed 1,113 tests before this new scenario was added; it does not qualify
the corrected follow-up path.

V6 leaves an accepted follow-up request queued until the previous request's
native position agrees. Counts continue to combine while waiting, and an
expired owner clears the request without execution. All **69 focused checks**
pass. The full QML suite was run before starting the next release build.

The final review also requires the explicit-position finalizer to consume its
same-document traversal proof. The three follow-up rows use a `popstate`
sequence and fail if that proof survives settlement; all 69 focused checks pass
after consuming it. The logs are
`target/journey-identity-position-proof-{baseline,fixed}.log`. The final full
QML run passes **1,116 tests** in **145.62 s**, including this refinement; production source is frozen under
`target/journey-identity-v6-source-hashes.json` before compilation.

The v6 release build and compiled qualification ran against that frozen source.
Logs use the `target/journey-identity-v6-` prefix and
`target/journey-identity-build-v6.{jsonl,log}`.

### V6 compiled verification

The final all-target release build passes in **12 min 53 s**. All **500 engine
unit tests** pass, with one opt-in benchmark ignored. Both public-CLI probes
pass, preserving the original retained visit and keeping streaming Reload at
one POST. All 16 primary native canaries pass offscreen and on Wayland (18
QtTest passes per run including setup and cleanup), including the completed
same-address GET/POST and coalesced cases.

The permanent SQLite gate passes in **10.69 s**. Its retained case restores the
original node; its unrecorded case stores exactly one new stream node; its
pending case selects the older of two distinct stored UUIDs after a **1.41 s**
writer lock. The observations are under
`target/application-tests/journey-history-artifacts/run-_z27tjuh/`.
The wider navigation gate, including mouse Back followed by `2L`, passes in
**65.00 s**. The final gate results are:

| Gate | Result |
| --- | --- |
| `application_journey_history` | Passed, 10.69 s |
| `application_navigation_usability` | Passed, 65.00 s |
| `application_streaming_history` | Initial run: 63/64 passed; later clean run: 64/64 passed, 201.16 s |
| `application_streaming_history_wayland` | Passed, 64/64, 207.80 s |
| `application_form_reload` | Passed, 44/44, 148.27 s |
| `application_form_reload_wayland` | Passed, 44/44, 143.49 s |
| `application_navigation_recovery` | Passed, 24.44 s |
| `application_navigation_recovery_wayland` | Passed, 23.53 s |
| `application_page_search` | Passed, 36.24 s |

The first full offscreen history run fails `private-post-pending`. Back reaches
the startup address, the renderer is complete and the logical history agrees,
but the UI retains `load_state: cancelled`. That run fails before the final
Journey identity assertion, so it does not establish the node outcome for
this case. No raw load-event trace was captured. The original failure remains
in `target/journey-identity-v6-application_streaming_history.log`.

Read-only observers then pass three runs of all eight pending GET/POST cases
and a full 64-case offscreen run. They capture no stopped load or cancelled
UI state. An initial observer attempt produced teardown errors from its own
late-bound registry target; those errors were corrected in the test harness
and are not browser failures. All temporary QML observers were removed.

The permanent fixture buffers native load metadata with `QSignalSpy` and prints
it if a cancelled state is observed at the completion check. A bounded
`FERRIC_HISTORY_PENDING_REPEATS` option repeats the pending flow in the same
owned window without changing the default matrix. With only that passive
buffer, **32 private POST-before-header Back cycles pass** in **20.61 s**, and
the original full offscreen CTest passes all 64 cases in **201.16 s**.
These passes do not explain or fix the original intermittent state. No
production Stop/cancellation change was made on that evidence.

The final clean log is
`target/journey-identity-v6-clean-application_streaming_history.log`; stress
evidence is `target/journey-identity-v6-private-post-pending-stress.log`.
Observer evidence is `target/journey-identity-v6-pending-load-observe-fixed-{1,2,3}.log`
and `target/journey-identity-v6-full-pending-ui-observe.log`. The six later
gate results are in `target/journey-identity-v6-remaining-status.json` and
their `target/journey-identity-v6-<gate>.log` files. Compiled resources were
compared with source throughout; all 40 frozen production hashes still match.

## Encoded streaming addresses and fragment visits — 2026-10-23

The review found two remaining address-representation defects. A renderer's
`location.href` retains percent encoding while QML's native URL text displays
spaces and Unicode. Back therefore rejected a verified streaming document, or
the preceding document while response headers were pending. An explicit Open
of an encoded fragment also missed same-document completion and could leave
the browser reporting a provisional load indefinitely.

The adapter now projects renderer observations and typed Open destinations
through Qt's URL representation before comparing them with the native view.
Document/target ownership and history-position checks remain in force. Encoded
path and query separators remain distinct from literal separators; this does
not use unrestricted percent decoding or change Rust's input validation.

Twelve focused baseline cases fail before the correction. All 83 focused
navigation checks pass afterward, including pending headers, GET/POST fragment
visits, encoded fragment text and separator distinctions. The full QML rerun
passes 1,221 checks. The first full run retained a timeout in the existing
selector resource-cap fixture; its isolated rerun and the full rerun pass
without changing that fixture or its limits.

The compiled history fixture now includes an encoded streaming URL as well as
an encoded preceding address, and adds encoded pending-header cases in every
window type for both GET and POST. Recovery's unchanged/changed fragment checks
also use encoded addresses and fragment text. Final compiled results, source
hashes and retained timeout diagnostics are recorded in
`target/review-issue-fixes-final-facts.json`.

All 72 compiled offscreen history scenarios pass (74 Qt checks including setup
and cleanup, 301.56 s). All 16 encoded GET/POST history scenarios also pass on
the owned Wayland compositor (18 Qt checks, 117.62 s). Both native recovery
gates pass with the encoded GET/POST and fragment addresses.

The expanded matrix has eight more history rows. Its overall CTest budget is
now 360 s, with a 330 s Wayland child-process budget and matching compositor
lifetime. Individual navigation assertions keep their existing deadlines. Two
earlier full runs reached the previous 300 s aggregate limit without an
assertion failure; both logs are retained. The successful final run executes
after compilation and the other heavy checks finish. The release build,
1,169 Rust tests, strict Qt-engine Clippy, formatting and whitespace checks
also pass; two opt-in Rust tests remain ignored.
