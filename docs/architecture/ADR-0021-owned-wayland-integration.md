# ADR-0021: Ferric-owned Wayland window integration

- Date: 2026-11-04
- Status: migration decision; production adoption gated on native lifecycle proof
- Supersedes: the KWindowSystem export/tag implementation choice in ADR-0004
- Qualified SDK: Qt 6.11.2, including matching Qt Gui private headers

Ferric will own small clients for freedesktop `xdg-foreign-v2` and
`xdg-toplevel-tag-v1`. They use Qt's existing connection, registry discovery,
event processing and native windows. No second connection, round trips,
polling, dispatch thread or periodic maintenance is permitted. Required KDE
components, including fetched sources, static libraries and dynamically loaded
plugins, are prohibited in the completed migration. Introducing a KDE dependency
after migration requires explicit user approval.

ADR-0004's browser request authority, pinned portal service owner, cancellation,
late-reply cleanup, URI policy and fallback rules remain requirements. The
chooser retains its five-second export deadline. Once a request may have reached
the portal, an uncertain outcome must never cause a second chooser to open.

One application context discovers both globals using Qt Wayland extensions.
One lifecycle observer per QWindow tracks Qt's actual surface and toplevel role.
The logical tag and preparation survive native recreation. Each new valid pair
gets a never-reused nonzero process-local u64 generation. Either object's
destruction synchronously invalidates native eligibility and the generation's
shared export. Cancelling a consumer detaches only that consumer. Pending work
cannot acquire authority over a replacement generation.

The serializable compositor identity remains tag, application ID and PID.
Generation is a separate internal witness. Every participant in asynchronous
group work must retain and revalidate its witness immediately before dispatch,
in addition to existing Core ownership and compositor corroboration. A submitted
tag is not compositor acknowledgement.

The sole native-window adapter includes Qt Gui's private
`qpa/qplatformwindow_p.h`, accessing `QNativeInterface::Private::QWaylandWindow`.
Ferric accepts responsibility for this private interface. Future SDK
qualification must verify matching header provenance, real pointer access,
direct lifecycle signal timing, tag-before-commit ordering, invalidation and
teardown. The existing QtWebEngine 6.11.2 compatibility guard remains in force.
Failure of this native proof stops dependent production migration; changing SDK,
adding another connection or weakening supported behaviour is not an escape.

Protocol XML is unmodified and pinned with upstream revision, checksums and
license notices in `crates/ferric-browser-engine-qt/protocols/provenance.json`.
Cargo and CMake use the same generation recipe. Outputs belong only in build
directories, with one protocol implementation per final executable.

The application explicitly retires window protocol objects and then globals
before Qt destroys its connection. Native pointers and protocol calls stay on
the GUI thread; workers receive immutable scalar witnesses only.

Historical qualification schemas and accepted KF6 receipts remain intact.
The completed implementation requires a separate versioned successor schema;
it must not masquerade as successful historical KF6 qualification.
