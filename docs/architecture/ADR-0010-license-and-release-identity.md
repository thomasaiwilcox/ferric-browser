# ADR-0010: License and release identity

- Status: accepted
- Date: 2026-09-18
- Scope: application source license, public name, application ID, and release
  signing identity

## Context

The development specification recommends GPL-3.0-or-later for the initial
application license. The project is adopting that license for its public
pre-alpha source and binary distributions.

Ferric Browser is an independent implementation. It may learn from documented
qutebrowser behavior, but must not copy qutebrowser source or branding without
preserving compatible notices and obligations. QtWebEngine/Chromium, fonts,
icons, dictionaries, blocklists, and scripts have separate notice and
redistribution obligations.

## Decision

Use `GPL-3.0-or-later` for all workspace crates and distribution artifacts.
Ship the complete license text as `LICENSE`. Keep dependency and asset notices
in the release inventory.

## Decision gate

The public identity is Ferric Browser, repository `ferric-browser`, executable
`ferric-browser`, and desktop ID `io.github.ferricbrowser.FerricBrowser`.
Publication remains blocked until the `ferricbrowser` namespace and final name
review are complete. No maintainer signature key is claimed by this ADR.

## Consequences

The project is copyleft and may be distributed as a public pre-alpha once the
remaining namespace, security, qualification, and packaging gates pass. This
decision is not trademark clearance.
