# ADR-0016: Qualification-gated native double-click

## Status

Implemented as an explicit Grid request, but disabled in the runtime pending
native Wayland qualification. No default-mode binding or general browser
compatibility claim is enabled.

## Decision

`FerricPagePointerAdapter` remains the only native pointer event boundary. Its
`double` action sends one bounded request using Qt's documented
press-release-press-double-click-release sequence. It keeps the captured view
and surface checks, request-ID deduplication, and a single terminal
acknowledgement. The action never falls back to DOM `click()` and never retries
after rejection or uncertainty.

Grid exposes the explicit `Alt+Enter` chord as `grid-double-click`. It is
available only while the local Grid session owns the active view; aliases,
CLI, IPC, macros, userscripts, repeat, and the generic action executor cannot
create that local authority. Normal-mode `Alt+Enter` remains untouched. The
runtime's `doubleClickEnabled` gate is currently false, so the adapter reports
`unavailable` without sending events. The Grid session terminates the request
and tells the user that native qualification is pending.

When a platform-specific qualification enables the gate, the adapter sends one
Qt transaction. Its WebEngine behavior is two DOM click events followed by one
`dblclick`; page handlers may act on both clicks. Ferric does not route either
click through its URL-opening path. The action means a native double-click
attempt, not exactly-once page activation.

## Evidence and limitations

On 2026-10-12, the offscreen Qt 6.11.2 / QtWebEngine 6.11.2 fixture exercised
the production adapter. It observed this DOM order:

`pointerdown, mousedown, pointerup, mouseup, click(detail=1), pointerdown, mousedown, pointerup, mouseup, click(detail=2), dblclick(detail=2)`.

All recorded events had `isTrusted=true` and active transient user activation.
The fixture observed two click handlers, one double-click handler, an
unchanged document URL, one delivered acknowledgement, and rejection of a
duplicate request ID. These are fixture observations for this Qt/WebEngine
build; they do not imply that fullscreen, popup, clipboard, file chooser, or
other activation-gated APIs will be approved.

The required native Wayland run was not available: the existing nested Weston
runner lacks the virtual-keyboard protocol used by its input helper. The
runtime gate therefore remains false. In-place page reactions between native
events remain governed by Qt/WebEngine's event path and are not an atomic DOM
transaction.

Qt documents `QMouseEvent`'s `MouseButtonDblClick` event and the `QQuickWindow`
double-click handler as public APIs. Qt Quick Test defines the observed event
sequence as press-release-press-double-click-release:

- [QMouseEvent](https://doc.qt.io/qt-6/qmouseevent.html)
- [QQuickWindow](https://doc.qt.io/qt-6/qquickwindow.html)
- [Qt Quick TestCase mouseDoubleClickSequence](https://doc.qt.io/qt-6/qml-qttest-testcase.html)
