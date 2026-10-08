# Structural continuity scope — draft, not a release approval

Status: VB-029 is in progress. A current-tree campaign is recorded in
`final-campaign-2026-10-05.md`, but it covered the source state at that time;
subsequent continuation edits remain unverified. This document describes the
intended narrow release boundary, not a qualification result or release
approval.

## Intended supported surface

Structural continuity is limited to durable normal-profile commits of bounded
browser structure: logical windows/pages, approved context membership, and
navigation descriptors. A location is replayable only when the existing
runtime has exact safe-GET evidence for it; uncertain or unsafe locations are
stored as non-replayable placeholders. Restoring a commit opens a fresh,
tab-capable window, loads only the selected qualified page, leaves inactive
pages lazy, and does not mutate the source window.

The implementation also provides bounded commit listing and details, deletion
preview, object validation/recovery, and same-profile context coordination.
The recorded campaign exercised these paths on its source snapshot; the
end-of-goal campaign must include all later edits before VB-029 can be
reassessed.

## Explicitly excluded

This surface does not restore page bodies, submitted form data, browser
history, cookies, arbitrary application state, or live renderer state. It does
not promise a simultaneous snapshot across windows. Private and transient
profiles are not structural capture sources. Current-window replacement stays
unavailable until a qualified live-retention path can preserve unknown work.

Page archives and their restricted viewer, cookie snapshot/seed/apply, media
position restore, and live-view retention each have separate capability and
security gates. A structural milestone must not enable or imply any of them.
Disabling one of those capabilities must leave already stored commit bytes
intact; unavailable formats are retained as unavailable rather than silently
converted to live navigation.

## Evidence required before calling VB-029 verified

- End-of-goal core, application, and storage versioning suites, including
  restart, corruption, quota refusal, and normal/private/transient isolation.
- The repository quality gate and relevant disposable Qt adapter/native
  journeys.
- A durable normal-profile commit surviving restart and exact selected-only
  safe-GET restore, with the source intact and inactive pages still unloaded.
- Regression-clean existing sessions, contexts, downloads, and configuration.
- A result record naming the tested Qt/Chromium runtime, exact commands, pass
  counts, skips, and unresolved failures. The earlier 2026-10-05 campaign is
  useful baseline evidence, not a substitute for checking later source edits.

After those checks, update [VB-029](VB-029.md) with what passed and preserve
every excluded capability above as a separate gate. VB-057's archive release
campaign remains independent.
