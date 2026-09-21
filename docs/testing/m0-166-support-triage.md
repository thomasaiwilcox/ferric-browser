# M0-166 support and triage evidence

Status: in progress, implementation slice recorded 2026-09-18.

`docs/SUPPORT.md` and the public bug template request the bounded environment,
reproduction, fresh-temporary-profile result, expected/actual behavior, and
sanitized diagnostics required by SUPPORT-001. They explicitly reject profile,
cookie, credential, page-content, and unredacted-log uploads. `SECURITY.md`
keeps vulnerability reporting out of public issues until a private route is
actually configured.

The support taxonomy separates core, Qt adapter, upstream engine,
desktop/portal, graphics-driver, packaging, site-policy, and
user-configuration failures. Workarounds must state scope and tradeoffs, and
external escalation requires authorization and a minimal reproduction.

The remaining release task is configuring and publishing the project owner's
private security channel; this source tree does not invent a contact address.
