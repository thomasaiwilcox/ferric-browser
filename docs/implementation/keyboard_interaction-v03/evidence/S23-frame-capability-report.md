# S23 independent child-frame capability report

**Decision: no-go for general cross-origin semantic targets in the current
production path.** The public provider can execute isolated code in an exact
child frame and return bounded plain data, including from cross-origin and
opaque sandboxed frames. The missing piece is a safe mapping from that native
frame handle to the parent-visible iframe rectangle. The API exposes size and
identity but no position; its child list has arbitrary order. The fixture
proves that two frames can share every exposed association field while their
outer rectangles differ. No child semantic target can be safely projected or
activated from that evidence.

## Supported Qt gate

[`packaging/dependencies.toml`](../../../../packaging/dependencies.toml)
declares Qt 6.11.2 as the current package minimum. The Qt 6.11.2 host reports
Qt6Core and Qt6WebEngineCore 6.11.2. The public C++ API compile and QML runtime
probe passed on that target. Qt documents `QWebEngineFrame` and the
`WebEngineView.mainFrame` property since Qt 6.8, and the QML
`webEngineFrame.children` property since Qt WebEngine 6.10. Therefore the
declared minimum covers the probed APIs. No older Qt version is a supported
target in the package manifest, and no cross-version runtime claim is made.

## Capability results

| Capability | Result |
| --- | --- |
| Isolated execution | `QWebEngineFrame.runJavaScript(script, worldId, callback)` compiled and executed in `ApplicationWorld` on exact cross-origin and sandbox frame handles. The parent page's `contentDocument` remained inaccessible for both; same-origin control access remained unchanged. Page-world globals were absent in ApplicationWorld. This is a browser-owned privileged provider boundary, not a page bridge. |
| Bounded data | The child query capped a 12-button fixture at four and returned booleans/counts only. Qt documents only plain serializable values for the callback. No page strings were returned or logged. |
| Frame identity | Public frame equality distinguishes the two live frame handles. The fixture changed a child's document while retaining its QWebEngineFrame handle; an ApplicationWorld document marker reset. On removal, the old handle became invalid and disappeared from a fresh `children` enumeration. |
| Parent isolation | Parent-world reads returned `null` for cross-origin and opaque sandbox `contentDocument`; the existing T035 regression suite also retained only the exterior surface. |
| Geometry correlation | **Unavailable for a general provider.** `webEngineFrame` exposes `size`, `url`, `name`, and `htmlName`, but no position. Qt specifies that `children` is in arbitrary order and `findFrameByName` may choose arbitrarily when names repeat. The fixture produced two distinct handles with equal URL/name/size and different parent iframe x-coordinates. From the child, `window.frameElement` was null across origin. The native provider has no supported one-to-one association to either parent rectangle. |
| Lifecycle observation | `isValid` safely rejects the captured handle after destruction. The installed public QWebEnginePage header has no frame-created/frame-destroyed signal, so live discovery requires fresh enumeration. Async callbacks are not a frame-destruction notification and must still be guarded against page teardown. |
| Sandbox and origin | The browser-owned provider could inspect the sandboxed child through its selected handle while the parent page remained origin-isolated. This confirms authority is elevated at the browser boundary and needs an explicit future capability policy. |

## Integration recommendation

Keep cross-origin frames as exterior surfaces in the current semantic collector.
Do not add parent DOM access, DevTools transport, private Qt APIs, guessed
iframe-array ordering, or a privileged page bridge. Preserve scoped/manual Grid
as the existing visual fallback. A future narrow adapter may be reconsidered
only if it can prove an exact iframe-element/frame-handle association and
fresh top-level geometry for ambiguous names, redirects, sandboxing, nested
frames, and document replacement. This probe does not establish that contract.

## Test evidence

- `logs/S23/frame-probe.log`: public C++ syntax compile and live Qt 6.11.2 QML fixture
  passed; cross-origin/sandbox world separation, bounded return data,
  navigation freshness, duplicate metadata, and destruction invalidation were
  asserted.
- `logs/S23/engine.log`: 442 Qt engine crate tests, 144 QML/WebEngine tests, and native
  CTest 1/1 passed. Existing T035 cross-origin exterior and closed-root tests
  passed.
- `logs/S23/wayland.log`: the bounded Weston startup smoke passed, but `wtype` could
  not access the virtual-keyboard protocol. T035 native Wayland input was not
  run.

The frame APIs and version gates are described in the Qt 6.11 documentation:
[QWebEngineFrame](https://doc.qt.io/qt-6/qwebengineframe.html),
[webEngineFrame QML value type](https://doc.qt.io/qt-6/qml-webengineframe.html),
[WebEngineView](https://doc.qt.io/qt-6.11/qml-qtwebengine-webengineview.html),
and [WebEngineScript](https://doc.qt.io/qt-6/qml-qtwebengine-webenginescript.html).
