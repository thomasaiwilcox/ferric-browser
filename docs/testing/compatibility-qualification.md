# Compatibility qualification report

This is the versioned report envelope for the Google and media qualification
unit in `DEVELOPMENT_SPEC.md`. It is deliberately separate from automated CI:
the live scenarios require an authorized disposable account, real portals,
and explicit external-effect review. `cargo xtask test compatibility` checks
that the report cannot omit a required scenario or use an unrecognized status;
it does not turn `not-run` into a pass.

Latest structural validation: 2026-09-19 — `cargo xtask test compatibility`
passed. Live scenarios remain `not-run` until their authorized evidence is
recorded below.

Application commit/package: not recorded

Qt build and Chromium security-patch level: Qt 6.11.2; Chromium base
140.0.7339.225; Chromium security-patch API value 151.0.7922.71; runtime
values recorded by `ferric-browser diagnostics --format json` on 2026-09-19.
These values do not replace the advisory review.

Distribution/compositor/GPU baseline: not recorded

Qualification date: not recorded

Credential and external-effects policy: no real credentials or external
messages are used by automated tests; live runs require an authorized,
disposable test account and recipient/meeting.

| Scenario | Status | Evidence or blocker |
| --- | --- | --- |
| G-01 | not-run | Authorized account run required |
| G-02 | not-run | Same-profile restart run required |
| G-03 | not-run | Two-profile isolation run required |
| G-04 | not-run | Disposable Gmail workflow required |
| G-05 | not-run | Disposable Calendar workflow required |
| G-06 | not-run | Disposable Drive workflow required |
| G-07 | not-run | Disposable Docs workflow required |
| G-08 | not-run | Disposable Sheets workflow required |
| G-09 | not-run | Two-participant Meet device run required |
| G-10 | not-run | Real screen-share portal run required |
| G-11 | not-run | Real window-share portal run required |
| G-12 | not-run | 60-minute Meet resilience run required |
| G-13 | not-run | Qualified media decode run required |
| G-14 | not-run | Notification lifecycle run required |
| G-15 | not-run | Authorized FIDO2 run required |
| G-16 | not-run | Disposable editor recovery run required |
| G-17 | not-run | OAuth-style and authorized popup run required |
