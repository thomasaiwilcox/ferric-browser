# C04 — Credential authority and boundary proof

## Threat model

Treat the webpage, every iframe, message, option, page title and correlation ID as untrusted. The transport library and native helper are dependencies, not infallible authorities. Prevent origin/RP confusion, subframe escalation, cross-profile delivery, stale document completion, prompt spoofing, resource exhaustion and automatic duplicate registration.

Prototype a dedicated isolated-world mediator and credential-only per-view Qt WebChannel using public APIs. Do not expose BrowserUi or any generic object with storage, evaluation, command, clipboard, process or synthetic-input methods. A reserved world is not the ordinary ApplicationWorld and is unavailable to userscripts/jseval. Select its ID centrally after auditing actual current usage; valid range/documentation is not permission to reuse an occupied world.

The MainWorld shim may request only credential operations for its own realm. Its messages do not carry authoritative origin, topOrigin, crossOrigin, secure-context, profile, tab or result-destination fields. Require real engine-produced message delivery from the mediator's own top-level window; reject other-frame sources and synthetic dispatched messages. `isTrusted` on a message is NOT proof of a physical gesture. Same-origin scripts legitimately have the same page authority; do not pretend a nonce protects against scripts already authorized in that realm.

## Document binding

Phone provider is top-level HTTPS only, including a separately owned top-level auth popup. No iframe provider, opaque origin, about/data/file/internal page or insecure production loopback exception. Browser-owned code in the actual main frame establishes its security origin, secure-context and applicable permissions-policy facts. Native policy corroborates exact view, profile, document/renderer generation and readiness. A URL alone is not security-origin proof.

Use an isolated-document nonce and a native generation-bound handshake. Keep native capabilities/nonces out of DOM/MainWorld and logs. Do not interpolate untrusted strings into executable script; pass bounded serialized data. Before admission and completion, challenge the current main-frame mediator in the same reserved world and require the captured document lease. Same-URL reload, cross-document history/BFCache restoration, discarded renderer and view replacement invalidate it. Prove safe same-document SPA handling separately; fail closed when identity is uncertain.

WebChannel alone supplies no caller-frame proof. Merely excluding subframe injection or reading the host URL does not satisfy this contract. Probe malicious frame postMessage, window.name changes, document.domain variants, object/prototype replacement, rapid same-origin navigation and old queued invocations.

## Hard gate

No real credential provider is wired until an independent reviewer accepts the production-Qt proof. Review/replace the old blanket no-WebChannel source guard with tighter executable isolation tests only through the explicit scoped ADR. If public APIs cannot supply a missing fact, record the failed primitive and keep phone disabled; do not substitute a MainWorld native object, console bridge or broad CSP-bypassing scheme. Helper/pure-policy work can continue independently.
