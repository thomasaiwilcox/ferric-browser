# ADR-0010: License and release identity

- Status: proposed, owner decision required before public publication
- Date: 2026-09-18
- Scope: application source license, public name, application ID, and release
  signing identity

## Context

The development specification recommends GPL-3.0-or-later for the initial
application license, while the current Cargo metadata remains the provisional
`MIT OR Apache-2.0` declaration. The repository has not selected a final
license or established a maintainer signing identity. Publishing either by
assumption would create an inaccurate legal or provenance claim.

RustBrowser is an independent implementation. It may learn from documented
qutebrowser behavior, but must not copy qutebrowser source or branding without
preserving compatible notices and obligations. QtWebEngine/Chromium, fonts,
icons, dictionaries, blocklists, and scripts have separate notice and
redistribution obligations.

## Options

1. Select GPL-3.0-or-later, as recommended by the specification.
2. Select another compatible application license after owner/legal review.
3. Do not publish until the owner has recorded the choice and checked the
   final project namespace and desktop identifiers.

## Decision gate

The owner must choose one application license, record it here, add the real
license text and notices, reconcile `[workspace.package].license`, and verify
dependency/asset obligations before public V1. The owner must also record the
owned public name, application ID, repository identity, and any managed
signature key. No invented maintainer key or signature is permitted.

## Consequences

Until the gate is closed, this tree may be built and tested but must not claim
to be a stable public release. The release artifact inventory and provenance
documents remain explicit about the pending choice.
