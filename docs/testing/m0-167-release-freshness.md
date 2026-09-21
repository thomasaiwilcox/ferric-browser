# M0-167 release freshness evidence

Status: in progress, implementation slice recorded 2026-09-20.

`docs/RELEASE_POLICY.md` defines the mandatory release-candidate review of Qt,
QtWebEngine, Chromium security notes/backports, Rust dependency advisories,
supported distribution packages, compatibility quirks, and capability inputs.
It requires an explicit mitigation or release block for applicable critical
engine vulnerabilities and forbids treating a base Chromium version as proof
of a security-patch level.

The same policy records proposed operational targets without claiming an SLA:
one working day for critical triage, evaluation when a usable package exists,
and a tested update within 72 hours. A real advisory review and staffed private
operational channel remain release responsibilities.

`cargo xtask check --locked --offline` now validates that this policy retains
the required advisory categories, mitigation/release-block language, honest
Chromium patch-status wording, and the explicit in-progress evidence marker.
This enforces the release contract without turning structural validation into
an advisory review.

## Development-host observation — 2026-09-19

This is an environment observation, not a release-candidate review or an
all-clear. The development host currently reports Qt packages `6.11.2-3`
(`qt6-base`), `6.11.2-1` (`qt6-webengine`, `qt6-declarative`, and
`qt6-wayland`), and Rust `1.98.0` from the active rustup toolchain. The Arch
package recipe's clean build and nested Wayland smoke test passed, but the
installed package and upgrade-path qualifications remain open.

One authoritative Qt advisory was checked: [CVE-2026-19248](https://www.qt.io/blog/security-advisory-cve-2026-19248)
is fixed in Qt 6.11.2 and later, so the observed Qt package versions are not
in its stated affected range. This does not establish that all Qt, QtWebEngine,
or Chromium advisories have been reviewed. The rebuilt development binary's
display-free diagnostics command did query the running QtWebEngine library and
reported Chromium base `140.0.7339.225` and security-patch API value
`151.0.7922.71`, both with runtime provenance. Qt documents that these are
separate values ([QtWebEngine overview](https://doc.qt.io/qt-6/qtwebengine-overview.html),
[runtime security-patch API](https://doc.qt.io/qt-6/qtwebenginecoreglobal.h)).
Those observed values are evidence for reporting, not an advisory conclusion.
The bridge accepts only bounded dotted numeric values; empty, control-bearing,
or malformed runtime strings become an explicit `unknown` fact rather than
being copied into diagnostics.

## Rust dependency advisory scan — 2026-09-20

`cargo-audit 0.22.2` scanned the checked-in `Cargo.lock` against RustSec
database commit `d5c17953a895cf19e8d3ce66eaa42b6fcfe1fb16`, updated
2026-09-19. The lockfile contains 144 dependencies; the scan found zero
vulnerabilities and no warnings, with no ignored advisories. The machine-readable
result was emitted by `cargo audit --json` and exited successfully. This closes
the RustSec portion of the gate for this exact lockfile. Qt/QtWebEngine,
Chromium patch, distribution-package, and maintainer-owned release reviews
remain separate obligations and are not inferred from this result.

The standalone `fuzz/Cargo.lock` was scanned with the same database and tool
on the same date: 145 dependencies, zero vulnerabilities, zero warnings, and
no ignored advisories. Both lockfile results are retained as release-review
evidence; the generated JSON files are temporary command output, not checked-in
claims.
