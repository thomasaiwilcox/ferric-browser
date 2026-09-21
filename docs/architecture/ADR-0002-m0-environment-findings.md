# ADR-0002: Initial M0 environment findings

Status: accepted as a feasibility record; bridge selected for M0  
Date: 2026-09-15

## Findings

- The host reports Qt 6.11.2 through `qmake6`.
- `Qt6Core`, `Qt6Gui`, `Qt6Quick`, `Qt6WebEngineQuick`,
  `Qt6WebEngineCore`, and `Qt6DBus` are discoverable through pkg-config.
- An active Wayland session is present (`WAYLAND_DISPLAY=wayland-1`,
  `XDG_SESSION_TYPE=wayland`). The actual Qt QPA selected by a RustBrowser
  window is not yet verified.
- An initial offline probe could not resolve the CXX-Qt registry packages. A
  later dependency-resolution probe succeeded, and CXX-Qt 0.10.0 is now pinned
  and builds against the installed Qt distribution.

## Consequence

The GUI slice uses the pinned CXX-Qt bridge, while the Qt-independent core
remains the authoritative implementation path and its tests continue to run
offline. The bridge is build-qualified but still requires runtime, callback,
and lifetime evidence before the engine boundary can be marked complete.
