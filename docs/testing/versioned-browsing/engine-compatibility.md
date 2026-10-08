# Engine compatibility and requalification policy

This policy governs native capability evidence and stored versioned-browsing
data separately. A runtime upgrade may withdraw a capability, but it must not
delete, rewrite, or silently reinterpret a durable commit.

## Fingerprint and evidence scope

Every release certificate must name the exact runtime tuple recorded in
[`capabilities.json`](capabilities.json): Qt, Qt WebEngine, Chromium base,
Chromium security patch, and distribution package revision. It must also record
the capability's backend, test-fixture revision, platform/compositor when the
observation is platform-sensitive, exact supported scope, unsupported cases,
and evidence IDs. A documentation version, successful build, or changed
feature flag is not a substitute for an installed-runtime fingerprint.

Certificates are exact-scope evidence, not a promise that all capabilities
share one support level. A change to a fingerprint component invalidates every
certificate whose evidence depends on that component. When the dependency is
unclear, invalidate the certificate rather than assuming compatibility. In
particular:

- Qt or Qt WebEngine changes invalidate affected lifecycle, save/download,
  profile-isolation, frame, accessibility, and archive-view certificates.
- Chromium base or security-patch changes invalidate browser-behavior and
  security-sensitive certificates, including offline archive interaction and
  page capture/restore behavior.
- A distribution package revision, native adapter/backend, request interceptor,
  profile ownership path, or helper bundle change invalidates certificates
  which exercise that component.
- A compositor, window-system, or platform change invalidates evidence whose
  claim includes native lifecycle, window ownership, keyboard, or accessibility
  behavior on that platform.

Invalidation changes the relevant gate to `unqualified`; it does not convert
`unsupported` into `supported`. A capability can return to `supported` only
after its primitive probe is repeated. Production use stays disabled until a
distinct release-qualification run passes for the exact fingerprint and scope.
The in-process requirement evaluator therefore requires the active runtime
fingerprint as an input; callers cannot ask it to resolve a certificate using
only its stored positive/negative state. Fingerprint construction also rejects
unknown/unavailable sentinel components, so matching placeholder strings
cannot masquerade as an observed runtime tuple.

## Requalification sequence

For each supported release runtime and any available transition runtime:

1. Record the installed fingerprint and compare it field-by-field with the
   certificate. Do not infer the Chromium security patch from the Qt version.
2. Run format compatibility fixtures for every retained manifest, page-sidecar,
   helper bundle, and archive backend schema that the build claims to read.
3. Repeat the relevant primitive probes, including negative/counterexample
   fixtures; then run the end-to-end qualification for each capability that
   could be enabled by the change.
4. Update only the affected capability records with new evidence IDs, exact
   scope, unsupported classes, and reason codes. Keep unrelated capabilities
   unchanged only when their evidence dependency is explicit.
5. If any required runtime or native surface is unavailable, leave the
   capability `unqualified` and production-disabled. Record a skip as a skip,
   not a pass.

The compatibility fixtures must cover the current stored format plus explicit
older and newer schema cases. A supported old schema may be read only through a
versioned, tested migration. Unknown older formats and newer formats are
read-only unavailable; they are not rewritten in place. Unknown engine
fingerprints do not invalidate the bytes of a structurally valid manifest, but
they do disable runtime-dependent interpretation and actions until requalified.

An unsupported archive backend is never guessed from file extension or byte
content. Archive references now carry the bounded backend ID
`qt-webengine-mhtml-v1`; the viewer accepts only that exact identifier. An
absent legacy identifier or an unknown valid identifier is refused with a fixed
unavailable result, leaves the stored object and commit intact, and never
substitutes the recorded live URL.
Changing backend or archive-format version requires both preserved-format
fixtures and native isolation qualification before the archive capability can
be re-enabled.

## Safe downgrade and retained data

When a runtime loses a previously supported capability:

- Preserve the profile-local journal, manifests, and immutable objects.
- Keep structural list/detail available when their schema and owner checks
  pass; do not imply that unavailable archive, live, cookie, or semantic state
  is restorable.
- Disable only actions whose required capability is unsupported or
  unqualified, and explain the fixed reason in the capability status surface.
- Never downgrade a richer representation to a weaker one silently. In
  particular, do not turn a failed archive open into live navigation, a failed
  sidecar restore into automatic form filling, or a failed live retention
  request into a claim of reconstructed continuity.
- Preserve explicit data-loss or cleanup operations as separate user-confirmed
  actions; an engine upgrade is not consent to prune history.

The current manifest writer emits schema 3. The reader has explicit,
non-writing schema-1-to-2 and schema-2-to-3 migrations. Schema 1 did not
define archive-backend identity, so migration ignores any backend claim
present in schema-1 bytes. Schema 2 did not define partial-page policy or
page-local archive failures, so migration discards any such claims in old
bytes. Legacy archive references remain visible as metadata but cannot be
opened without a qualified backend. Unknown older and newer schemas remain
unavailable. Runtime certificates and manifest migration coverage remain
separate release gates.

## Current qualification state

The checked-in runtime record currently names Qt/Qt WebEngine 6.11.2,
Chromium base 140.0.7339.225, security patch 151.0.7922.71, and package
6.11.2-1. The compatibility matrix has not yet been exercised across a
transition runtime. All production capability flags remain false until the
per-capability evidence records say otherwise. See the
[final campaign](final-campaign-2026-10-05.md) for current native blockers and
the [VB-093 implementation record](VB-093.md) for deferred tests.

The isolated patched engine now has a separate
[native baseline record](engine-capability-spike.md): three runs passed 40 checks
each on 2026-10-01. It reports the same version tuple but is **not** the package
artifact or a replacement for its certificates. Its patch, library, subprocess,
harness and log hashes are pinned separately. No production gate is promoted by
those synthetic cookie-copy and positive RTC-observation results.
The extended engine additionally passed three 26-check HTTPS runs with 19
canonical records and actual partition/ancestor request behavior, while retaining
three passing 40-check regressions. This is separately pinned experimental
evidence, not promotion of the package backend or a release certificate.

The v3 frozen-registry witness also passed three 35-check runs with Qt's default
process configuration and three with the exact `--site-per-process` switch; both
cross-origin cases in each stricter run measured two renderer processes. These
are separate configuration tuples, not evidence that default Qt uses OOP frames.
The original and HTTPS suites each passed three more times on v3. The source lock
pins every fixture (including the shared HTTPS fixture), patch, binary and log.
The witness restores its freeze before returning and therefore cannot certify
retained admission or complete media/capture inactivity.

The 2026-10-04 disposable Qt 6.11.2 extension adds a bounded pending-media to
active-stream handoff, a synthetic fake-desktop source, and an exact Qt Quick
`WebEngineView` owner bridge. The focused native RTC fixture passed once in
default mode and once with strict site isolation; the latter observed two
cross-origin renderer processes. The synthetic screen-capture route delivered
a nonzero track frame in both configurations. A separate Qt Quick fixture
passed once for empty admission, overlapping-acquire refusal, dispatch-time
invalidation, and refusal after a data-only peer appeared. These observations
show a feasible *scoped experimental route* on the pinned source; they do not
establish complete renderer/media/capture absence, all camera/microphone and
capture categories, renderer-loss handling, production Rust ownership, or
release qualification. The Quick bridge was exercised only in its recorded
configuration, not as a second process-isolation matrix.

This extension is a disposable source overlay, not approval to ship a Qt fork
or a promotion of `versioning.live_retention` or current-window replacement.
The patch order, zero-fuzz reconstruction evidence, fixture fingerprints,
runtime artifact hashes, and exact remaining coverage are recorded in
[`tools/versioning-engine-spike/source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json)
and the [capability-spike report](engine-capability-spike.md). Production
capabilities remain disabled; requalification would require the full native
activity/fault matrix and a separately approved shipping backend.
After those runs, the native and exact Qt Quick fixtures were extended with a
synthetic camera route (permission, frame, active-refusal and stop-cleanup
checks). Neither latest fixture has been built or run; the source lock keeps
the earlier exercised hashes distinct, so this does not add camera coverage to
the preceding results or change certificate status.
