# ADR: Public Qt Widgets page-host feasibility

## Status

Diagnostic prototype only. No production factory or product integration is authorized by this result.

## Context

The current embedded browser uses Qt Quick composition. Qt WebEngine's public popup adoption API accepts a `QWebEnginePage`; this prototype checks whether a Widgets-owned page/view host can preserve nested popup document state and opener behavior after its source host closes. The R02 prototype established the basic public API route, while F02e0 adds profile, failure, close, chrome, dialog, and interaction checks.

## Prototype boundary

The standalone CTest project owns its disposable loopback server, temporary profile directories, Widgets shells, `QWebEnginePage` objects, and `QQuickWidget` chrome. It makes no product bridge, factory, profile registry, or credential changes. Each result must name Qt 6.11.2 and distinguish QtTest-generated events from compositor-delivered input.

## Required observations

The lifecycle case records page identity and JavaScript nonces before and after adoption, closes the source through normal `RequestClose` handling, checks a retained `WindowProxy.closed`, and verifies that a sentinel navigation from that closed proxy did not reach the fixture server. It also exercises nested adoption, survivor pointer input, close order, failed adoption, and final page/profile drain. Separate cases check persistent/off-record cookie isolation, rejection of cross-profile adoption, dialog parent ownership, QML chrome focus, and a native input attempt.

## Decision rule

Public ownership is a candidate architecture only if all required lifecycle and isolation observations pass on the pinned SDK. A native window/input failure or missing assertion remains a qualification gap; synthetic Qt events do not stand in for compositor input. This prototype does not establish production routing, credential behavior, cross-platform support, or physical input qualification.
