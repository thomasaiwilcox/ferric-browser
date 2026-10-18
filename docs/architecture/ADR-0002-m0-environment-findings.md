# ADR-0002: Initial M0 environment findings

Status: accepted as a feasibility record; bridge selected for M0  
Date: 2026-09-15

## Findings

- The host reports Qt 6.11.2 through `qmake6`.
- `Qt6Core`, `Qt6Gui`, `Qt6Quick`, `Qt6WebEngineQuick`,
  `Qt6WebEngineCore`, and `Qt6DBus` are discoverable through pkg-config.
- An active Wayland session is present (`WAYLAND_DISPLAY=wayland-1`,
  `XDG_SESSION_TYPE=wayland`). The actual Qt QPA selected by a Ferric Browser
  window is not yet verified.
- An initial offline probe could not resolve the CXX-Qt registry packages. A
  later dependency-resolution probe succeeded, and CXX-Qt 0.10.0 is now pinned
  and builds against the installed Qt distribution.

## Consequence

The GUI slice uses the pinned CXX-Qt bridge, while the Qt-independent core
remains the authoritative implementation path and its tests continue to run
offline. The bridge is build-qualified but still requires runtime, callback,
and lifetime evidence before the engine boundary can be marked complete.

## 2026-10-18: bounded nullable download-source compatibility exception

The production browser and native observer both reproduced a SIGSEGV when a
cancelled request delivered its binary response after its auxiliary Quick view
had been destroyed. The QtWebEngine 6.11.2
[`QQuickWebEngineDownloadRequest::view()` implementation](https://raw.githubusercontent.com/qt/qtwebengine/v6.11.2/src/webenginequick/api/qquickwebenginedownloadrequest.cpp)
dereferences a null source adapter despite the API's nullable-source contract.
The causal public regression waits for a second download offer and real GET
before releasing the cancelled response; immediate release can miss deferred
view destruction.

Public `QWebEngineDownloadRequest::page()` safely returns null, but also returns
null for live Quick clients, so it cannot resolve ownership. QObject property
access invokes the same faulty getter. Guessing the owner by URL or the active
window could attribute a private or secondary-window request incorrectly.
Retaining cancelled views indefinitely would retain abandoned work and does
not provide a bounded lifetime policy. Replacing or patching the system Qt
distribution is outside this adapter change.

The accepted exception is limited to `download_source.cpp`: check the source
adapter for null through the matching installed private SDK declaration, then
call the public getter. The build requires SDK 6.11.2 and the adapter verifies
the running WebEngine version before accessing private state. A mismatch
disables download admission with update/rebuild guidance. The helper uses a
declared pointer-to-member for the protected scoped pointer; it does not
downcast request objects to a probe subclass or guess field offsets. It never
exposes a private page, changes owner/profile policy or permits private network,
authentication or transport APIs.

The installed private header depends on one interface header absent from the
SDK. Its unmodified upstream source, hash, copyright/SPDX notice, selected
LGPL-3.0-only license and full license text are recorded under
`crates/ferric-browser-engine-qt/src/qt_compat`. The package ships the license,
NOTICE and provenance, alongside the application's GPL license. Qt remains
dynamically linked and no Qt implementation is modified.

Qualification on the frozen guard checkpoint passes seven standalone native
checks (live/destroyed sources in regular/private profiles), 25 native action
feedback checks and both private-query variants of the causal public download
probe. These results qualify the crash boundary, not complete download behavior.
The complete native Downloads group still exposes before-header cancellation,
navigation and HTTP authentication defects. Responsive file finalization also
remains open.

Any SDK change requires reviewing this declaration/interface pair and repeating
native lifetime and causal production regressions. Remove the exception only
after qualifying an upstream getter that safely implements the nullable
contract. This exception does not relax the public-API rule elsewhere.
