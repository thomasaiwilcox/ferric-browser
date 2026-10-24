# C05 — WebAuthn shim, native provider and truthful capabilities

Intercept only supported explicit publicKey get/create calls, and only when the phone feature is enabled for the current qualified document. Capture native function references before page scripts modify them; preserve the correct receiver. Non-public-key calls use the original implementation. Conditional mediation, subframes and unsupported contexts retain native behaviour; do not advertise phone availability there.

Snapshot/validate publicKey options without losing BufferSource offsets, raw bytes, omitted versus null fields, algorithm order, timeout or AbortSignal. Retain the original semantic native call and its signal, not a lossy phone JSON reconstruction. Malformed or oversized data produces a defined compatible error, not a silent fallback or endless promise. Reject page-supplied authority fields at the internal boundary.

Use a browser-owned provider choice before starting a bridge-managed ceremony. Never race phone and native authenticators. When native is chosen, call the preserved native path without manufacturing user activation or trusted events. Prove the asynchronous chooser preserves valid native behaviour; if not, redesign the choice before shipping rather than weakening WebAuthn. Requests requiring a platform authenticator cannot be silently satisfied by a cross-platform phone.

Keep challenge, effective RP ID, allow/exclude credentials, user handle, algorithms, UV/resident-key requirements and supported extensions. Build an explicit conformance table for known extensions: supported and verified, optional ignored with no success claim, native-only, or rejected as required by the request. No blanket copied stripping of appid/appidExclude or misreporting that unsupported extensions succeeded. Related-origin expansion is excluded.

Return the expected public-key credential/response object behaviour: binary fields, id/rawId, type, attachment where defined, getClientExtensionResults, toJSON, assertion/attestation-specific methods and appropriate prototype behaviour. Test ordinary site libraries which read or patch response methods. Do not claim full DOM/WebAuthn conformance from instanceof alone. Registration and assertions must verify independently, not just deserialize.

Capability probes settle promptly and truthfully. Phone availability does not mean a local platform authenticator or conditional autofill exists. Preserve unaffected native answers; do not blanket-stub every capability or cancel every Qt native request. Unsupported configurations must still leave normal password/native-key options available when the site supports them.

AbortSignal, rejected selection and helper results settle the owning promise exactly once. Registration after a possibly completed device operation must never be automatically recreated through a fallback provider. Browser-facing error category may explain uncertainty without exposing secret/options/error dumps to the page.
