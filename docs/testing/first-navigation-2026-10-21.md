# First navigation from a native blank page — 2026-10-21

## Reproduction

During manager keyboard qualification, the Wayland journey gate twice failed
to persist its first loopback visit. Diagnostics showed the requested document
URL in the actual page, the same address in browser state, and `load_state`
equal to `complete`, while the fixture database contained only `about:blank`.

A separate run reproduced the same failure in the journey-destination fixture.
Its load-event spy saw only `about:blank` started and succeeded notifications,
although the requested first page was already rendered in the same live view.
The first rendered navigation and its recorded history disagreed.

The source-loaded trace then identified the sequence: renderer initialization
first selected the bootstrap model's temporary `about:blank`, then the native
blank page, then the real address. The outstanding blank load completed after
the real address was selected and overwrote its commit.

## Repair

The shared web-content renderer starts with an empty URL and defers its first
model-based initialization until the current startup work has finished. The
timer reads the latest model and requires a live owner. An explicit navigation
sets the initialized flag first and takes precedence. This prevents the
temporary bootstrap address from starting an engine load before the intended
native or web content is selected.

Popup adoption is also an explicit navigation. Qualification caught a pending
initializer replacing an adopted popup with `about:blank`: all five popup-source
new-window cases and the popup-command journey failed their document checks.
A source probe confirmed the wrong document URL. The popup acceptance path now
marks the view initialized before `request.openIn(view)`, after its existing
profile validation, so that pending model initialization cannot replace the
engine-owned document.

Qt 6.11.2's [WebEngineView setter](https://github.com/qt/qtwebengine/blob/v6.11.2/src/webenginequick/api/qquickwebengineview.cpp#L1123)
returns immediately for an empty URL. Its
[load-state notifications](https://github.com/qt/qtwebengine/blob/v6.11.2/src/webenginequick/api/qquickwebengineview.cpp#L644)
are posted through a zero-delay timer.
The diagnosis of the browser's collision comes from the native traces above.

The native graph and destination tests retain real loopback visits and require
their actual document URLs and durable journey records. The graph test also
rejects a phantom `about:blank` journey node. Failure diagnostics report
load-event URLs, current navigation state and records from the fixture's own
databases.

## Verification

The source-loaded debug runner passed all seven destination, graph and manager
checks with tracing, then passed all seven again without tracing in 13.762
seconds. The repaired trace had no startup blank-address load. This probe uses
the production Rust engine with source QML and is separate from compiled-resource
qualification; the normal native resource guard remains intact.

The popup probe first reproduced an actual `about:blank` document, then passed
alongside both destinations, the graph and manager forms: **8 source-mode
checks** in 15.886 seconds. Compiled popup qualification is recorded below.

The complete QML suite passed **869 checks**, with no failures or skips, in
194.993 seconds with the popup handoff included. Its log contains no runtime
type, reference, binding-loop or assignment errors. Lint has no
critical/error/fatal diagnostics.

The final release build passed in **8m 15s**. Both native test programs were
relinked against that archive. All four compiled native gates passed:

- `application_navigation_usability`: **14 checks**, 25.936 seconds.
- `application_switcher`: **36 checks**, 114.596 seconds.
- `application_command_editing_wayland`: **12 checks**, 16.470 seconds.
- `application_keyboard_chrome_wayland`: **18 checks**, 70.240 seconds.

These cover both journey destinations, relationship navigation, native
blank/start history, secondary-window back/forward and manager keyboard forms.
The graph's placeholder-history assertion passed. The resource guard compared
thirteen source/compiled chrome resources, including both window components.

The additional workflow run passed **21 checks** in 71.287 seconds: manager
feedback, secondary managers, regular/private libraries, all eleven new-window
cases (including five popup variants), popup commands, session preview and both
startup link-confirmation outcomes. The focused popup run also passed six
checks in 18.599 seconds. The four gates and workflow run total **101 native
checks**, with no failures or skips. Their logs contain no runtime type,
reference, binding-loop or assignment errors.

Artifacts are in `/tmp/ferric-manager-keyboard-20261021/`.
