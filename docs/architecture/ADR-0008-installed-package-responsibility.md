# ADR-0008: Installed package and engine-update responsibility

- Status: accepted for the Arch V1 distribution boundary
- Date: 2026-09-18
- Scope: package ownership, runtime probing, and engine freshness

## Decision

The Arch system package owns RustBrowser's executable, QML/resource files,
desktop metadata, icon, and documented optional integrations. The system Qt 6
distribution owns QtWebEngine and Chromium security updates. RustBrowser
reports the exact runtime Qt/engine package facts and qualified/unqualified or
blocked state, but never runs a package manager, changes repositories, or
pretends that an application update includes an engine security patch.

## Consequences and evidence

Installed-package and clean-chroot qualification are required before release.
Optional portals, PipeWire, codecs, dictionaries, and external tools are
reported independently and do not become hidden compile-time assumptions.

- `packaging/PKGBUILD`
- `packaging/dependencies.toml`
- `crates/browser-engine-qt/src/diagnostics.rs`
- `docs/RELEASE_POLICY.md`
- `docs/testing/m0-21-arch-packaging.md`

## Reconsideration trigger

Revisit only after a maintained distribution security/behavior gap is
demonstrated and a replacement ownership plan is approved.
