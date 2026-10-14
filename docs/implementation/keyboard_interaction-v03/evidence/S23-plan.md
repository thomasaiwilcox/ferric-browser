# S23 frame-provider research plan

## Question

Can the supported Qt WebEngine public frame API collect a bounded result from
an independently selected cross-origin child, retain exact frame/document
freshness, and map a child semantic target to the top-level view without a
parent-world bypass or page bridge?

## Scope

The research uses the public `QWebEngineFrame` / QML `webEngineFrame` API in
an opt-in fixture. A local parent page contains same-origin, cross-origin,
opaque sandboxed, duplicate-metadata, navigated, and removable child frames.
The probe does not change Ferric runtime code and never prints page text or
labels. It returns only bounded booleans and counts.

The project package manifest declares Qt 6.11.2 as its minimum. Qt documents
`QWebEngineFrame` and `WebEngineView.mainFrame` from Qt 6.8, while QML child
enumeration is available from Qt WebEngine 6.10. The installed Qt 6.11.2
target is above those gates. This investigation compiles and runs on that
declared target; it does not claim a test on unsupported older Qt versions.

## Evidence

- T052 static inspection checks the package minimum, public headers, QML API
  gate, C++ compilation, result type, geometry fields, and frame lifecycle
  signals.
- T052 WebEngine fixture probes cross-origin and sandbox execution in
  `ApplicationWorld`, same-origin separation, bounded return data, document
  navigation, duplicate frame metadata/geometry, and destruction validity.
- T035 reruns the existing WebEngine exterior/closed-root cases. Native
  Wayland input is attempted separately; startup alone is not treated as
  target/input qualification.

## Decision rule

Public frame execution is only provider feasibility. A production semantic
action requires an unambiguous parent-element/frame mapping, a current
document identity, bounded data, and usable top-level geometry. If the public
API cannot establish those together, preserve parent isolation and the
cross-origin surface/manual Grid fallback.
