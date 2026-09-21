# Release freshness and maintenance policy

This document defines the review evidence required before publishing a
Ferric Browser release. It is a project process, not a claim that this
unstaffed development tree provides an operational security SLA.

## Freshness review

For every release candidate, maintainers must record the review date and
review the following sources against the exact supported distribution package:

1. Qt and QtWebEngine release/security advisories;
2. Chromium security notes and the QtWebEngine backport/patch model;
3. Rust dependency updates from the locked graph and their advisories;
4. supported distribution package updates and known-bad ranges;
5. the current `packaging/engine-version-policy.toml`, compatibility quirks,
   and capability inputs.

The release record must identify applicable critical vulnerabilities and the
mitigation or explicit release block for each. A Chromium base version alone
does not establish a security-patch level. The browser must not knowingly ship
an applicable, unmitigated critical engine vulnerability.

The review must be completed by a maintainer with access to the authoritative
advisory sources. Offline repository tests and Rust dependency scanners do not
constitute an engine-security review.

## Operational targets

These are proposed maintenance targets once maintainers and release
infrastructure exist; they are not an SLA for the current project:

- triage a critical security report within one working day;
- evaluate an upstream critical fix as soon as a usable supported package is
  available;
- target a tested update within 72 hours of that package becoming available.

The project should publish current support status and record inability to meet
a target rather than leave stale security claims in place. The browser never
updates itself, runs a package manager, changes repositories, or silently
replaces its engine.

## Release record

Each release record should include the review date, supported package/version,
advisory sources checked, unresolved risks, decision owner, and links to the
qualification and compatibility reports. Do not invent signatures, maintainer
identities, or advisory conclusions when the evidence is unavailable.
