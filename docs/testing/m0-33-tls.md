# M0-33 TLS certificate errors

Updated: 2026-10-15

Status: in-progress

Requirement: AUTH-004. The request lifecycle lives in
`FerricBrowserRuntimeRequests.qml`; native presentation lives in
`FerricCertificatePrompts.qml`. URL normalization remains in the Rust adapter.

Certificate failures block by default. Non-overridable errors, subresources,
protected Google hosts and unavailable origins have no bypass surface.
An engine-marked overridable main-frame error for another host shows its full
canonical origin and bounded, literal error description in the requesting window.
The actual Qt request is deferred while the decision is pending; Enter initially
chooses Go back and Escape rejects the request.

Continue for this session invokes Qt's `acceptCertificate()`. Qt retains
host/certificate/error allowances in memory and may reuse them during the browser
session. The prompt states this scope; it no longer promises a request-only
exception. The browser writes no persistent certificate exception and does not
change the system trust store or introduce a global ignore flag. See
[Qt's decision API](https://doc.qt.io/qt-6.11/qml-webenginecertificateerror.html) and
[its in-memory certificate policy](https://raw.githubusercontent.com/qt/qtwebengine/6.11/src/core/net/ssl_host_state_delegate_qt.cpp).

Origins exclude credentials, paths, queries and fragments. Client-certificate
requests use a separate adapter for Qt's scheme-less TLS authority; page URLs
never use that transport-specific interpretation.

## Current evidence

The 2026-10-15 debug offscreen production runner passes all six local HTTPS
cases: cancellation and explicit continuation in primary, secondary and popup
windows. A disposable QSslServer uses a newly generated self-signed certificate;
no trust-store setup or external site is involved. Continuation also verifies a
second navigation using the session allowance. Cancellation sends no HTTP
request. Separate request guards cover protected hosts, non-overridable errors,
subresources and unavailable origins. Full integration records live in
`docs/implementation/product-polish-2026-10-15.md`.

These checks do not establish the complete TLS release gate. Real HSTS,
mixed-content, OS trust-store interoperability and supported Wayland/compositor
qualification remain distinct work. Client-certificate selection and physical
security-key authentication also need their own real-credential evidence.
