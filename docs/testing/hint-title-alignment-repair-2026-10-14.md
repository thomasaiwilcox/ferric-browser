# Scrolled article hint alignment repair — 2026-10-14

The user reported that typing a hint displayed near an article title on the
scrolled Reddit homepage opened its subreddit instead of the article/comments.
The running executable matched the previous installation's SHA-256, so this was
a defect in the qualified build. This follows the
[earlier hint quality repair](hint-quality-repair-2026-10-14.md).
Product source is `d5ada2e641157da0c13342cd2be75d2bb552a826`.

## Reproduction and causes

Two independent defects combined to make the destination hard to infer:

- Reddit exposes a transparent full-card post anchor over a visible title anchor
  with the same resolved URL. The collector required an exact pointer receiver
  for every link, so it omitted the title and placed a post hint at the card's
  corner near the subreddit. Some titles are nested inside an extra wrapper.
- Badge avoidance indexed only controls at most 160 × 80 pixels. Wide title
  fragments were unprotected. A deterministic fixture moved a community badge
  from its target at `(40, 60, 140, 16)` to `(40, 80, 32, 22)`, directly over the
  separate title at `(40, 80, 400, 35)`. Cached placement also bypassed avoidance.

Before the fix, title discovery and badge association regressions failed at
three scales. A new production application regression, linked to the previous
embedded resources, failed at 80%, 100% and 125% page zoom because the title had
no hint. The preceding release tests did not cover this combined scrolled-card
journey or assert its final navigation destination.

## Repair and boundaries

URL hints admit a visible title beneath a larger transparent anchor only when
the receiver's local component contains that title, their resolved URLs match
exactly, and the receiver covers the clipped title rectangle. The receiver must
be a direct anchor hit, enabled and non-inert, with no background image, opaque
background or generated pseudo-content. A body/document container, coincident
equal-size visible box, separate dialog, unrelated destination or hidden title
is rejected.
This is a generic structural rule; it contains no Reddit selector or URL rule.

The retained title keeps its own element ID, document, frame, name and URL.
Freshness applies the same eligibility rule and rejects destination changes and
stale identities. No URL equality substitutes another element at dispatch.
Native pointer preflight and semantic DOM click still require the selected
link's exact pointer visibility. Supported frame mapping and native transform,
surface, scope, configuration and gesture checks remain in force. Script facade
version is 20; page interaction version is 3.

Badge avoidance indexes all visible target fragments. Containing card/region
rectangles do not block their own children. Nearby candidates can move along
the owning fragment and stay on it or within a four-scaled-pixel gap, allowing
metadata badges to avoid a neighboring title. Cached placements are retained
only while they stay near their resized fragments and avoid other controls.
Every distinct target retains its label;
unavoidable actual badge collisions still use selectable stacks. Edge leaders
connect to the nearest interior point, avoiding long lines across a card.

## Qualification

Evidence is in ignored `target/hint-title-alignment/`. Qt is 6.11.2 with Chromium
140. Tests use private/offscreen fixtures and ApplicationWorld scripts. The live
probe saves structural tags, slot roles, geometry and destination categories;
it does not save feed text, account data or article URLs.

| Check | Result | Evidence |
| --- | --- | --- |
| Regressions against previous behavior | Expected failures at three zooms/scales | `regression-red.log`, `application-red.log`, `title-layout-red.log` |
| QML/WebEngine functional suite | 192 passed, no failures | `qml-qualified.log` |
| Native interaction suite | 94 passed, no failures | `native.log` |
| Anonymous live Reddit homepage | Every eligible visible title collected; no community badges over titles at scroll 0, 400 and 800 | `reddit-qualified.log` |
| Production application | 65 unique checks passed; opt-in lifetime checked separately | `application-initial.log`, `application-navigation.log`, `application-summary.json` |
| DPR 2 application geometry and title navigation | 8 passed, no failures | `application-dpr2.log` |
| 1,000 production hint cycles | 3 passed; QObject count 3,384 → 3,384; tracking/maps/observers cleared | `lifetime.log` |
| Full repository gate | Passed; 898 Rust tests, two ignored opt-in cases, QML lint, architecture, generated docs and release inputs | `xtask-qualified.log` |
| Strict workspace/all-targets Clippy | Passed | `clippy.log` |
| Release resource parity | All 83 embedded files match source bytes and manifest hashes | `application-navigation.log`, `resource-parity.json` |
| Formatting and implementation pack integrity | Passed | `pack-qualified.json` |
| Synthetic visual preview | Inspected; title outline/badge, separate community badge and short leader | `title-preview.png` |

The first new production test sampled `location.pathname` before asynchronous
navigation completed: the browser URL already showed the post, while the old
document still returned `/fixture.html`. Its 62 other checks passed. Correcting
the harness to wait for the loaded document's path made all three new zoom cases
pass. The focused rerun also rechecked embedded resource bytes. The application
summary consolidates unique case names across these runs; it does not represent
the initial failing run as an all-green suite. Existing Qt teardown-context
warnings remain as documented in the earlier report.

## Latency and measured tradeoff

The 500-target gate uses ten warmups and 100 samples on a quiet host, with no
compilation or other qualification jobs. Both stages remain below 200 ms p95.

| Stage | 80% zoom | 100% zoom | 125% zoom |
| --- | --- | --- | --- |
| Collector round trip p95 | 105.01 ms | 96.80 ms | 94.39 ms |
| Overlay time to visible p95 | 140.57 ms | 128.30 ms | 112.16 ms |

See `performance.log` and `performance-summary.json`. The unchanged heavy
1,000/5,000-target workloads were not remeasured for this follow-up.

A sequential comparison against `e847803` uses the same harness, fixture, fonts
and component at both revisions (`overlay-baseline.log`, `overlay-current.log`,
`overlay-comparison.json`). Overlay p95 changes from 120.60 to 137.31 ms at 80%,
109.39 to 113.94 ms at 100%, and 60.18 to 116.19 ms at 125%. The 80% and 125%
changes exceed the investigation threshold. The new avoidance, containment and
distance checks add work. At 125%, the new geometry also yields 63 badge
representatives, including 44 individual unclustered controls, versus 20
representatives and no individual controls previously. That requires 43 more
visible badge delegates. These results document a measurable quality/cost
tradeoff within the established gate; they do not claim unchanged latency.

## Installation

The validated executable is installed at `/home/tom/.local/bin/ferric-browser`.
SHA-256 is `a8fe29b6b4e44bedffd93c908c55c306499595ed15e8624dbb894c14c1d73903`.
All 13 deployed files match the recorded source hashes; HTTP/HTTPS default
handlers are unchanged. Previous files are backed up under
`target/hint-title-alignment/previous-installation/`.

Installed help, privacy-safe diagnostics, private offscreen IPC and private
headless Weston startup/IPC checks pass. See `installation.json`, `install.log`,
`installed-diagnostics.json`, `installed-ipc.log`, `installed-wayland-check.log`
and `installed-wayland-ipc.log`. The user's existing browser session was not
closed; restarting Ferric loads this executable and its embedded scripts.

Physical input, screen-reader behavior and human usability on the user's exact
feed remain unqualified. This repair addresses the reproduced title-to-community
misassociation; it does not assert that every page layout is bug-free.
