# Versioned Browsing staged-plan campaign — 2026-10-07

This is a qualification checkpoint for the feasible, scoped implementation on
Qt/QtWebEngine 6.11.2. It is **not** full-feature completion or production
release approval.

## Queue and implemented scope

The queue audit separated scoped new-window structural restore (VB-024), exact
Save Page MHTML capture (VB-044), and clean single-tab off-record branching
(VB-059) from their stricter, unresolved extensions. VB-066 now adds one-use,
explicit structural export from an exactly linked transient child to its
normal-profile source owner. Its application branch suite passed 15/15 and
the feature-enabled Qt check/build passed. Native export-dialog and restart
journeys remain release-campaign work; the production clean-branch gate is
off. The pack validator reports 99 tasks: 52 scoped verified, 3 in progress,
39 blocked, 5 not started. `--next` returns no dependency/capability-ready
task; it must not be overridden by relabeling unresolved work complete.

The archive collector now records whether the native profile-key service
allowed media in the exact challenge. The application checks sidecar
categories against that native choice and the one-use consent. A missing key
cannot cause unkeyed media collection; non-media geometry/control data may
still be admitted when no key is available. The focused Qt callback suite
passed 16 active tests with 36 native fixtures ignored, and the focused
application capture suite passed 39/39.

## Final broad gate

`cargo xtask check --locked --offline` passed (exit 0) after correcting a
test-only argument-order error found by the first broad run. The passing run
included 253 QML passes, zero failures and eight expected live-fixture skips;
native profile-download routing, clean-branch profile/cookie separation, exact
MHTML archive and capture, restore, storage-full/cancellation/lifecycle fault
matrices, cookie/HTTPS probes, release-input manifest, license policy, full
workspace Rust tests and doc tests. The exact native archive matrix included
POST/redirect, mutating page, cross-origin frame, editor/selection, keyed
paused-media, same-URL child-frame reload, and archive cleanup cases.

This gate does not substitute for the still-missing compositor-visible
parent/child focus, multi-view clean-window host, branch popup/permission,
current-window replacement, arbitrary-site compatibility, or strict engine
capability acceptance journeys. The ordinary viewless MV3 extension probe
remained disabled because activation crashes on the tested runtimes. Cookie
probes confirmed that the public callback does not expose full SameSite and
partition identity or an exact completion boundary; they are negative
capability evidence, not cookie snapshot qualification.

## Backend and release decision

ADR-0016 records the decision to retain public Qt for scoped paths and not
ship the isolated patched-engine prototype. A shipping backend would need a
version-pinned Qt Quick owner bridge, complete WebRTC/media absence lease,
canonical cookie snapshot/seed/read-back and separately coordinated live
write-back, plus an ordinary viewless download origin and a security-update,
build, test and rollback commitment. These are review prerequisites, not
implementation claims. No production capability was enabled. VB-060, VB-061
and VB-063 remain in progress; the strict live-stash, cookie and viewless
download extensions remain blocked under their recorded gates.

## Post-check continuation: window clean-branch host

After the broad gate above, the test-feature window-scope `branch --clean`
route gained a hidden tab-capable off-record child host. Rust publishes one
ordered child lineage only after QML has allocated and checked every native
view; proved safe GETs start afterward. The focused application suite passed
15/15, the popup-owner QML fixture 9/9, the window-owner fixture 30/30, and
the feature-enabled Qt check/browser build passed. A disposable offscreen
three-tab native run published two separate ephemeral three-tab children
without changing the normal parent. Its counted local document received
exactly one additional GET in the second child. Restart showed only the normal
window and no branch-named WebEngine storage directory. This is scoped native
evidence, not compositor focus, confirmed close, popup/permission, page-load
failure or full release qualification.

The post-continuation `cargo xtask check --locked --offline` then passed with
256 QML passes, zero failures and eight expected skips, plus native probes,
workspace Rust tests and doc tests. A fresh feature-enabled browser build
passed. In a separate disposable native journey, a source with one counted
GET page and two blank tabs produced a three-tab clean child. The GET count
remained 1 after child publication, rose to 2 only on selecting the child
background page, and remained 2 on repeated selection. Attempting to close
the first-view lease anchor left the child at three tabs. Restart restored
only the normal window, with no branch-named on-disk WebEngine profile. The
test profile was permanently removed. These results do not qualify the
remaining VB-061 journeys or alter any production gate.

The child now maintains a bounded, URL-free `pending`/`loaded`/`failed`
outcome for each original page, keyed by stable tab ID rather than current
tab position. Its focused QML fixture passed 9/9. The second complete
`cargo xtask check --locked --offline` passed on this source (257 QML passes,
zero failures, eight expected skips; native probes, workspace Rust and doc
tests). An ordinary child `window-close` CLI request displayed confirmation
and did not silently close the child; confirmation itself was not exercised
in the offscreen journey. Native failed-page reporting and confirmed close
remain open. The feature-enabled browser build on this final QML source also
passed.

A final disposable feature-enabled native smoke check exercised the selected
proved-GET path with the outcome hook installed. After a controlled typed
source navigation, the fixture GET count rose from 8 to 9 on clean-child
publication, with both normal and ephemeral windows registered and no QML
diagnostic error. The direct startup URL had made multiple earlier requests,
so only the controlled delta is counted. This is not an inspection of the
private outcome array or failed-page qualification. The exact disposable
process, fixture and profile were removed.

In a bounded negative journey, the source completed one counted local GET,
the loopback fixture was stopped, and a clean child still registered alongside
the unchanged parent without a QML diagnostic error. No private child
outcome was observable through the public CLI, so failed-page reporting
remains unqualified. That disposable process/profile was also removed.

A later focused test-feature native Qt journey resolved the CLI visibility
gap without exposing page content: its one-use loopback source GET succeeded,
the child replay connection was dropped, and an internal diagnostic observed
the child page outcome `failed`. Qt's internal blank/error completion had
initially overwritten that outcome as `loaded`; the recorder now requires
the planned safe-GET start and keeps the first terminal result. The same
journey requested an ordinary child close, observed confirmation, proceeded,
and saw the child retire while the parent stayed live. The QML decision
fixture passed 9/9 and the focused native test passed. This is scoped
single-page failure/close evidence, not the mixed group, popup, permission,
focus or strict backend campaign.

The first post-change full gate found a timing race in an existing archive
test harness: it exited on the “capture saved” status before an independently
queued captured-tab close callback completed. The exact journey passed on
rerun. Its harness now waits for the already-required tab-count transition
within the existing deadline; it does not weaken the production assertion.
The corrected focused archive journey and the subsequent complete `cargo
xtask check --locked --offline` both passed. The complete gate again recorded
257 QML passes, zero failures and eight expected skips, all native archive
journeys, cookie/HTTPS negative capability probes, release-input and licence
checks, workspace Rust tests and doc tests. The final feature-enabled browser
build on this exact source also passed.

A subsequent feature-gated native Qt journey exercised a real scripted
popup from the published clean child. An internal, content-free diagnostic
observed the popup bound to the child's exact off-record profile, permission
UI and request interceptor. During child shutdown, the popup raised its own
page-state “Close anyway” prompt; after explicit confirmation, its approved
window object was released only when outstanding request/download ownership
was clear. Both popup and child retired while the normal parent remained.
The focused native test passed, as did the 9/9 branch-popup and 30/30
window-owner QML fixtures. This does not qualify target=_blank, redirect,
saved-trust/permission prompts, compositor focus, mixed-group failure or
strict cookie/WebRTC/viewless capabilities. The subsequent complete gate
passed on this popup-release source; no production gate changed.

On 2026-10-08 the feature-gated native journey expanded and passed twice.
It observed one three-tab child with an original-page `failed` replay,
unchanged failure attribution after child tab reorder, a separate `loaded`
safe page and a blank `pending` placeholder while the normal three-tab
parent stayed intact. Separate clean children opened scripted and
`target=_blank` popups; both followed a local 302 and kept the exact
off-record profile, child permission UI and request interceptor. A saved
normal-profile exact-origin notifications allow did not carry into another
clean child: the child showed a fresh prompt, the parent none, and explicit
deny/confirmed close completed. These are focused native tests, not a
shipping capability qualification. Failed allocation, first-view anchor
replacement, cross-origin OAuth/certificate trust, compositor focus and
the strict engine guarantees remain open. The complete workspace gate passed
after this expanded test bridge, as did the feature-enabled browser build,
focused native journey twice, pack validation and checksum verification.
No production gate was enabled.

The later feature-gated branch native journey was hardened against startup
navigation races: it requires a committed normal source page and a distinct
typed safe-GET proof, counts only fixture-rejected uncookied child requests,
and explicitly confirms the separate page-state close prompt when presented.
The same run also refuses a clean window branch with its tab-capable factory
absent while leaving the normal parent intact. It passed once through Cargo
and five more times in fresh processes. This is focused branch evidence; the
full post-change workspace gate and browser build were still pending at that
point; no production capability is promoted by these tests alone.

The post-change `cargo xtask check --locked --offline` then passed: 257 QML
passes, zero failures, eight expected skips, native archive/cookie/HTTPS
probes, release and licence checks, workspace Rust tests and doc tests. The
feature-enabled browser build also passed. Public-Qt cookie identity remained
unqualified in its negative probe, and no production capability gate changed.

A further test-feature native extension passed through Cargo and five
fresh-process repeats. Scripted and `target=_blank` clean-child popups followed
the loopback `127.0.0.1` → `localhost` → `127.0.0.1` redirect round trip while
retaining exact off-record child profile, permission UI and interceptor
ownership. A user `tab-close` on the mixed group's selected first-view anchor
was refused by the core without changing any child tab ID/order. These are
additional scoped journeys, not external OAuth/certificate-trust or late
view-failure qualification. The post-extension `cargo xtask check --locked
--offline` passed (257 QML passes, zero failures, eight expected skips,
native probes/journeys, release/licence, workspace Rust and doc tests), as
did the feature-enabled browser build. Production capabilities remain disabled.

On 2026-10-08 the feature-gated native branch journey additionally passed a
late extra-view factory loss after a hidden two-tab child had bound its native
owner but before lineage publication. The unpublished child retired, the
two-tab parent remained intact, and a subsequent branch from that parent
published and closed normally. The driver explicitly navigates published
children to popup/permission fixtures, keeping that evidence separate from
the distinct typed safe-GET replay proof. Cargo ran the focused journey and
five further fresh-process runs passed. Partial-view creation, first-view
replacement, compositor focus, external OAuth and saved trust remain open;
the production capability gates remain off. The complete post-change
`cargo xtask check --locked --offline` passed (257 QML passes, zero failures,
eight expected skips, native probes/journeys, workspace Rust and doc tests),
as did the feature-enabled browser build.

A further 2026-10-08 VB-061 native revision directly invoked QML's
first-view close guard after the mixed-group reorder. It refused closure and
kept the original child tab IDs/order intact, beyond the earlier core-command
refusal. A separate repeat exposed a refused child GET that never delivered a
terminal Qt load callback; started-but-pending clean-branch pages now fail
closed after a 15-second deadline with an exact live-view recheck. The
timeout decision fixture passed 10/10, and the combined native journey passed
through Cargo and five fresh-process repeats. A later disposable native
fixture held the first uncookied child replay socket open beyond the deadline
and observed the timeout-specific `failed` result before releasing it; this
also passed through Cargo and five fresh-process repeats. The mixed-page
route separately retained an immediately dropped replay. First-view
replacement, compositor focus and strict backend guarantees remain open.
The final post-fixture `cargo xtask check --locked --offline` passed (258 QML,
zero failures, eight expected skips, native probes/journeys, workspace Rust
and doc tests), as did the feature-enabled browser build.
