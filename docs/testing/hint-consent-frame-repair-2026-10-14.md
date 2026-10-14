# Consent-frame hint repair — 2026-10-14

Phoronix's consent buttons live in a Sourcepoint iframe at a different origin.
The parent collector could label the exterior but could not access its controls.
Frame-local collection found all three buttons; the missing contract was exact
frame association, projection and activation routing.

## Implemented behavior

- Already-focused verified frames contribute semantic targets. Unfocused frames
  offer **Hint inside frame**, which explicitly moves focus and starts an all-target,
  current-action session inside the selected frame. Ordinary region scopes retain
  their focus-preserving contract.
- Public Qt frame handles are associated with retained parent iframe elements
  through unique document focus. Duplicate names, URLs, sizes and enumeration
  order do not select a receiver. Parent DOM origin isolation stays intact.
- A browser-owned frame/document lease namespaces IDs and label reservations.
  Every script checks its isolated-world document epoch and live native ancestry.
  Collection and activation validate focus, clipping and supported projection.
- Positive axis-aligned mappings include borders, padding, nesting, zoom and
  viewport clipping. Unsupported transforms, stale documents, covered controls,
  redirected focus and sandbox script restrictions fail closed.
- Hint badges preserve page focus; the existing window key router consumes hint
  gestures. Explicit confirmation freezes the displayed selection and rejects a
  pending collection result before freshly validating its retained receiver.
- Work stays bounded: eight frame levels, 64 native handles/leases, 128 pending
  operations, one-second frame-operation deadlines, and shared 50,000-inspection /
  5,000-candidate limits. Cancellation discards late results. An acknowledged action
  may dismiss its frame; unknown outcomes never trigger a retry.
- Independently collected child targets support validated URLs, exact focus,
  semantic activation and nested frame entry. Native pointer and region/spatial
  scope actions on child targets remain unavailable. Parent-owned Grid fallback
  remains supported.

## Qualification

Evidence lives in ignored `target/hint-cookie-dialog/`.

| Check | Result | Evidence |
| --- | --- | --- |
| Final embedded resource parity | 84 resources match source bytes and hashes | `application-installable.log` |
| Final production frame checks | 29 passed, including zoom/resize rejection and initialization/cleanup | `application-installable.log` |
| Full production application suite before final viewport refinement | 89 passed; two opt-in lifetime cases skipped | `application-all.log` |
| DPR 2 frame and Grid journeys | 20 passed | `application-dpr2.log` |
| Live Phoronix, disposable profiles | Accept/Reject All dismiss; Manage My Choices opens hintable preferences; 5 passed | `live-installable.log` |
| Native input adapter suite | Passed | `native-qualified.log` |
| Rust core ownership/label checks | 148 passed | `core-qualified.log` |
| Repository gate | 900 Rust tests passed; two opt-in cases ignored; QML lint, architecture, docs and release checks passed | `xtask-green.log` |
| Strict Clippy | Passed for workspace/all targets | `clippy-qualified.log` |
| Final QML/WebEngine scripts | 192 passed | `qml-installable.log` |
| Lifetime | 1,000 frame cycles and 1,000 standard cycles passed; native leases cleared each cycle; Qt objects 3,387 → 3,387; maps/observers empty | `lifetime-complete.log` |
| Common-500 collection p95, 80/100/125% | 111.51 / 102.62 / 92.89 ms | `performance-idle.log` |
| Common-500 overlay p95, 80/100/125% | 188.55 / 171.73 / 125.66 ms; all below 200 ms | `performance-summary.json` |
| Pack integrity/evidence records | Passed; this is record validation, not broad implementation completion | `pack-qualified.json` |

The repository gate, strict Clippy, full application suite and DPR 2 journeys
precede the final viewport-stamp and aggregate-depth refinements. The final
release build, resource parity, focused production frame checks, QML scripts,
live consent journeys, lifetime runs and performance measurements cover those
refinements. These results do not claim a fresh full-suite run on the final tree.

The frame fixtures cover all three consent choices at 80/100/125% zoom,
already-focused entry, duplicate metadata, nesting, positive scaling and an opaque
sandbox with script permission. Each choice activates exactly once and hint keys
never reach the page. Navigation, replacement, removal, occlusion, focus theft,
rotation, zoom/resize, disallowed sandbox scripts, redirection, timeout and late completion
are rejected.

The first combined stress/benchmark/build run exceeded
[QtTest's five-minute watchdog](https://doc.qt.io/qt-6.11/qtest-overview.html#increasing-test-function-timeout).
Its SIGABRT was the test watchdog (confirmed by coredump metadata and QtTest's
fatal timeout), with no OOM evidence. The finite 1,000-frame-cycle workload also
outlasts that default limit on an idle host, so its opt-in CTest now sets a bounded
ten-minute QtTest limit. The completed frame and standard runs took 361.6 seconds
together. Performance numbers from the contended run are not qualification evidence. Earlier source-routing probes and
failed setup iterations are retained separately and are not represented as passes.
Existing Qt teardown-context warnings occur in the application harness.

## Installation

Installed atomically at `/home/tom/.local/bin/ferric-browser`, together with
matching user guides, desktop entry and icon (13 files checked by SHA-256).

Binary SHA-256:
`1622eb8e84c690afe637ba845c1f89ab79c2159b4a80040b197f759738b0cf84`.

`installation.json` records the previous hashes and unchanged browser/HTTP/HTTPS
handler defaults. The previous executable and resources are backed up under
`target/hint-cookie-dialog/previous-installation/`. Installed help, diagnostics,
private offscreen startup/IPC and nested Wayland startup/IPC checks pass. The
nested compositor check does not qualify physical Wayland input.

Existing user sessions and consent choices remain untouched. A running old
browser must restart to use the new embedded resources. On an unfocused consent
frame, type its hint and select **Hint inside frame**, then type a button's hint.
No remote changes were published during implementation and installation.
