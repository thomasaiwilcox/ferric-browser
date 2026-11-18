# Hint reliability closeout — 16 November 2026

This pass fixes the previously reproduced busy-page collection failure, retains
the complete two-tab playback journey as an integration test, and reconciles
the acceptance inventory. It keeps the existing policy architecture, one-second
script deadline, global work ceilings, and one bounded request retry.

## Busy-page snapshots

A dirty notification previously discarded the pending ordinary inventory. A
clock/progress update between ordinary discovery and policy extraction could
therefore defeat both attempts. Publication and geometry checks also treated
unrelated later dirty notifications as invalidating the captured snapshot.

A dirty pending inventory is now held weakly until completion or retirement.
The policy stage may repeat ordinary discovery using its remaining native grant,
then extract policy facts in the same synchronous renderer turn. It proceeds
only when the complete candidate projection and truncation state equal the
original native inventory. Text, geometry, physical-identity or membership
changes refuse completion; the existing bounded retry may capture a new request.
Policy matching is recomputed even when candidate rows are unchanged.

The repeated scan consumes the original request's remaining inspection/predicate
allowance. Invalid or inflated grants are refused before DOM work. The original
nonce, checked input identity, selected-world priority and aggregate accounting
are preserved. An advanced renderer snapshot revision needs charged work and
native completion; it is never a configuration revision or an action permit.

Native sealing compares the retained coherent snapshot, documents, references,
plan, stamp and geometry rather than demanding page-wide quiescence. A later
collection still retires the old proof. Selection retains fresh semantic and
native action checks. Native frame ancestry filters bounded passive mutation
batches for actual path/slot dependencies, and captures the frame border/content
geometry. Verification rejects movement or resize while allowing geometry-neutral
updates. Its probe retains the conservative unprocessed-record fence. Slot and
host-path ABA, navigation, replacement, focus changes and detached objects remain rejection cases.

## Measurements and validation

The new style/text/attribute regression fails all three positive rows before the
fix. The previous production build fails both deterministic busy-page journeys.
Those journeys mutate the DOM after every suspension, including the internal
refresh, so a quiet interval cannot accidentally make the test pass.

The qualified build passes both busy journeys with **one captured request each**.
Diagnostic command-to-verified-publication samples are
**464 ms** for Top and
**1842 ms** for a selected foreign frame.
These are individual offscreen software-fixture observations with QtTest polling,
not paint-latency percentiles or a new performance-gate qualification.

| Current check | Result |
| --- | --- |
| Production browser/engine build and embedded resource parity | Passed |
| Scoped native browser run, including busy collection, routing, collections and ownership | 144 passed, 0 failed |
| Corrected explicit stale-refresh fixture, two additional independent replays | Passed with the original five-second wait |
| Collection work/ledger Rust suite | 48 passed; production Rust unchanged, test-source difference verified as formatting only |
| Preset/interchange Rust suite | 7 passed |
| `tst_deferred_collection.qml` | 50 passed |
| `tst_proof_objects.qml` | 100 passed |
| `tst_composed_depth.qml` | 34 passed |
| `tst_native_document_receipts.qml` | 26 passed |
| `tst_frame_receipts.qml` | 24 passed |
| Explicit selector-replacement exclusion | Passed |

The native suite includes the actual passive-burst activation case. Adapter
handle assertions freeze passive polling only after publication; the explicit
stale-refresh routing fixture stops the competing background poll before owning
its manual cycle. Neither fixture relaxes a product deadline or permits a stale
action. Busy request counts come from distinct captured plans; the UI retry
counter resets after publication and is not used to infer retry counts.

## Permanent two-tab media regression

`ApplicationTests::hintPageKeyAcrossTabs` is registered as
`application_hint_media_tabs` in CTest (60-second process ceiling). It uses two
local HTTP documents and a small owned silent VP8 WebM, with the approved Hint
and explicit-passthrough mapping contexts. Both documents remain alive through
A → B → A switching. It uses the default `f` binding and automatic receiver
handoff; an explicit receiver probe would hide the original ownership defect.

Six `p` gestures each deliver exactly one trusted `k` down/up pair, change real
video playback and return to Normal. Six subsequent Normal `p` checks send no
page event and leave playback unchanged. Inactive-document event counts remain
unchanged, and both videos finish paused. Six recorded publication observations
are retained in the native log; they are not general latency bounds. The shipped
preset parser regression now rejects Normal admission and requires Hint and both
explicit passthrough contexts.

## Acceptance and scope

The original 112 scenarios are preserved. **106** now have concrete
source/test mappings, with retained historical receipts and current closeout
receipts distinguished. Partial coverage does not certify every clause/backend
in a scenario. Historical planned test names are not executable evidence.
Open physical-input/platform cases are not declarations of missing implementation.

The Qt 6.11.2 interpreter safety workaround remains. The wider X11/layout/physical
input, installed-package/file-dialog and complete accessibility matrices remain
unqualified. Earlier installed YouTube media checks retain their earlier binary
scope; this build's new media and busy-page qualification uses owned fixtures,
not a new replay of the user's live YouTube homepage.

A genuinely changing inventory, exhausted remaining budget, or conservative
slot/shadow structural fence may still refuse a collection. This pass does not
promise availability under arbitrary continuous structural churn. The owner's
deferred 5,000-target deadline and original failed latency gates remain failed
or unqualified in their historical records. No new performance waiver or pass
is inferred.

## Evidence and reproduction

Raw receipts are under ignored `target/hint-reliability-closeout/`, including the
original red cases and intermediate failures. Initial fixture setup used a runner
without FrameReceiptProbe; the first media test queried a not-yet-created view
and aborted through the test helper's fatal diagnostic. Its core
identified that helper path. The corrected test waits for the selected view.
A native proof regression caught parent-frame movement; explicit captured-geometry
comparison preserves its retirement. The clock fixture is placed outside normal
flow so changing its text does not move the receiver. Earlier busy handle
assertions raced an automatic subsequent refresh, and an
explicit stale-refresh wait raced background polling. Their failing receipts
remain preserved. Final receipts use the corrected fixtures and current artifact.

`final/qualification-checks.json` is the final check record;
`final/release-source-inputs.json` captures its exact inputs. The earlier
`build-checks.json`/`completion-checks.json` files keep their original failed or
source-mismatch statuses and are not relabelled as qualification. The final
reporting/acceptance documentation changes occur after the executable checks and
are audited separately at handover.

Qualified browser SHA-256: `f3c98058f2a14e792c6eea5f7998c845ed5b0db96af3ef401a72a9c67aa045cc`.

With the qualified SDK selected, the focused native checks are reproducible as:

```sh
tools/performance/with_qt611.sh "$APPLICATION_TESTS" \
  embeddedResourceParity hintRequestWorkspaceCollection:busy-top \
  hintRequestWorkspaceCollection:busy-foreign hintPageKeyAcrossTabs \
  hintPageKeyRouting:stale-refresh
```

The real-origin suites use
`tests/site-policy-http/run_fixture.py --runner "$QML_TESTS" --test-file NAME`
under `crates/ferric-browser-engine-qt/`. The harness owns and closes its loopback
server. No user profile, saved site policy or public API is migrated by this fix.
