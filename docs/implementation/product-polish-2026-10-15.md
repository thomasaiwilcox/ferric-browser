# Refined browser experience — implementation report

Status: complete and qualified on 2026-10-18.
Branch: `polish/refined-browser-experience`. Base commit: `c490311`.

## Product intent

Ferric should feel precise and capable to someone who uses commands and plain
configuration files, with the clarity and finish expected of an everyday
browser. Human-readable meaning and useful next actions lead. Real command
names, configuration keys, security boundaries and selectable diagnostics stay
available deliberately.

All seven original areas remain covered by this pass. The chronological
[checkpoint archive](product-polish-2026-10-15-checkpoints.md) preserves the
investigations, failed reproductions and evidence from earlier source versions.
Its historical pending items are superseded by the current report.

## Completed scope

| Area | Implemented result | Final acceptance |
| --- | --- | --- |
| Site Ledger, Site Doctor and diagnostics | Readable summaries, explicit report review, literal page data, scoped owners, asynchronous collection/export and useful unavailable states. | Passed — Compiled owner/export/lifecycle journeys and rendered surfaces. |
| Profiles, sessions and contexts | Typed previews, draft retention, pending/error feedback, catalog refresh, durable permissions and safe context reload/deletion. | Passed — Worker, restart, failed-storage and multi-window integration. |
| Start, guide, libraries and navigation | Readable labels and explanations, truthful loading/empty/failed states, action details, native-page history and visible-target confirmation. | Passed — Guide lifetime, library, navigation and target-search journeys. |
| Status and error messages | Changed pages/selections, history boundaries, failed configuration work and file-save failures explain recovery. Late callbacks retain newer feedback. IDs and raw records stay in deliberate details/protocols. | Passed — Rust contracts, actual command feedback and public CLI/IPC. |
| Prompts | Safe defaults, plain-text origins/content, captured request owners, bounded layout, keyboard dismissal and focus restoration. | Passed — Cross-window authentication/TLS/permission prompts, native chooser/notification boundaries and compact renders. |
| Whole-window finish | Primary, secondary, private, temporary and popup workflows retain their own input and scope. Larger-font download controls stay visible; keyboard focus reveals controls after scrolling. Library deletion feedback respects the active modal owner. | Passed — Compiled window/manager/clipboard journeys and rendered walkthrough. |
| Combined evidence | Source and production artifacts remain frozen through the repository, native and public-process checks. Normal profiles and the installed browser are separate from owned fixtures. | Passed — Final qualification manifest and manual render review. |

## Asynchronous focus and interaction ownership

Library deletion feedback restores focus only while the library is the active,
interactive modal surface. Both the queued timer and its focus entry point use
the existing modal ownership rule. The regression reproduced focus moving behind
a covering dialog before the fix; all 18 library checks now pass. Cases include
opening the dialog before feedback or before the queued timer, normal frontmost
restoration, and returning from the covering dialog.

Periodic Grid geometry observations now belong only to the selection phase.
An observation issued before Enter can return after dispatch begins; it is then
ignored. The action still performs its own current geometry, owner and native
surface validation. A controlled native regression first demonstrated the old
callback clearing the active dispatch envelope. The corrected engine preserves
that envelope and completes the trusted click. Live geometry-change rejection
and the secondary/private/popup matrix are also qualified.

## Download reliability

Explicit commands, hints and Retry use a fixed browser-owned document in the
captured Qt profile. A cancellable fetch keeps cookies and authentication in
that profile without navigating the source page. Ordinary native page downloads
retain their guarded source-ownership path.

Controller leases admit the exact request through the existing interceptor
policy. A changed destination is stopped before its request and checked before
restarting from a fresh controller. Redirect limits, embedded credentials,
HTTPS downgrade and security-denied targets remain enforced. A controller
already controlled by a site worker is rejected. Website content cannot execute
inside the fixed document; response metadata and byte delivery are bounded.

A bounded Rust worker accepts at most eight live streams per window and one
unacknowledged 64 KiB chunk per stream. Pause/Resume retain buffered data; Cancel
and owner retirement revoke further writes. Stable numeric phase values define
the QML adapter boundary. Native/public checks verify actual TCP cancellation
before headers, complete binary payloads, authentication and source preservation.

File preparation runs off the GUI thread. It creates an exclusive hidden file
inside the selected directory, copies and synchronizes its contents there, and
retains the directory descriptor. The GUI validates the captured session, owner
and destination immediately before exclusive publication. Existing files are
preserved. Cancellation and retirement prevent late publication without joining
a blocked worker. File contents are synced; the final directory entry is not
separately fsynced, so directory-rename crash durability is outside this claim.

The session retains final size and failure details after the transport object
ends. Saving controls read those authoritative values, show Cancel and withhold
Open until publication succeeds. Saving does not display completed network
progress. A same-profile peer can control the original owner while counting
only its own work for close warnings.

The download-source compatibility guard is isolated to the exact QtWebEngine
6.11.2 SDK under ADR-0002. A runtime mismatch fails before controller creation
or a target request. Refresh the interface, notices and regressions when
changing the qualified SDK.

## Validation record

The final manifest is under ignored
`target/product-polish/final-polish-qualification-v7/`. It records source hashes,
commands, logs, frozen executable/library hashes, the causal before-test and
explicit provenance for retained checks of unchanged subsystems.

- Current-source strict workspace Clippy passed with warnings denied.
- `cargo xtask check` passed: 1100 Rust tests, 631 QML checks, native profile
  and download-source boundaries, architecture, generated documentation,
  formatting, licensing and release-artifact policy. Two opt-in benchmark/model
  tests remain intentionally ignored.
- The corrected production engine passed 14 focused spatial checks and
  190 complete window checks. The full action-feedback suite passed
  25 checks. Embedded-resource parity is included.
- Download acceptance passed all three groups: 64 transport/TLS/policy checks,
  seven held-sync cases (nine QtTest results including setup/cleanup), and the
  actual runtime-version mismatch probe. The seven held-sync rows skipped by
  the ordinary download executable passed in their dedicated interposed run.
- Clipboard passed 81 checks. Close, switcher and context reload passed 16, 11
  and 33 checks. Five chooser/notification groups passed; both portal groups
  passed after updating obsolete fixture contracts. The two-process history
  probe retained four records and both exact saved payloads.
- Public Downloads passed both private-query configurations. Startup recovery
  and public action feedback passed. These unchanged paths retain their scoped
  evidence in the final manifest.
- Manual review covered 159 rendered views across 18 contact sheets, with
  full-size checks of compact saving controls, large-font library/permission
  surfaces and window-scoped overlays. The review and image hashes are recorded
  in `target/product-polish/final-render-review/review.json`. All 110 product
  QML/JavaScript files are identical to the reviewed interface sources.

The completion batch also corrected obsolete native test fixtures: completion
bookkeeping now uses a real live owner/profile, JavaScript test doubles exercise
common admission without entering the native QObject accessor, native retirement
checks actual connection abortion, and session retries remain available while
saved history refreshes. No ownership or security check was relaxed.

The architecture checker requires the opaque `BrowserApplication` boundary and
continues to reject raw model writes and reducer bypasses. Its positive/negative
regressions pass. The unchanged default initializer moved into a dedicated
module, keeping the bridge at 2,947 lines within its existing 3,500-line limit.

## Scope of the evidence

Owned Qt offscreen, local HTTP/HTTPS, private D-Bus and nested compositor
fixtures establish the stated application behavior. Simulated identity and
security-key prompts establish ownership/layout, not real credential or key
transport. Physical camera/microphone, hardware keys, live desktop notification
services and compositor protocols retain their documented capability limits.
Those limits remain explicit in the product rather than implying access worked.

No installation, commit, push or publication is part of this completion pass.
