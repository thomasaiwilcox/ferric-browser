# Source register and research limits

Prepared 24 October 2026. Ferric's default main ref was read through the connected GitHub tool; pinned source reads establish baseline `e4aa108f9d63c99c3b3e977c2061d6d8dde48568`. Full file blob identities and the limits of each read are in `control/source_manifest.json`. Repository code was not compiled or run while authoring. A local network clone attempt was unavailable; it was not substituted with an invented checkout. The pack's Python tools are separately exercised locally.

## Ferric anchors

Repository: https://github.com/thomasaiwilcox/ferric-browser
Pinned root: https://github.com/thomasaiwilcox/ferric-browser/tree/e4aa108f9d63c99c3b3e977c2061d6d8dde48568

Read paths include README, Cargo.toml, docs/ARCHITECTURE.md, packaging/dependencies.toml, .github/workflows/ci.yml, FerricPrimaryBrowserWindow.qml, FerricBrowserWindow.qml, FerricHintWorkspace.qml, FerricPageServices.qml, FerricProfileResourceRegistry.qml, FerricProfileResources.qml, native profile_lifetime.h, application/profile_session.rs, config/browser_config.rs, scripts/EngineRequests.js and the application CMake registration.

Indexed search during review sometimes returned older `a245360c...` source. Search located symbols, but only pinned fetches are baseline evidence. Existing Hyprland integration and old no-WebChannel invariants must be reread locally at execution; the old proposal retains their historical source map. Do not presume old line numbers or names still apply.

## Public API and transport references

- Qt 6.11 WebEngineView: https://doc.qt.io/qt-6.11/qml-qtwebengine-webengineview.html — profile, new-window request and channel world interfaces. Public unversioned pages may redirect to a newer snapshot; use the installed 6.11 API and SDK, not a future enum/property.
- Qt WebAuth UX request: https://doc.qt.io/qt-6.11/qwebenginewebauthuxrequest.html — native account/PIN/retry/cancel UX, not an external credential-completion/phone-QR transport API.
- Qt new-window request: https://doc.qt.io/qt-6.11/qml-qtwebengine-webenginenewwindowrequest.html — preserve the engine's request handoff and opener relationship.
- WebAuthn Level 3: https://www.w3.org/TR/webauthn-3/ — origin/RP, client data, native request and credential semantics. Resolve exact sections in the conformance table; implementation caps never authorize weakened origin checks.
- libwebauthn candidate: https://github.com/linux-credentials/libwebauthn/tree/9c0e5f78ac445d35dca05170efbb0afefc5a89ae — inspect and lock actual APIs. Pinned manifest declares package0.10.0, Rust1.88 and LGPL-2.1-or-later. Candidate status is source-reviewed, not certification of library or Ferric integration.
- Ohm reference: https://github.com/ijt/ohm — prior proposal inspected startup, PasskeyBroker and Rust helper. Use as comparative design input, not a blanket security endorsement or unlicensed code source. Its warm-daemon approach is distinct from a speculative spare. Any reused source must preserve license/provenance; don't transplant its page-token/custom-scheme trust model unchanged.
- Hyprland current dispatchers: https://wiki.hypr.land/configuring/core/dispatchers/ — verify installed-version/capability behaviour; documented group entry is not proof of a hypothetical exact address-to-address join API.
- Hyprland selectors: https://wiki.hypr.land/configuring/core/selectors/ — check actual target selector availability. A compositor ID still needs trustworthy correlation to the originating Qt page host.
- KWaylandExtras: https://api.kde.org/kwaylandextras.html — exportToplevel is a public window-parenting handle API (since6.28), not a Hyprland client address translator.

## Use of the original proposal

`reference/ORIGINAL_PROPOSAL.md` is retained unchanged for auditability. It was based on Ferric `af30f6...`; its claim that secondary hints are disabled is obsolete. This pack explicitly supersedes that claim and the old Runtime*.qml anchoring. Suggested settings/protocol/type/file names in this pack are implementation requirements or proposals, not assertions that they already exist in Ferric.

Any source or dependency movement during execution must be recorded in source-map.execution.json and the lockfile/review records. Never infer implementation from an upstream README alone. Tests cited as required here are acceptance specifications until agents bind them to real executable assertions.
